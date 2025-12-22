//! Lyrics discovery for local audio files
//!
//! Finds lyrics from local files (LRC, TTML, etc.) or embedded metadata.
//! Uses the `features::lyrics` module for parsing all supported formats.

use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use std::fs;
use std::path::{Path, PathBuf};

use crate::features::lyrics::{self, LyricLineOwned};

/// Supported lyrics file extensions
const LYRICS_EXTENSIONS: &[&str] = &[
    "lrc",  // Standard LRC
    "yrc",  // NetEase YRC
    "qrc",  // QQ Music QRC
    "lys",  // Lyricify Syllable
    "ttml", // Apple Music TTML
];

/// Find lyrics for an audio file
///
/// Priority:
/// 1. Same-name lyrics file (supports all formats: .lrc, .yrc, .qrc, .lys, .ttml)
/// 2. Embedded lyrics (USLT tag)
pub fn find_lyrics(audio_path: &Path) -> Option<Vec<LyricLineOwned>> {
    // Priority 1: Check for same-name lyrics file (any supported format)
    if let Some(lyrics_path) = find_lyrics_file(audio_path) {
        if let Ok(content) = fs::read_to_string(&lyrics_path) {
            let lines = lyrics::parse_lyrics(&content);
            if !lines.is_empty() {
                tracing::debug!("Loaded {} lyrics lines from {:?}", lines.len(), lyrics_path);
                return Some(lines);
            }
        }
    }

    // Priority 2: Check embedded lyrics
    if let Some(embedded) = extract_embedded_lyrics(audio_path) {
        let lines = lyrics::parse_lyrics(&embedded);
        if !lines.is_empty() {
            tracing::debug!(
                "Loaded {} embedded lyrics lines from {:?}",
                lines.len(),
                audio_path
            );
            return Some(lines);
        }

        // Plain text lyrics without timestamps - create single line
        if !embedded.trim().is_empty() {
            return Some(vec![LyricLineOwned {
                words: vec![lyrics::LyricWordOwned {
                    start_time: 0,
                    end_time: u64::MAX,
                    word: embedded,
                    roman_word: String::new(),
                }],
                start_time: 0,
                end_time: u64::MAX,
                ..Default::default()
            }]);
        }
    }

    None
}

/// Find lyrics file with same name as audio file
/// Searches for all supported extensions
fn find_lyrics_file(audio_path: &Path) -> Option<PathBuf> {
    let parent = audio_path.parent()?;
    let stem = audio_path.file_stem()?.to_str()?;

    for ext in LYRICS_EXTENSIONS {
        // Try lowercase extension
        let path = parent.join(format!("{}.{}", stem, ext));
        if path.exists() {
            return Some(path);
        }

        // Try uppercase extension
        let path = parent.join(format!("{}.{}", stem, ext.to_uppercase()));
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Extract embedded lyrics from audio file
fn extract_embedded_lyrics(audio_path: &Path) -> Option<String> {
    let tagged_file = Probe::open(audio_path).ok()?.read().ok()?;

    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())?;

    // Try USLT (Unsynchronized Lyrics) first
    if let Some(lyrics) = tag.get_string(&ItemKey::Lyrics) {
        if !lyrics.is_empty() {
            return Some(lyrics.to_string());
        }
    }

    None
}

/// Get lyrics file path for a song (if exists)
pub fn get_lyrics_path(audio_path: &Path) -> Option<PathBuf> {
    find_lyrics_file(audio_path)
}

/// Convert LyricLineOwned to the UI LyricLine format
pub fn to_ui_lyric_lines(lines: Vec<LyricLineOwned>) -> Vec<crate::ui::pages::LyricLine> {
    lyrics::to_ui_lyrics(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lyrics_extensions() {
        assert!(LYRICS_EXTENSIONS.contains(&"lrc"));
        assert!(LYRICS_EXTENSIONS.contains(&"yrc"));
        assert!(LYRICS_EXTENSIONS.contains(&"ttml"));
    }
}
