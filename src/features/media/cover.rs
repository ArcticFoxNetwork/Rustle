//! Cover art discovery and extraction
//!
//! Finds cover art from embedded metadata or external files.

use anyhow::{Context, Result};
use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use std::fs;
use std::path::{Path, PathBuf};

/// Source of cover art
#[derive(Debug, Clone)]
pub enum CoverArtSource {
    /// Embedded in audio file (data, mime_type)
    Embedded(Vec<u8>, String),
    /// External file path
    External(PathBuf),
}

impl CoverArtSource {
    /// Get the cover art as bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        match self {
            CoverArtSource::Embedded(data, _) => Ok(data.clone()),
            CoverArtSource::External(path) => {
                fs::read(path).context("Failed to read cover art file")
            }
        }
    }

    /// Get the MIME type
    pub fn mime_type(&self) -> &str {
        match self {
            CoverArtSource::Embedded(_, mime) => mime,
            CoverArtSource::External(path) => match path.extension().and_then(|e| e.to_str()) {
                Some("jpg") | Some("jpeg") => "image/jpeg",
                Some("png") => "image/png",
                Some("gif") => "image/gif",
                Some("webp") => "image/webp",
                Some("bmp") => "image/bmp",
                _ => "image/jpeg",
            },
        }
    }

    /// Get the file path (for external) or None (for embedded)
    pub fn path(&self) -> Option<&Path> {
        match self {
            CoverArtSource::Embedded(_, _) => None,
            CoverArtSource::External(path) => Some(path),
        }
    }
}

/// Common cover art filenames to search for (in priority order)
const COVER_FILENAMES: &[&str] = &[
    "cover",
    "folder",
    "front",
    "albumart",
    "album",
    "artwork",
    "art",
    "thumb",
    "thumbnail",
];

/// Supported image extensions
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "bmp"];

/// Find cover art for an audio file
///
/// Priority:
/// 1. Embedded in audio file
/// 2. Same-name image file
/// 3. Common cover filenames in same directory
pub fn find_cover_art(audio_path: &Path) -> Option<CoverArtSource> {
    // Priority 1: Check embedded metadata
    if let Some(embedded) = extract_embedded_art(audio_path) {
        return Some(embedded);
    }

    // Priority 2: Check for same-name image file
    if let Some(external) = find_same_name_image(audio_path) {
        return Some(CoverArtSource::External(external));
    }

    // Priority 3: Check for common cover filenames
    if let Some(external) = find_common_cover_file(audio_path) {
        return Some(CoverArtSource::External(external));
    }

    None
}

/// Extract embedded cover art from audio file
fn extract_embedded_art(audio_path: &Path) -> Option<CoverArtSource> {
    let tagged_file = Probe::open(audio_path).ok()?.read().ok()?;

    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())?;

    let picture = tag.pictures().first()?;
    let data = picture.data().to_vec();
    let mime = picture
        .mime_type()
        .map(|m| m.to_string())
        .unwrap_or_else(|| "image/jpeg".to_string());

    Some(CoverArtSource::Embedded(data, mime))
}

/// Find image file with same name as audio file
fn find_same_name_image(audio_path: &Path) -> Option<PathBuf> {
    let parent = audio_path.parent()?;
    let stem = audio_path.file_stem()?.to_str()?;

    for ext in IMAGE_EXTENSIONS {
        let image_path = parent.join(format!("{}.{}", stem, ext));
        if image_path.exists() {
            return Some(image_path);
        }

        // Also check uppercase extension
        let image_path_upper = parent.join(format!("{}.{}", stem, ext.to_uppercase()));
        if image_path_upper.exists() {
            return Some(image_path_upper);
        }
    }

    None
}

/// Find common cover art file in same directory
fn find_common_cover_file(audio_path: &Path) -> Option<PathBuf> {
    let parent = audio_path.parent()?;

    for filename in COVER_FILENAMES {
        for ext in IMAGE_EXTENSIONS {
            // Try lowercase
            let cover_path = parent.join(format!("{}.{}", filename, ext));
            if cover_path.exists() {
                return Some(cover_path);
            }

            // Try capitalized
            let capitalized = format!(
                "{}{}",
                filename.chars().next()?.to_uppercase(),
                &filename[1..]
            );
            let cover_path = parent.join(format!("{}.{}", capitalized, ext));
            if cover_path.exists() {
                return Some(cover_path);
            }

            // Try uppercase
            let cover_path = parent.join(format!(
                "{}.{}",
                filename.to_uppercase(),
                ext.to_uppercase()
            ));
            if cover_path.exists() {
                return Some(cover_path);
            }
        }
    }

    None
}

/// Extract embedded cover art and save to a file
///
/// Returns the path to the saved file, or None if no embedded art
pub fn extract_embedded_cover(audio_path: &Path, output_dir: &Path) -> Result<Option<PathBuf>> {
    let embedded = match extract_embedded_art(audio_path) {
        Some(CoverArtSource::Embedded(data, mime)) => (data, mime),
        _ => return Ok(None),
    };

    let (data, mime) = embedded;

    // Determine extension from MIME type
    let ext = match mime.as_str() {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        _ => "jpg",
    };

    // Create output filename based on audio file name
    let stem = audio_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("cover");

    let output_path = output_dir.join(format!("{}.{}", stem, ext));

    // Ensure output directory exists
    fs::create_dir_all(output_dir).context("Failed to create output directory")?;

    // Write cover art
    fs::write(&output_path, &data).context("Failed to write cover art file")?;

    Ok(Some(output_path))
}

/// 获取歌曲封面路径，必要时提取内嵌封面
///
/// 加载歌曲时使用此函数
/// It will:
/// 1. Return external cover path if found
/// 2. Extract embedded cover to cache dir if found
/// 3. Return None if no cover available
pub fn get_cover_path(audio_path: &Path, cache_dir: &Path) -> Option<PathBuf> {
    match find_cover_art(audio_path) {
        Some(CoverArtSource::External(path)) => Some(path),
        Some(CoverArtSource::Embedded(data, mime)) => {
            // Extract to cache directory
            let ext = match mime.as_str() {
                "image/jpeg" => "jpg",
                "image/png" => "png",
                "image/gif" => "gif",
                "image/webp" => "webp",
                _ => "jpg",
            };

            // Use hash of audio path for unique filename
            let hash = xxhash_rust::xxh3::xxh3_64(audio_path.to_string_lossy().as_bytes());
            let output_path = cache_dir.join(format!("cover_{:016x}.{}", hash, ext));

            // Only write if not already cached
            if !output_path.exists() {
                if let Err(e) = fs::create_dir_all(cache_dir) {
                    tracing::warn!("Failed to create cache dir: {}", e);
                    return None;
                }
                if let Err(e) = fs::write(&output_path, &data) {
                    tracing::warn!("Failed to write cover cache: {}", e);
                    return None;
                }
            }

            Some(output_path)
        }
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cover_filenames_priority() {
        // cover.jpg should be found before folder.jpg
        assert_eq!(COVER_FILENAMES[0], "cover");
        assert_eq!(COVER_FILENAMES[1], "folder");
    }
}
