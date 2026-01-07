//! Cache management module
//!
//! Handles cache size calculation, cleanup, and automatic eviction.

use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use tracing::{info, warn};

use crate::utils::{avatars_cache_dir, banners_cache_dir, covers_cache_dir, songs_cache_dir};

/// Information about a cached file
#[derive(Debug)]
struct CacheEntry {
    path: PathBuf,
    size: u64,
    modified: SystemTime,
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Total size in bytes
    pub total_bytes: u64,
    /// Number of files
    pub file_count: usize,
    /// Size by category
    pub covers_bytes: u64,
    pub songs_bytes: u64,
    pub banners_bytes: u64,
    pub avatars_bytes: u64,
}

impl CacheStats {
    /// Get total size in megabytes
    pub fn total_mb(&self) -> u64 {
        self.total_bytes / (1024 * 1024)
    }
}

/// Get all cache directories
fn cache_directories() -> Vec<PathBuf> {
    vec![
        covers_cache_dir(),
        songs_cache_dir(),
        banners_cache_dir(),
        avatars_cache_dir(),
    ]
}

/// Collect all cache entries from a directory
fn collect_entries(dir: &PathBuf) -> Vec<CacheEntry> {
    let mut entries = Vec::new();

    if !dir.exists() {
        return entries;
    }

    let read_dir = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            warn!("Failed to read cache directory {:?}: {}", dir, e);
            return entries;
        }
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        entries.push(CacheEntry {
            path,
            size: metadata.len(),
            modified,
        });
    }

    entries
}

/// Calculate cache statistics
pub fn calculate_cache_stats() -> CacheStats {
    let mut stats = CacheStats::default();

    // Covers
    for entry in collect_entries(&covers_cache_dir()) {
        stats.covers_bytes += entry.size;
        stats.file_count += 1;
    }

    // Songs
    for entry in collect_entries(&songs_cache_dir()) {
        stats.songs_bytes += entry.size;
        stats.file_count += 1;
    }

    // Banners
    for entry in collect_entries(&banners_cache_dir()) {
        stats.banners_bytes += entry.size;
        stats.file_count += 1;
    }

    // Avatars
    for entry in collect_entries(&avatars_cache_dir()) {
        stats.avatars_bytes += entry.size;
        stats.file_count += 1;
    }

    stats.total_bytes =
        stats.covers_bytes + stats.songs_bytes + stats.banners_bytes + stats.avatars_bytes;

    stats
}

/// Clear all cache
pub fn clear_all_cache() -> Result<ClearResult, CacheError> {
    let mut result = ClearResult::default();

    for dir in cache_directories() {
        if !dir.exists() {
            continue;
        }

        let entries = collect_entries(&dir);
        for entry in entries {
            match fs::remove_file(&entry.path) {
                Ok(_) => {
                    result.files_deleted += 1;
                    result.bytes_freed += entry.size;
                }
                Err(e) => {
                    warn!("Failed to delete cache file {:?}: {}", entry.path, e);
                    result.errors += 1;
                }
            }
        }
    }

    info!(
        "Cache cleared: {} files deleted, {} MB freed, {} errors",
        result.files_deleted,
        result.bytes_freed / (1024 * 1024),
        result.errors
    );

    Ok(result)
}

/// Enforce cache size limit by deleting oldest files
///
/// Returns the number of bytes freed
pub fn enforce_cache_limit(max_cache_mb: u64) -> Result<ClearResult, CacheError> {
    let max_bytes = max_cache_mb * 1024 * 1024;
    let mut result = ClearResult::default();

    // Collect all cache entries
    let mut all_entries: Vec<CacheEntry> = Vec::new();
    for dir in cache_directories() {
        all_entries.extend(collect_entries(&dir));
    }

    // Calculate current total size
    let current_size: u64 = all_entries.iter().map(|e| e.size).sum();

    if current_size <= max_bytes {
        info!(
            "Cache size {} MB is within limit {} MB",
            current_size / (1024 * 1024),
            max_cache_mb
        );
        return Ok(result);
    }

    // Sort by modification time (oldest first)
    all_entries.sort_by(|a, b| a.modified.cmp(&b.modified));

    let mut freed: u64 = 0;
    let target_free = current_size - max_bytes;

    // Delete oldest files until we're under the limit
    for entry in all_entries {
        if freed >= target_free {
            break;
        }

        match fs::remove_file(&entry.path) {
            Ok(_) => {
                freed += entry.size;
                result.files_deleted += 1;
                result.bytes_freed += entry.size;
            }
            Err(e) => {
                warn!("Failed to delete cache file {:?}: {}", entry.path, e);
                result.errors += 1;
            }
        }
    }

    info!(
        "Cache cleanup: {} files deleted, {} MB freed (target was {} MB)",
        result.files_deleted,
        result.bytes_freed / (1024 * 1024),
        target_free / (1024 * 1024)
    );

    Ok(result)
}

/// Result of a cache clear operation
#[derive(Debug, Clone, Default)]
pub struct ClearResult {
    pub files_deleted: usize,
    pub bytes_freed: u64,
    pub errors: usize,
}

impl ClearResult {
    /// Get bytes freed in megabytes
    pub fn mb_freed(&self) -> u64 {
        self.bytes_freed / (1024 * 1024)
    }
}

/// Clean up orphan .tmp files from incomplete downloads
///
/// This should be called at application startup to remove any temp files
/// left behind from interrupted downloads.
pub fn cleanup_temp_files() -> ClearResult {
    let mut result = ClearResult::default();

    for dir in cache_directories() {
        if !dir.exists() {
            continue;
        }

        let read_dir = match fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(e) => {
                warn!("Failed to read cache directory {:?}: {}", dir, e);
                continue;
            }
        };

        for entry in read_dir.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            // Check if it's a .tmp file
            if path.extension().map(|e| e == "tmp").unwrap_or(false) {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                match fs::remove_file(&path) {
                    Ok(_) => {
                        info!("Cleaned up orphan temp file: {:?} ({} bytes)", path, size);
                        result.files_deleted += 1;
                        result.bytes_freed += size;
                    }
                    Err(e) => {
                        warn!("Failed to delete temp file {:?}: {}", path, e);
                        result.errors += 1;
                    }
                }
            }
        }
    }

    if result.files_deleted > 0 {
        info!(
            "Temp file cleanup: {} files deleted, {} bytes freed",
            result.files_deleted, result.bytes_freed
        );
    }

    result
}

/// Cache operation errors
#[derive(Debug, Clone)]
pub enum CacheError {
    Io(String),
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheError::Io(e) => write!(f, "Cache IO error: {}", e),
        }
    }
}

impl std::error::Error for CacheError {}
