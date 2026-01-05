//! Utility functions

use iced::Color;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::error;

// ============================================================================
// Image Extensions
// ============================================================================

/// Common image file extensions for cache lookup
pub const IMAGE_EXTENSIONS: &[&str] = &["jpg", "png", "gif", "webp", "bmp"];

/// Find an existing cached image file with any common extension
///
/// # Arguments
/// * `dir` - The directory to search in
/// * `stem` - The filename without extension (e.g., "cover_123")
///
/// # Returns
/// The path to the existing file if found, None otherwise
pub fn find_cached_image(dir: &Path, stem: &str) -> Option<PathBuf> {
    IMAGE_EXTENSIONS
        .iter()
        .map(|ext| dir.join(format!("{}.{}", stem, ext)))
        .find(|p| p.exists())
}

// ============================================================================
// Color Extraction
// ============================================================================

/// Extracted color palette from an image (simple 2-color version)
#[derive(Debug, Clone)]
pub struct ColorPalette {
    /// Primary dominant color
    pub primary: Color,
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self {
            primary: Color::from_rgb(0.4, 0.2, 0.5),
        }
    }
}

impl ColorPalette {
    /// Extract dominant color from an image file
    pub fn from_image_path(path: &Path) -> Self {
        match DominantColors::extract_from_path(path) {
            Some(colors) => {
                tracing::debug!(
                    "Extracted colors from {:?}: primary=({:.2}, {:.2}, {:.2})",
                    path,
                    colors.primary.r,
                    colors.primary.g,
                    colors.primary.b
                );
                Self {
                    primary: colors.primary,
                }
            }
            None => {
                tracing::warn!("Failed to extract colors from {:?}, using default", path);
                Self::default()
            }
        }
    }
}

/// Dominant colors extracted from an image using k-means clustering
#[derive(Debug, Clone, Default)]
pub struct DominantColors {
    /// Primary dominant color
    pub primary: Color,
    /// Secondary dominant color
    pub secondary: Color,
    /// Tertiary dominant color
    pub tertiary: Color,
    /// Average brightness (0.0 = dark, 1.0 = bright)
    pub brightness: f32,
}

impl DominantColors {
    /// Create default dark colors for when no image is available
    pub fn dark_default() -> Self {
        Self {
            primary: Color::from_rgb(0.08, 0.06, 0.12),
            secondary: Color::from_rgb(0.05, 0.05, 0.08),
            tertiary: Color::from_rgb(0.02, 0.02, 0.04),
            brightness: 0.1,
        }
    }

    /// Extract dominant colors from an image file path (string version)
    pub fn from_image_path(path: &str) -> Option<Self> {
        Self::extract_from_path(Path::new(path))
    }

    /// Extract dominant colors from an image file path
    pub fn extract_from_path(path: &Path) -> Option<Self> {
        let img = match image::open(path) {
            Ok(img) => img,
            Err(e) => {
                tracing::warn!("Failed to open image {:?}: {}", path, e);
                return None;
            }
        };
        let img = img.to_rgb8();
        let img = image::imageops::resize(&img, 32, 32, image::imageops::FilterType::Nearest);

        let mut pixels: Vec<(u8, u8, u8)> = Vec::new();
        for pixel in img.pixels() {
            pixels.push((pixel[0], pixel[1], pixel[2]));
        }

        if pixels.is_empty() {
            return None;
        }

        let colors = kmeans_colors(&pixels, 3);

        let brightness = colors
            .iter()
            .map(|(r, g, b)| (*r as f32 * 0.299 + *g as f32 * 0.587 + *b as f32 * 0.114) / 255.0)
            .sum::<f32>()
            / colors.len() as f32;

        let to_background_color =
            |r: u8, g: u8, b: u8, brightness_factor: f32, saturation_boost: f32| -> Color {
                let rf = r as f32 / 255.0;
                let gf = g as f32 / 255.0;
                let bf = b as f32 / 255.0;

                let max = rf.max(gf).max(bf);
                let min = rf.min(gf).min(bf);
                let delta = max - min;

                let (r_out, g_out, b_out) = if delta < 0.01 {
                    (
                        rf * brightness_factor,
                        gf * brightness_factor,
                        bf * brightness_factor,
                    )
                } else {
                    let avg = (rf + gf + bf) / 3.0;
                    let boost = |v: f32| -> f32 {
                        let diff = v - avg;
                        (avg + diff * saturation_boost).clamp(0.0, 1.0) * brightness_factor
                    };
                    (boost(rf), boost(gf), boost(bf))
                };

                Color::from_rgb(r_out, g_out, b_out)
            };

        Some(Self {
            primary: to_background_color(colors[0].0, colors[0].1, colors[0].2, 0.65, 1.6),
            secondary: to_background_color(colors[1].0, colors[1].1, colors[1].2, 0.50, 1.5),
            tertiary: to_background_color(colors[2].0, colors[2].1, colors[2].2, 0.25, 1.3),
            brightness,
        })
    }
}

/// Simple k-means clustering for color extraction
fn kmeans_colors(pixels: &[(u8, u8, u8)], k: usize) -> Vec<(u8, u8, u8)> {
    if pixels.is_empty() || k == 0 {
        return vec![(20, 15, 30); k];
    }

    let mut centroids: Vec<(f32, f32, f32)> = (0..k)
        .map(|i| {
            let idx = i * pixels.len() / k;
            let p = pixels[idx.min(pixels.len() - 1)];
            (p.0 as f32, p.1 as f32, p.2 as f32)
        })
        .collect();

    for _ in 0..10 {
        let mut clusters: Vec<Vec<(u8, u8, u8)>> = vec![Vec::new(); k];

        for &pixel in pixels {
            let mut min_dist = f32::MAX;
            let mut min_idx = 0;

            for (idx, centroid) in centroids.iter().enumerate() {
                let dist = color_distance(pixel, *centroid);
                if dist < min_dist {
                    min_dist = dist;
                    min_idx = idx;
                }
            }

            clusters[min_idx].push(pixel);
        }

        for (idx, cluster) in clusters.iter().enumerate() {
            if !cluster.is_empty() {
                let sum: (u32, u32, u32) = cluster.iter().fold((0, 0, 0), |acc, p| {
                    (acc.0 + p.0 as u32, acc.1 + p.1 as u32, acc.2 + p.2 as u32)
                });
                let len = cluster.len() as f32;
                centroids[idx] = (sum.0 as f32 / len, sum.1 as f32 / len, sum.2 as f32 / len);
            }
        }
    }

    centroids.sort_by(|a, b| {
        let brightness_a = a.0 * 0.299 + a.1 * 0.587 + a.2 * 0.114;
        let brightness_b = b.0 * 0.299 + b.1 * 0.587 + b.2 * 0.114;
        brightness_a
            .partial_cmp(&brightness_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    centroids
        .iter()
        .map(|(r, g, b)| (*r as u8, *g as u8, *b as u8))
        .collect()
}

/// Calculate squared distance between a pixel and a centroid
fn color_distance(pixel: (u8, u8, u8), centroid: (f32, f32, f32)) -> f32 {
    let dr = pixel.0 as f32 - centroid.0;
    let dg = pixel.1 as f32 - centroid.1;
    let db = pixel.2 as f32 - centroid.2;
    dr * dr + dg * dg + db * db
}

// ============================================================================
// Time & Path Utilities
// ============================================================================

/// Format timestamp as relative time (e.g., "2天前")
pub fn format_relative_time(timestamp: i64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let diff_secs = now - timestamp;
    let diff_mins = diff_secs / 60;
    let diff_hours = diff_mins / 60;
    let diff_days = diff_hours / 24;

    if diff_days > 30 {
        let months = diff_days / 30;
        format!("{}个月前", months)
    } else if diff_days > 0 {
        format!("{}天前", diff_days)
    } else if diff_hours > 0 {
        format!("{}小时前", diff_hours)
    } else if diff_mins > 0 {
        format!("{}分钟前", diff_mins)
    } else {
        "刚刚".to_string()
    }
}

/// Get the base cache directory for rustle
pub fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rustle")
}

/// Get the covers cache directory
pub fn covers_cache_dir() -> PathBuf {
    cache_dir().join("covers")
}

/// Get the songs cache directory
pub fn songs_cache_dir() -> PathBuf {
    cache_dir().join("songs")
}

/// Get the banners cache directory
pub fn banners_cache_dir() -> PathBuf {
    cache_dir().join("banners")
}

/// Get the avatars cache directory
pub fn avatars_cache_dir() -> PathBuf {
    cache_dir().join("avatars")
}

/// Detect image format from magic bytes
/// Returns the correct file extension (without dot)
fn detect_image_format(bytes: &[u8]) -> &'static str {
    if bytes.len() < 8 {
        return "jpg"; // Default fallback
    }

    // PNG: 89 50 4E 47 0D 0A 1A 0A
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return "png";
    }

    // JPEG: FF D8 FF
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return "jpg";
    }

    // GIF: 47 49 46 38
    if bytes.starts_with(&[0x47, 0x49, 0x46, 0x38]) {
        return "gif";
    }

    // WebP: 52 49 46 46 ... 57 45 42 50
    if bytes.len() >= 12 && bytes.starts_with(&[0x52, 0x49, 0x46, 0x46]) && &bytes[8..12] == b"WEBP"
    {
        return "webp";
    }

    // BMP: 42 4D
    if bytes.starts_with(&[0x42, 0x4D]) {
        return "bmp";
    }

    "jpg" // Default fallback
}

/// Download an image from URL to local path
/// Returns the local path if successful
///
/// The function detects the actual image format from magic bytes and saves
/// with the correct extension, regardless of what extension was requested.
///
/// # Arguments
/// * `client` - The NCM client for downloading
/// * `url` - The image URL
/// * `base_path` - The base local path (extension will be replaced based on actual format)
/// * `width` - Resize width (for NCM image API)
/// * `height` - Resize height (for NCM image API)
pub async fn download_img(
    client: &crate::api::NcmClient,
    url: &str,
    base_path: PathBuf,
    width: u16,
    height: u16,
) -> Option<PathBuf> {
    // Ensure parent directory exists
    if let Some(parent) = base_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            error!("Failed to create cache directory: {}", e);
            return None;
        }
    }

    // Get the stem (filename without extension)
    let stem = base_path.file_stem()?.to_str()?;
    let parent = base_path.parent()?;

    // Check if file already exists with any common image extension
    if let Some(existing) = find_cached_image(parent, stem) {
        return Some(existing);
    }

    // Download to a temporary path first to detect format
    let temp_path = parent.join(format!("{}.tmp", stem));

    match client
        .client
        .download_img(url, temp_path.clone(), width, height)
        .await
    {
        Ok(_) => {
            // Read the file to detect format
            match std::fs::read(&temp_path) {
                Ok(bytes) => {
                    let ext = detect_image_format(&bytes);
                    let final_path = parent.join(format!("{}.{}", stem, ext));

                    // Rename temp file to final path with correct extension
                    if let Err(e) = std::fs::rename(&temp_path, &final_path) {
                        error!("Failed to rename temp file: {}", e);
                        // Try to clean up temp file
                        let _ = std::fs::remove_file(&temp_path);
                        return None;
                    }

                    Some(final_path)
                }
                Err(e) => {
                    error!("Failed to read downloaded image: {}", e);
                    let _ = std::fs::remove_file(&temp_path);
                    None
                }
            }
        }
        Err(e) => {
            error!("Failed to download image: {}", e);
            let _ = std::fs::remove_file(&temp_path);
            None
        }
    }
}

/// Download a cover image for a song
pub async fn download_cover(
    client: &crate::api::NcmClient,
    song_id: u64,
    pic_url: &str,
) -> Option<PathBuf> {
    if pic_url.is_empty() {
        return None;
    }
    let path = covers_cache_dir().join(format!("cover_{}.jpg", song_id));
    download_img(client, pic_url, path, 200, 200).await
}

/// Download a banner image
pub async fn download_banner(
    client: &crate::api::NcmClient,
    target_id: u64,
    pic_url: &str,
) -> Option<PathBuf> {
    if pic_url.is_empty() {
        return None;
    }
    let path = banners_cache_dir().join(format!("banner_{}.jpg", target_id));
    download_img(client, pic_url, path, 800, 280).await
}

/// Download user avatar
pub async fn download_avatar(
    client: &crate::api::NcmClient,
    user_id: u64,
    avatar_url: &str,
) -> Option<PathBuf> {
    if avatar_url.is_empty() {
        return None;
    }
    let path = avatars_cache_dir().join(format!("avatar_{}.jpg", user_id));
    download_img(client, avatar_url, path, 200, 200).await
}

/// Download playlist cover image
pub async fn download_playlist_cover(
    client: &crate::api::NcmClient,
    playlist_id: u64,
    cover_url: &str,
) -> Option<PathBuf> {
    if cover_url.is_empty() {
        return None;
    }
    let path = covers_cache_dir().join(format!("playlist_{}.jpg", playlist_id));
    download_img(client, cover_url, path, 300, 300).await
}

/// Download playlist creator avatar
pub async fn download_playlist_creator_avatar(
    client: &crate::api::NcmClient,
    playlist_id: u64,
    avatar_url: &str,
) -> Option<PathBuf> {
    if avatar_url.is_empty() {
        return None;
    }
    let path = avatars_cache_dir().join(format!("playlist_creator_{}.jpg", playlist_id));
    download_img(client, avatar_url, path, 100, 100).await
}
