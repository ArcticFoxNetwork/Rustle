//! Song resolver module
//!
//! Provides unified song resolution for both local and NCM songs.
//! Handles caching, URL fetching, and cover downloading.
//! Uses SharedBuffer for streaming playback (no file-based streaming).

use std::path::PathBuf;
use std::sync::Arc;

use crate::api::NcmClient;
use crate::audio::streaming::{SharedBuffer, StreamingEvent, start_buffer_download};
use crate::database::DbSong;

/// Result of resolving a song with streaming support
#[derive(Debug, Clone)]
pub struct ResolvedSong {
    /// Local file path (for caching reference)
    pub file_path: String,
    /// Local cover path (if available)
    pub cover_path: Option<String>,
    /// Shared buffer for direct memory playback (None if using cached file)
    pub shared_buffer: Option<SharedBuffer>,
    /// Duration in seconds (from API)
    pub duration_secs: Option<u64>,
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
/// 1. Checks if the song is already cached locally (with any audio extension)
/// 2. If not, downloads using SharedBuffer for streaming playback
/// 3. Downloads cover if not already cached
pub async fn resolve_song(
    client: Arc<NcmClient>,
    song: &DbSong,
    event_tx: tokio::sync::mpsc::Sender<StreamingEvent>,
) -> Option<ResolvedSong> {
    let ncm_id = get_ncm_id(song);

    let song_cache_dir = crate::utils::songs_cache_dir();
    let cover_cache_dir = crate::utils::covers_cache_dir();

    std::fs::create_dir_all(&song_cache_dir).ok()?;
    std::fs::create_dir_all(&cover_cache_dir).ok()?;

    // Use stem for cache lookup - actual extension determined by format detection
    let song_stem = ncm_id.to_string();
    let cover_file_path = cover_cache_dir.join(format!("cover_{}.jpg", ncm_id));

    // Resolve cover in background
    let cover_path = resolve_cover(
        &client,
        ncm_id,
        song.cover_path.as_deref(),
        &cover_file_path,
    )
    .await;

    // Check if song is already fully cached (with any audio extension)
    if let Some(cached_path) = crate::utils::find_cached_audio(&song_cache_dir, &song_stem) {
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
            let _ = event_tx.send(StreamingEvent::Playable).await;
            let _ = event_tx.send(StreamingEvent::Complete).await;
            return Some(ResolvedSong {
                file_path: cached_path.to_string_lossy().to_string(),
                cover_path,
                shared_buffer: None,
                duration_secs: None,
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
    let urls = match client.songs_url(&[ncm_id]).await {
        Ok(urls) => urls,
        Err(e) => {
            tracing::error!("Failed to get song URL for {}: {}", ncm_id, e);
            let _ = event_tx.send(StreamingEvent::Error(e.to_string())).await;
            return None;
        }
    };

    let song_url = match urls.first() {
        Some(u) if !u.url.is_empty() => u.url.clone(),
        _ => {
            tracing::error!("No valid URL returned for song {}", ncm_id);
            let _ = event_tx
                .send(StreamingEvent::Error("No URL available".to_string()))
                .await;
            return None;
        }
    };

    // Use stem-based path - actual extension will be determined during download
    // The download function will detect format and save with correct extension
    let cache_path = song_cache_dir.join(&song_stem);

    // Use unified download function - content_length will be obtained from GET response
    let shared_buffer = start_buffer_download(song_url, cache_path.clone(), Some(event_tx));

    // Return immediately with the buffer
    // Note: file_path uses stem only - actual cached file will have correct extension
    Some(ResolvedSong {
        file_path: cache_path.to_string_lossy().to_string(),
        cover_path,
        shared_buffer: Some(shared_buffer),
        duration_secs: Some(song.duration_secs as u64),
    })
}

/// Resolve cover image - download if not cached or return existing local path
async fn resolve_cover(
    client: &NcmClient,
    ncm_id: u64,
    pic_url: Option<&str>,
    _cover_file_path: &PathBuf,
) -> Option<String> {
    // First, check if cover already exists in local cache
    let cover_cache_dir = crate::utils::covers_cache_dir();
    let stem = format!("cover_{}", ncm_id);
    if let Some(existing_path) = crate::utils::find_cached_image(&cover_cache_dir, &stem) {
        return Some(existing_path.to_string_lossy().to_string());
    }

    // If not cached, try to download from URL
    if let Some(url) = pic_url {
        if !url.is_empty() && url.starts_with("http") {
            tracing::debug!("Downloading cover for song {}", ncm_id);
            if let Some(path) = crate::utils::download_cover(client, ncm_id, url).await {
                return Some(path.to_string_lossy().to_string());
            }
        }
    }

    None
}
