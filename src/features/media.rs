//! Media file discovery and metadata extraction
//!
//! Handles finding cover art and lyrics from multiple sources:
//! 1. Embedded metadata (ID3/FLAC tags)
//! 2. External files (same-name, cover.jpg, folder.jpg, etc.)
//! 3. Lyrics files (LRC, YRC, QRC, LYS, TTML)

use anyhow::Result;
use std::path::{Path, PathBuf};

use super::lyrics::LyricLineOwned;

pub mod cover;
pub mod lyrics;

pub use cover::CoverArtSource;

/// Find cover art for an audio file
///
/// Priority:
/// 1. Embedded in audio file (ID3 APIC / FLAC METADATA_BLOCK_PICTURE)
/// 2. Same-name image file (song.mp3 -> song.jpg/png)
/// 3. Common cover filenames (cover.jpg, folder.jpg, front.jpg, albumart.jpg)
pub fn find_cover_art(audio_path: &Path) -> Option<CoverArtSource> {
    cover::find_cover_art(audio_path)
}

/// Find lyrics for an audio file
///
/// Supports all formats: LRC, YRC, QRC, LYS, TTML
///
/// Priority:
/// 1. Same-name lyrics file (song.mp3 -> song.lrc/yrc/qrc/lys/ttml)
/// 2. Embedded lyrics in audio file (USLT tag)
pub fn find_lyrics(audio_path: &Path) -> Option<Vec<LyricLineOwned>> {
    lyrics::find_lyrics(audio_path)
}

/// Extract and save embedded cover art to a file
///
/// Returns the path to the saved cover file, or None if no embedded art
pub fn extract_embedded_cover(audio_path: &Path, output_dir: &Path) -> Result<Option<PathBuf>> {
    cover::extract_embedded_cover(audio_path, output_dir)
}
