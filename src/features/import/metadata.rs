//! Audio metadata extraction with encoding fallback
//!
//! Uses lofty for metadata reading, with custom encoding handling
//! for legacy files that use GBK/Shift-JIS/etc.

use anyhow::{Context, Result};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::Accessor;
use std::path::Path;

use super::encoding::{decode_string, normalize_string};

/// Extracted metadata from an audio file
#[derive(Debug, Clone)]
pub struct AudioMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: i64,
    pub track_number: Option<i64>,
    pub year: Option<i64>,
    pub genre: Option<String>,
    pub format: String,
    /// Raw cover art data (if present)
    pub cover_data: Option<Vec<u8>>,
    /// Cover art MIME type
    pub cover_mime: Option<String>,
}

impl Default for AudioMetadata {
    fn default() -> Self {
        Self {
            title: "Unknown Title".to_string(),
            artist: "Unknown Artist".to_string(),
            album: "Unknown Album".to_string(),
            duration_secs: 0,
            track_number: None,
            year: None,
            genre: None,
            format: "unknown".to_string(),
            cover_data: None,
            cover_mime: None,
        }
    }
}

/// Extract metadata from an audio file
pub fn extract_metadata(path: &Path) -> Result<AudioMetadata> {
    let tagged_file = Probe::open(path)
        .context("Failed to open audio file")?
        .read()
        .context("Failed to read audio file")?;

    let properties = tagged_file.properties();
    let duration = properties.duration();

    // Determine format from file extension
    let format = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_else(|| "unknown".to_string());

    // Try to get the primary tag, or any available tag
    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());

    let mut metadata = AudioMetadata {
        duration_secs: duration.as_secs() as i64,
        format,
        ..Default::default()
    };

    if let Some(tag) = tag {
        // Extract title with encoding fallback
        if let Some(title) = tag.title() {
            metadata.title = normalize_string(&decode_string(title.as_bytes()));
        }

        // Extract artist with encoding fallback
        if let Some(artist) = tag.artist() {
            metadata.artist = normalize_string(&decode_string(artist.as_bytes()));
        }

        // Extract album with encoding fallback
        if let Some(album) = tag.album() {
            metadata.album = normalize_string(&decode_string(album.as_bytes()));
        }

        // Track number
        metadata.track_number = tag.track().map(|t| t as i64);

        // Year
        metadata.year = tag.year().map(|y| y as i64);

        // Genre with encoding fallback
        if let Some(genre) = tag.genre() {
            metadata.genre = Some(normalize_string(&decode_string(genre.as_bytes())));
        }

        // Extract cover art
        if let Some(picture) = tag.pictures().first() {
            metadata.cover_data = Some(picture.data().to_vec());
            metadata.cover_mime = Some(
                picture
                    .mime_type()
                    .map(|m| m.to_string())
                    .unwrap_or_else(|| "image/jpeg".to_string()),
            );
        }
    }

    // If title is still unknown, use filename
    if metadata.title == "Unknown Title" {
        metadata.title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Unknown Title".to_string());
    }

    Ok(metadata)
}

/// Try to parse artist and title from filename
///
/// Common patterns:
/// - "Artist - Title.mp3"
/// - "Artist_-_Title.mp3"
/// - "01 - Artist - Title.mp3"
/// - "01. Title.mp3"
/// - "Title.mp3"
pub fn parse_filename(filename: &str) -> (Option<String>, Option<String>) {
    // Remove extension
    let name = filename
        .rsplit_once('.')
        .map(|(name, _)| name)
        .unwrap_or(filename);

    // Try "Artist - Title" pattern (most common)
    if let Some((artist, title)) = name.split_once(" - ") {
        // Check if artist part is just a track number
        let artist_trimmed = artist.trim();
        if artist_trimmed.parse::<u32>().is_ok()
            || artist_trimmed
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.')
        {
            // It's a track number, so the rest is the title
            return (None, Some(normalize_string(title)));
        }
        return (
            Some(normalize_string(artist_trimmed)),
            Some(normalize_string(title)),
        );
    }

    // Try "Artist_-_Title" pattern
    if let Some((artist, title)) = name.split_once("_-_") {
        return (
            Some(normalize_string(artist)),
            Some(normalize_string(title)),
        );
    }

    // Try "01. Title" or "01 Title" pattern
    let name_trimmed = name.trim();
    if name_trimmed.len() > 3 {
        let first_chars: String = name_trimmed.chars().take(3).collect();
        if first_chars.chars().take(2).all(|c| c.is_ascii_digit()) {
            let rest = &name_trimmed[2..].trim_start_matches(['.', ' ', '_']);
            if !rest.is_empty() {
                return (None, Some(normalize_string(rest)));
            }
        }
    }

    // Just return the filename as title
    (None, Some(normalize_string(name)))
}

/// Apply smart filename parsing to fill in missing metadata
pub fn apply_smart_parsing(metadata: &mut AudioMetadata, filename: &str) {
    let (parsed_artist, parsed_title) = parse_filename(filename);

    // Only apply if metadata is missing
    if metadata.artist == "Unknown Artist" {
        if let Some(artist) = parsed_artist {
            metadata.artist = artist;
        }
    }

    if metadata.title == "Unknown Title" {
        if let Some(title) = parsed_title {
            metadata.title = title;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_filename_artist_title() {
        let (artist, title) = parse_filename("周杰伦 - 七里香.mp3");
        assert_eq!(artist, Some("周杰伦".to_string()));
        assert_eq!(title, Some("七里香".to_string()));
    }

    #[test]
    fn test_parse_filename_track_number() {
        let (artist, title) = parse_filename("01 - 七里香.mp3");
        assert_eq!(artist, None);
        assert_eq!(title, Some("七里香".to_string()));
    }

    #[test]
    fn test_parse_filename_simple() {
        let (artist, title) = parse_filename("七里香.mp3");
        assert_eq!(artist, None);
        assert_eq!(title, Some("七里香".to_string()));
    }

    #[test]
    fn test_parse_filename_numbered() {
        let (artist, title) = parse_filename("01. 七里香.mp3");
        assert_eq!(artist, None);
        assert_eq!(title, Some("七里香".to_string()));
    }
}
