//! Audio download module
//!
//! Provides functions for downloading songs from NCM, reusing cached files,
//! verifying downloaded file integrity, and writing metadata tags.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use tracing::{info, warn};

use crate::utils::{
    detect_audio_format, download_bytes, find_cached_audio, sanitize_filename, songs_cache_dir,
};

/// Verify downloaded audio file integrity using lofty
pub fn verify_integrity(path: &Path) -> Result<(), String> {
    use lofty::prelude::*;
    use lofty::probe::Probe;

    let tagged_file = Probe::open(path)
        .map_err(|e| format!("Failed to open: {}", e))?
        .read()
        .map_err(|e| format!("Failed to read: {}", e))?;

    let props = tagged_file.properties();
    let duration = props.duration().as_secs();
    if duration == 0 {
        return Err("File has zero duration".to_string());
    }

    info!(
        "Verified audio file: {:?} ({}s)",
        path.file_name().unwrap_or_default(),
        duration
    );
    Ok(())
}

/// Try to reuse a cached audio file by copying it to the download directory
fn reuse_cache(
    ncm_id: u64,
    download_dir: &Path,
    artist: &str,
    title: &str,
) -> Result<Option<PathBuf>, String> {
    let cached = match find_cached_audio(&songs_cache_dir(), &ncm_id.to_string()) {
        Some(p) => p,
        None => return Ok(None),
    };

    let ext = cached.extension().and_then(|e| e.to_str()).unwrap_or("mp3");

    let filename = format!(
        "{} - {}.{}",
        sanitize_filename(artist),
        sanitize_filename(title),
        ext
    );
    let dest = download_dir.join(&filename);

    if dest.exists() {
        info!("Download file already exists: {:?}", dest);
        return Ok(Some(dest));
    }

    fs::copy(&cached, &dest).map_err(|e| format!("Failed to copy cached file: {}", e))?;

    info!(
        "Reused cached audio: {:?} -> {:?}",
        cached.file_name().unwrap_or_default(),
        dest.file_name().unwrap_or_default()
    );
    Ok(Some(dest))
}

/// Download a song from URL, verify it, and write metadata tags.
///
/// `on_progress(downloaded, total)` is called with byte counts during download.
pub async fn download_song(
    ncm_id: u64,
    song_url: &str,
    download_dir: &Path,
    meta: &crate::metadata::SongMetadata,
    on_progress: impl Fn(u64, u64),
) -> Result<PathBuf, String> {
    // 1. Try cache reuse first
    if let Some(path) = reuse_cache(ncm_id, download_dir, &meta.artist, &meta.title)? {
        return Ok(path);
    }

    // 2. Ensure download directory exists
    fs::create_dir_all(download_dir)
        .map_err(|e| format!("Failed to create download dir: {}", e))?;

    // 3. Build filename and paths
    let stem = format!(
        "{} - {}",
        sanitize_filename(&meta.artist),
        sanitize_filename(&meta.title)
    );
    let tmp = download_dir.join(format!("{}.tmp", stem));
    {
        let existing = crate::utils::AUDIO_EXTENSIONS
            .iter()
            .map(|e| download_dir.join(format!("{}.{}", stem, e)))
            .find(|p| p.exists());
        if let Some(p) = existing {
            info!("Download file already exists: {:?}", p);
            return Ok(p);
        }
    }

    // 4. Download audio stream + cover art in parallel
    let client = reqwest::Client::new();
    let response = client
        .get(song_url)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP error: {}", status));
    }

    let total = response.content_length().unwrap_or(0);
    let mut file =
        fs::File::create(&tmp).map_err(|e| format!("Failed to create temp file: {}", e))?;
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download error: {}", e))?;
        std::io::Write::write_all(&mut file, &chunk).map_err(|e| format!("Write error: {}", e))?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, total);
    }

    drop(file);

    if downloaded == 0 {
        let _ = fs::remove_file(&tmp);
        return Err("Downloaded 0 bytes".to_string());
    }

    // 5. Detect format from magic bytes, then rename
    let ext = {
        let mut buf = [0u8; 16];
        let mut f = fs::File::open(&tmp)
            .map_err(|e| format!("Failed to open temp for format detection: {}", e))?;
        let n = f.read(&mut buf).unwrap_or(0);
        detect_audio_format(&buf[..n]).to_string()
    };
    let dest = download_dir.join(format!("{}.{}", stem, ext));
    fs::rename(&tmp, &dest).map_err(|e| format!("Failed to rename temp file: {}", e))?;

    // 6. Verify the final file is playable
    if let Err(e) = verify_integrity(&dest) {
        let _ = fs::remove_file(&dest);
        return Err(format!("Downloaded file is corrupt: {}", e));
    }

    // 7. Download cover art (if URL) and write metadata tags
    let mut edits = meta.to_metadata_edits();
    if let Some(crate::metadata::CoverSource::Url(url)) = &meta.cover {
        if let Some((data, mime)) = download_bytes(url).await {
            edits.cover_data = Some(data);
            edits.cover_mime = Some(mime);
        }
    }
    if let Err(e) = crate::features::import::save_metadata(&dest, &edits) {
        warn!("Failed to write metadata tags to {:?}: {}", dest, e);
    }

    info!(
        "Downloaded: {:?} ({} bytes, {})",
        dest.file_name().unwrap_or_default(),
        downloaded,
        ext
    );
    Ok(dest)
}
