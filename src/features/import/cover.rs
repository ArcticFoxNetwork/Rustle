//! Cover art caching system
//!
//! Extracts cover art from audio files, generates thumbnails,
//! and caches them to disk for fast access.

use anyhow::{Context, Result};
use image::ImageFormat;
use image::imageops::FilterType;
use std::io::Cursor;
use std::path::PathBuf;
use xxhash_rust::xxh3::xxh3_64;

/// Default thumbnail size (width and height)
pub const THUMBNAIL_SIZE: u32 = 300;

/// Cover cache manager
#[derive(Debug)]
pub struct CoverCache {
    cache_dir: PathBuf,
}

impl CoverCache {
    /// Create a new cover cache with the specified cache directory
    pub fn new(cache_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&cache_dir).context("Failed to create cover cache directory")?;
        Ok(Self { cache_dir })
    }

    /// Generate a hash for cover art data
    fn hash_cover(data: &[u8]) -> String {
        format!("{:016x}", xxh3_64(data))
    }

    /// Get the path where a cover with the given hash would be stored
    fn cover_path(&self, hash: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.jpg", hash))
    }

    /// Save cover art to cache, returning the hash and path
    ///
    /// The cover is resized to a thumbnail and saved as JPEG for consistency
    fn save_cover(&self, data: &[u8]) -> Result<(String, PathBuf)> {
        let hash = Self::hash_cover(data);
        let path = self.cover_path(&hash);

        // Skip if already cached
        if path.exists() {
            return Ok((hash, path));
        }

        // Load and resize image
        let img = image::load_from_memory(data).context("Failed to decode cover image")?;

        // Resize to thumbnail, maintaining aspect ratio
        let thumbnail = img.resize(THUMBNAIL_SIZE, THUMBNAIL_SIZE, FilterType::Lanczos3);

        // Save as JPEG
        let mut output = Vec::new();
        thumbnail
            .write_to(&mut Cursor::new(&mut output), ImageFormat::Jpeg)
            .context("Failed to encode thumbnail")?;

        std::fs::write(&path, &output).context("Failed to write cover to cache")?;

        Ok((hash, path))
    }

    /// Save cover art from raw bytes, with optional MIME type hint
    pub fn save_cover_with_mime(
        &self,
        data: &[u8],
        _mime: Option<&str>,
    ) -> Result<(String, PathBuf)> {
        // image crate auto-detects format, so we ignore MIME hint
        self.save_cover(data)
    }
}

/// Get the default cover cache directory
pub fn default_cache_dir() -> PathBuf {
    crate::utils::covers_cache_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_consistency() {
        let data = b"test cover data";
        let hash1 = CoverCache::hash_cover(data);
        let hash2 = CoverCache::hash_cover(data);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_uniqueness() {
        let data1 = b"test cover data 1";
        let data2 = b"test cover data 2";
        let hash1 = CoverCache::hash_cover(data1);
        let hash2 = CoverCache::hash_cover(data2);
        assert_ne!(hash1, hash2);
    }
}
