//! Song resolver module
//!
//! Provides unified song resolution for both local and NCM songs.
//! Handles caching, URL fetching, and cover downloading.

use std::path::PathBuf;
use std::sync::Arc;

use crate::api::NcmClient;
use crate::database::DbSong;

/// Result of resolving a song
#[derive(Debug, Clone)]
pub struct ResolvedSong {
    /// Local file path to play
    pub file_path: String,
    /// Local cover path (if available)
    pub cover_path: Option<String>,
    /// Whether the song was cached
    pub was_cached: bool,
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

/// Resolve a song - get playable file path and cover
///
/// This function:
/// 1. Checks if the song is already cached locally
/// 2. If not, downloads the song from NCM
/// 3. Downloads cover if not already cached
///
/// The music quality is determined by the client's internal setting.
pub async fn resolve_song(client: Arc<NcmClient>, song: &DbSong) -> Option<ResolvedSong> {
    let ncm_id = get_ncm_id(song);

    let song_cache_dir = crate::utils::songs_cache_dir();
    let cover_cache_dir = crate::utils::covers_cache_dir();

    // Ensure directories exist
    std::fs::create_dir_all(&song_cache_dir).ok()?;
    std::fs::create_dir_all(&cover_cache_dir).ok()?;

    let song_file_path = song_cache_dir.join(format!("{}.mp3", ncm_id));
    let cover_file_path = cover_cache_dir.join(format!("cover_{}.jpg", ncm_id));

    // Resolve cover
    let cover_path = resolve_cover(
        &client,
        ncm_id,
        song.cover_path.as_deref(),
        &cover_file_path,
    )
    .await;

    // Check if song is already cached
    if song_file_path.exists() {
        tracing::debug!("Song {} found in cache: {:?}", ncm_id, song_file_path);
        return Some(ResolvedSong {
            file_path: song_file_path.to_string_lossy().to_string(),
            cover_path,
            was_cached: true,
        });
    }

    // Download song
    tracing::info!("Downloading song {} from NCM", ncm_id);
    match client.songs_url(&[ncm_id]).await {
        Ok(urls) => {
            if let Some(song_url) = urls.first() {
                if song_url.url.is_empty() {
                    tracing::error!("Empty URL returned for song {}", ncm_id);
                    return None;
                }

                tracing::debug!("Got song URL: {}", song_url.url);
                if client
                    .client
                    .download_file(&song_url.url, song_file_path.clone())
                    .await
                    .is_ok()
                {
                    return Some(ResolvedSong {
                        file_path: song_file_path.to_string_lossy().to_string(),
                        cover_path,
                        was_cached: false,
                    });
                } else {
                    tracing::error!("Failed to download song {}", ncm_id);
                }
            } else {
                tracing::error!("No URL returned for song {}", ncm_id);
            }
        }
        Err(e) => {
            tracing::error!("Failed to get song URL for {}: {}", ncm_id, e);
        }
    }

    None
}

/// Resolve cover image - download if not cached or return existing local path
/// Uses download_cover which handles format detection automatically
async fn resolve_cover(
    client: &NcmClient,
    ncm_id: u64,
    pic_url: Option<&str>,
    _cover_file_path: &PathBuf,
) -> Option<String> {
    // First, check if cover already exists in local cache
    let cover_cache_dir = crate::utils::covers_cache_dir();
    let stem = format!("cover_{}", ncm_id);
    for ext in &["jpg", "png", "gif", "webp", "bmp"] {
        let existing_path = cover_cache_dir.join(format!("{}.{}", stem, ext));
        if existing_path.exists() {
            return Some(existing_path.to_string_lossy().to_string());
        }
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

/// Batch resolve covers for multiple songs
/// Returns a list of (song_id, cover_path) pairs
pub async fn resolve_covers_batch(
    client: Arc<NcmClient>,
    songs: &[DbSong],
    limit: usize,
) -> Vec<(i64, String)> {
    let cover_cache_dir = crate::utils::covers_cache_dir();
    std::fs::create_dir_all(&cover_cache_dir).ok();

    let mut results = Vec::new();

    for song in songs.iter().take(limit) {
        let ncm_id = get_ncm_id(song);
        let cover_file_path = cover_cache_dir.join(format!("cover_{}.jpg", ncm_id));

        // Skip if already cached
        if cover_file_path.exists() {
            results.push((song.id, cover_file_path.to_string_lossy().to_string()));
            continue;
        }

        // Get pic_url from cover_path (which might be a URL for NCM songs)
        let pic_url = song.cover_path.as_deref();

        if let Some(path) = resolve_cover(&client, ncm_id, pic_url, &cover_file_path).await {
            results.push((song.id, path));
        }
    }

    results
}
