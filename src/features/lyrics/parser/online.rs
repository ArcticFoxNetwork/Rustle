//! Online lyrics fetching module
//!
//! Fetches lyrics from NetEase Cloud Music API and caches them locally.

use anyhow::Result;
use std::path::PathBuf;

use super::{LyricLineOwned, LyricsFormat, merge_translation, parse_lyrics_with_format};
use crate::api::NcmClient;

/// Lyrics cache directory
fn lyrics_cache_dir() -> PathBuf {
    crate::utils::cache_dir().join("lyrics")
}

/// Get cached lyrics file path for a song
fn get_cache_path(ncm_id: u64, suffix: &str) -> PathBuf {
    let cache_dir = lyrics_cache_dir();
    cache_dir.join(format!("{}{}", ncm_id, suffix))
}

/// Load cached lyrics
pub fn load_cached_lyrics(ncm_id: u64) -> Option<Vec<LyricLineOwned>> {
    // Try YRC first (word-level)
    let yrc_path = get_cache_path(ncm_id, ".yrc");
    if yrc_path.exists()
        && let Ok(content) = std::fs::read_to_string(&yrc_path)
    {
        let mut lines = parse_lyrics_with_format(&content, LyricsFormat::Yrc);
        if !lines.is_empty() {
            // Try to load translation
            let tlrc_path = get_cache_path(ncm_id, ".tlrc");
            if tlrc_path.exists()
                && let Ok(trans_content) = std::fs::read_to_string(&tlrc_path)
            {
                let trans_lines = super::parse_lrc_sidecar(&trans_content);
                merge_translation(&mut lines, &trans_lines);
            }
            return Some(lines);
        }
    }

    // Fall back to LRC
    let lrc_path = get_cache_path(ncm_id, ".lrc");
    if lrc_path.exists()
        && let Ok(content) = std::fs::read_to_string(&lrc_path)
    {
        let mut lines = parse_lyrics_with_format(&content, LyricsFormat::Lrc);
        if !lines.is_empty() {
            // Try to load translation
            let tlrc_path = get_cache_path(ncm_id, ".tlrc");
            if tlrc_path.exists()
                && let Ok(trans_content) = std::fs::read_to_string(&tlrc_path)
            {
                let trans_lines = super::parse_lrc_sidecar(&trans_content);
                merge_translation(&mut lines, &trans_lines);
            }
            return Some(lines);
        }
    }

    None
}

/// Fetch YRC (word-level) lyrics from NCM API
pub async fn fetch_yrc_lyrics(
    client: &NcmClient,
    ncm_id: u64,
) -> Result<(Option<String>, Option<String>)> {
    // NCM API for YRC lyrics
    // This requires a different API endpoint that returns YRC format
    // For now, we'll use the standard lyrics API

    let lyrics = client.song_lyric(ncm_id).await?;

    // Check if the lyrics contain YRC format markers
    let main_lyric = if !lyrics.lyric.is_empty() {
        Some(lyrics.lyric.join("\n"))
    } else {
        None
    };

    let trans_lyric = if !lyrics.tlyric.is_empty() {
        Some(lyrics.tlyric.join("\n"))
    } else {
        None
    };

    Ok((main_lyric, trans_lyric))
}

/// Save lyrics to cache
pub fn save_lyrics_cache(
    ncm_id: u64,
    main_lyric: &str,
    trans_lyric: Option<&str>,
    is_yrc: bool,
) -> Result<()> {
    let cache_dir = lyrics_cache_dir();
    std::fs::create_dir_all(&cache_dir)?;

    let suffix = if is_yrc { ".yrc" } else { ".lrc" };
    let main_path = get_cache_path(ncm_id, suffix);
    std::fs::write(&main_path, main_lyric)?;

    if let Some(trans) = trans_lyric
        && !trans.is_empty()
    {
        let trans_path = get_cache_path(ncm_id, ".tlrc");
        std::fs::write(&trans_path, trans)?;
    }

    Ok(())
}

/// Fetch and parse lyrics with automatic format detection
pub async fn fetch_lyrics(client: &NcmClient, ncm_id: u64) -> Result<Vec<LyricLineOwned>> {
    // Check cache first
    if let Some(cached) = load_cached_lyrics(ncm_id) {
        tracing::debug!("Loaded cached lyrics for {}", ncm_id);
        return Ok(cached);
    }

    // Fetch from API
    let (main_lyric, trans_lyric) = fetch_yrc_lyrics(client, ncm_id).await?;

    let main_lyric = main_lyric.ok_or_else(|| anyhow::anyhow!("No lyrics found"))?;

    // Detect format and parse
    let format = super::detect_format(&main_lyric);
    let is_yrc = format == LyricsFormat::Yrc;

    let mut lines = parse_lyrics_with_format(&main_lyric, format);

    // Merge translation if available
    if let Some(trans) = &trans_lyric {
        let trans_lines = super::parse_lrc_sidecar(trans);
        merge_translation(&mut lines, &trans_lines);
    }

    // Save to cache
    if let Err(e) = save_lyrics_cache(ncm_id, &main_lyric, trans_lyric.as_deref(), is_yrc) {
        tracing::warn!("Failed to cache lyrics: {}", e);
    }

    Ok(lines)
}
