//! Local music library import module
//!
//! Handles:
//! - Recursive folder scanning
//! - Metadata extraction with encoding fallback
//! - Cover art caching
//! - File deduplication
//! - .m3u/.pls playlist parsing
//! - Folder watching for auto-import
//! - Smart filename parsing

mod cover;
mod encoding;
mod metadata;
mod progress;
mod scanner;
mod watcher;

pub use cover::{CoverCache, default_cache_dir};
pub use progress::{ScanHandle, ScanProgress, ScanState, progress_channel};
pub use scanner::{ScanConfig, scan_and_import};
pub use watcher::{FolderWatcher, WatchEvent};

use std::path::PathBuf;

/// Supported audio file extensions
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "m4a", "ogg", "opus", "aac", "wma", "aiff",
];

/// Check if a file extension is a supported audio format
pub fn is_audio_file(path: &PathBuf) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}
