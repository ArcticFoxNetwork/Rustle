//! Song resolver module
//!
//! Provides unified song resolution for both local and NCM songs.
//! Handles caching, URL fetching, and cover downloading.
//! Uses SharedBuffer for streaming playback (no file-based streaming).

use std::sync::Arc;

use crate::api::NcmClient;
use crate::api::{NcmQualityLevel, TrackUrl};
use crate::audio::identity::PlaybackContext;
use crate::audio::streaming::{
    SharedBuffer, StreamingEvent, StreamingEventKind, StreamingIdentity, start_buffer_download,
    wait_for_buffer_playable,
};
use crate::database::DbSong;

/// Result of resolving a song with streaming support
#[derive(Debug, Clone)]
pub struct ResolvedSong {
    /// Finalized cache file path with the detected audio extension.
    /// `None` means playback remains ring-buffer backed.
    pub finalized_cache_path: Option<String>,
    /// Local cover path (if available)
    pub cover_path: Option<String>,
    /// Shared buffer for direct memory playback (None if using cached file)
    pub shared_buffer: Option<SharedBuffer>,
    /// Duration in seconds (from API)
    pub duration_secs: Option<u64>,
    /// Quality requested by settings and the quality actually returned by NCM.
    pub quality: Option<ResolvedAudioQuality>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAudioQuality {
    pub requested: NcmQualityLevel,
    pub actual: NcmQualityLevel,
    pub bitrate: Option<u32>,
    pub size: Option<u64>,
    pub format: Option<String>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
    pub channels: Option<u32>,
    pub channel_layout: Option<String>,
    pub immerse_type: Option<String>,
}

impl From<&TrackUrl> for ResolvedAudioQuality {
    fn from(url: &TrackUrl) -> Self {
        Self {
            requested: url.requested_level,
            actual: url.level,
            bitrate: (url.rate > 0).then_some(url.rate),
            size: url.size,
            format: url.format.clone(),
            sample_rate: url.sample_rate,
            bit_depth: url.bit_depth,
            channels: url.channels,
            channel_layout: url.channel_layout.clone(),
            immerse_type: url.immerse_type.clone(),
        }
    }
}

/// Check if a song needs resolution (NCM song without local file)
pub fn needs_resolution(song: &DbSong) -> bool {
    // NCM songs have negative IDs or file_path starting with "ncm://"
    let is_ncm = song.id < 0 || song.file_path.starts_with("ncm://");

    if !is_ncm {
        return false;
    }

    // Check if we have a valid local file
    if song.file_path.is_empty() || song.file_path.starts_with("ncm://") {
        return true;
    }

    // Check if the file actually exists
    !std::path::Path::new(&song.file_path).exists()
}

/// Get NCM song ID from DbSong
pub fn get_ncm_id(song: &DbSong) -> u64 {
    if song.id < 0 {
        (-song.id) as u64
    } else if song.file_path.starts_with("ncm://") {
        song.file_path
            .trim_start_matches("ncm://")
            .parse()
            .unwrap_or(song.id as u64)
    } else {
        song.id as u64
    }
}

/// Resolve a song with streaming support
///
/// This function:
/// 1. Checks whether the preferred quality is already cached locally
/// 2. Otherwise negotiates the actual quality with the official playback API
/// 3. Reuses an actual-quality cache or streams into one with SharedBuffer
/// 4. Reuses a cover already cached by the unified image pipeline
pub async fn resolve_song(
    client: Arc<NcmClient>,
    song: &DbSong,
    context: PlaybackContext,
    event_tx: tokio::sync::mpsc::Sender<StreamingEvent>,
) -> Result<ResolvedSong, String> {
    let ncm_id = get_ncm_id(song);
    let identity = StreamingIdentity::Playback(context.clone());

    let song_cache_dir = crate::utils::songs_cache_dir();

    std::fs::create_dir_all(&song_cache_dir)
        .map_err(|error| format!("failed to create song cache directory: {error}"))?;

    // Use stem for cache lookup - actual extension determined by format detection
    let requested_level = client.current_quality_level();
    // Before URL negotiation, only the preferred-level cache can be identified
    // safely. After negotiation, the server-returned actual-level cache is
    // checked separately below.
    let requested_stem = format!("{}_{}", ncm_id, requested_level.api_level());

    // Cover downloads are handled by app::update::images; song resolution only reuses cache.
    let cover_path = resolve_cover(ncm_id).await;

    // Check if the exact requested quality is already fully cached.
    if let Some(cached_path) = crate::utils::find_cached_audio(&song_cache_dir, &requested_stem) {
        let file_size = std::fs::metadata(&cached_path)
            .map(|m| m.len())
            .unwrap_or(0);
        // Use duration-based heuristic: 40KB/s at 320kbps
        let expected_min_size = (song.duration_secs as u64) * 40 * 1024;
        let is_complete =
            file_size > 0 && (expected_min_size == 0 || file_size >= expected_min_size * 8 / 10);

        if is_complete {
            tracing::debug!(
                "Song {} found in cache: {:?} ({} bytes)",
                ncm_id,
                cached_path,
                file_size
            );
            let _ = event_tx
                .send(StreamingEvent::new(
                    identity.clone(),
                    StreamingEventKind::Playable,
                ))
                .await;
            let _ = event_tx
                .send(StreamingEvent::new(
                    identity.clone(),
                    StreamingEventKind::Complete,
                ))
                .await;
            return Ok(ResolvedSong {
                finalized_cache_path: Some(cached_path.to_string_lossy().to_string()),
                cover_path,
                shared_buffer: None,
                duration_secs: None,
                quality: Some(ResolvedAudioQuality {
                    requested: requested_level,
                    actual: requested_level,
                    bitrate: None,
                    size: Some(file_size),
                    format: cached_path
                        .extension()
                        .and_then(|value| value.to_str())
                        .map(ToString::to_string),
                    sample_rate: None,
                    bit_depth: None,
                    channels: None,
                    channel_layout: None,
                    immerse_type: None,
                }),
            });
        }

        tracing::info!(
            "Song {} cache incomplete ({} bytes), using streaming buffer",
            ncm_id,
            file_size
        );
        // Remove incomplete cache file
        let _ = std::fs::remove_file(&cached_path);
    }

    // Get song URL
    tracing::info!("Downloading song {} from NCM (streaming)", ncm_id);
    let url = match client.resolve_track_url(ncm_id, requested_level).await {
        Ok(url) => url,
        Err(error) => {
            let message = format!(
                "歌曲 {ncm_id} 获取官方播放地址失败（音质偏好 {}）：{error}",
                requested_level.api_level()
            );
            tracing::error!("{message}");
            let _ = event_tx
                .send(StreamingEvent::new(
                    identity.clone(),
                    StreamingEventKind::Error(message.clone()),
                ))
                .await;
            return Err(message);
        }
    };

    let song_url = url.url.clone();
    let quality = Some(ResolvedAudioQuality::from(&url));

    // Use stem-based path - actual extension will be determined during download
    // The download function will detect format and save with correct extension
    let actual_stem = format!("{}_{}", ncm_id, url.level.api_level());
    if actual_stem != requested_stem
        && let Some(cached_path) = crate::utils::find_cached_audio(&song_cache_dir, &actual_stem)
    {
        let file_size = std::fs::metadata(&cached_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        let expected_min_size = (song.duration_secs as u64) * 40 * 1024;
        let is_complete =
            file_size > 0 && (expected_min_size == 0 || file_size >= expected_min_size * 8 / 10);
        if is_complete {
            let _ = event_tx
                .send(StreamingEvent::new(
                    identity.clone(),
                    StreamingEventKind::Playable,
                ))
                .await;
            let _ = event_tx
                .send(StreamingEvent::new(
                    identity.clone(),
                    StreamingEventKind::Complete,
                ))
                .await;
            return Ok(ResolvedSong {
                finalized_cache_path: Some(cached_path.to_string_lossy().to_string()),
                cover_path,
                shared_buffer: None,
                duration_secs: None,
                quality,
            });
        }
        let _ = std::fs::remove_file(cached_path);
    }
    let cache_path = song_cache_dir.join(actual_stem);

    // Use unified download function - content_length will be obtained from GET response
    let shared_buffer =
        start_buffer_download(song_url, cache_path.clone(), identity, Some(event_tx));

    if !wait_for_buffer_playable(&shared_buffer, 30).await {
        tracing::error!(
            "Song {} did not reach the streaming startup watermark",
            ncm_id
        );
        shared_buffer.cancel();
        return Err(format!("歌曲 {ncm_id} 未达到流式播放启动缓冲水位"));
    }

    // The downloader continues filling the bounded window and sparse cache in
    // the background after the decoder has a stable startup reserve.
    Ok(ResolvedSong {
        finalized_cache_path: None,
        cover_path,
        shared_buffer: Some(shared_buffer),
        duration_secs: Some(song.duration_secs as u64),
        quality,
    })
}

async fn resolve_cover(ncm_id: u64) -> Option<String> {
    crate::image::resolve_cached(crate::image::ImageKind::SongCover, ncm_id)
        .map(|p| p.to_string_lossy().to_string())
}
