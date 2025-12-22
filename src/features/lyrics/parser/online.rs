//! Online lyrics fetching module
//!
//! Fetches lyrics from NetEase Cloud Music API and caches them locally.

use anyhow::Result;
use std::path::PathBuf;

use super::{LyricLineOwned, LyricsFormat, merge_translation, parse_lyrics_with_format};
use crate::api::NcmClient;

/// Lyrics cache directory
fn lyrics_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rustle")
        .join("lyrics")
}

/// Get cached lyrics file path for a song
fn get_cache_path(ncm_id: u64, suffix: &str) -> PathBuf {
    let cache_dir = lyrics_cache_dir();
    cache_dir.join(format!("{}{}", ncm_id, suffix))
}

/// Check if lyrics are cached
pub fn is_lyrics_cached(ncm_id: u64) -> bool {
    // Check for any cached format
    get_cache_path(ncm_id, ".yrc").exists() || get_cache_path(ncm_id, ".lrc").exists()
}

/// Load cached lyrics
pub fn load_cached_lyrics(ncm_id: u64) -> Option<Vec<LyricLineOwned>> {
    // Try YRC first (word-level)
    let yrc_path = get_cache_path(ncm_id, ".yrc");
    if yrc_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&yrc_path) {
            let mut lines = parse_lyrics_with_format(&content, LyricsFormat::Yrc);
            if !lines.is_empty() {
                // Try to load translation
                let tlrc_path = get_cache_path(ncm_id, ".tlrc");
                if tlrc_path.exists() {
                    if let Ok(trans_content) = std::fs::read_to_string(&tlrc_path) {
                        let trans_lines =
                            parse_lyrics_with_format(&trans_content, LyricsFormat::Lrc);
                        merge_translation(&mut lines, &trans_lines);
                    }
                }
                return Some(lines);
            }
        }
    }

    // Fall back to LRC
    let lrc_path = get_cache_path(ncm_id, ".lrc");
    if lrc_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&lrc_path) {
            let mut lines = parse_lyrics_with_format(&content, LyricsFormat::Lrc);
            if !lines.is_empty() {
                // Try to load translation
                let tlrc_path = get_cache_path(ncm_id, ".tlrc");
                if tlrc_path.exists() {
                    if let Ok(trans_content) = std::fs::read_to_string(&tlrc_path) {
                        let trans_lines =
                            parse_lyrics_with_format(&trans_content, LyricsFormat::Lrc);
                        merge_translation(&mut lines, &trans_lines);
                    }
                }
                return Some(lines);
            }
        }
    }

    None
}

/// Fetch lyrics from NCM API and cache them
pub async fn fetch_and_cache_lyrics(
    client: &NcmClient,
    ncm_id: u64,
    song_name: &str,
    singer: &str,
    album: &str,
) -> Result<Vec<LyricLineOwned>> {
    // Create cache directory
    let cache_dir = lyrics_cache_dir();
    std::fs::create_dir_all(&cache_dir)?;

    // Fetch lyrics from API
    let song_info = crate::api::SongInfo {
        id: ncm_id,
        name: song_name.to_string(),
        singer: singer.to_string(),
        album: album.to_string(),
        album_id: 0,
        pic_url: String::new(),
        duration: 0,
        song_url: String::new(),
        copyright: crate::api::SongCopyright::Unknown,
    };

    // Use the existing get_lyrics method which handles caching
    let lyrics_data = client.get_lyrics(&song_info).await?;

    if lyrics_data.is_empty() {
        anyhow::bail!("No lyrics found for song {}", ncm_id);
    }

    // Convert to LyricLineOwned format
    // The get_lyrics method returns Vec<(u64, String)> where u64 is timestamp
    let mut lines: Vec<LyricLineOwned> = Vec::new();
    let mut current_time = 0u64;

    for (time, text) in lyrics_data {
        // Skip if same timestamp (translation line)
        if time == current_time && !lines.is_empty() {
            // This is likely a translation line
            if let Some(last_line) = lines.last_mut() {
                if last_line.translated_lyric.is_empty() {
                    last_line.translated_lyric = text.trim().to_string();
                }
            }
            continue;
        }

        current_time = time;
        lines.push(LyricLineOwned {
            words: vec![super::LyricWordOwned {
                start_time: time,
                end_time: 0,
                word: text.trim().to_string(),
                roman_word: String::new(),
            }],
            start_time: time,
            end_time: 0,
            ..Default::default()
        });
    }

    // Calculate end times
    for i in 0..lines.len() {
        let end_time = if i + 1 < lines.len() {
            lines[i + 1].start_time
        } else {
            lines[i].start_time + 5000 // Default 5 seconds for last line
        };
        lines[i].end_time = end_time;
        if let Some(word) = lines[i].words.first_mut() {
            word.end_time = end_time;
        }
    }

    Ok(lines)
}

/// Fetch YRC (word-level) lyrics from NCM API
pub async fn fetch_yrc_lyrics(
    client: &NcmClient,
    ncm_id: u64,
) -> Result<(Option<String>, Option<String>)> {
    // NCM API for YRC lyrics
    // This requires a different API endpoint that returns YRC format
    // For now, we'll use the standard lyrics API

    let lyrics = client.client.song_lyric(ncm_id).await?;

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

    if let Some(trans) = trans_lyric {
        if !trans.is_empty() {
            let trans_path = get_cache_path(ncm_id, ".tlrc");
            std::fs::write(&trans_path, trans)?;
        }
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
        let trans_lines = parse_lyrics_with_format(trans, LyricsFormat::Lrc);
        merge_translation(&mut lines, &trans_lines);
    }

    // Save to cache
    if let Err(e) = save_lyrics_cache(ncm_id, &main_lyric, trans_lyric.as_deref(), is_yrc) {
        tracing::warn!("Failed to cache lyrics: {}", e);
    }

    Ok(lines)
}
