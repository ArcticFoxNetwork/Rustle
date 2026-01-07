//! Unified queue navigation - Single Source of Truth for index calculations
//!
//! This module provides a single, consistent way to calculate next/prev indices
//! across all play modes. All code that needs to determine which song comes next
//! or previous should use this module.

use crate::database::DbSong;
use crate::features::PlayMode;

/// Cached shuffle indices for consistent preloading
/// When in shuffle mode, we pre-calculate the next/prev indices so that
/// preloading and actual playback use the same values.
#[derive(Debug, Clone, Default)]
pub struct ShuffleCache {
    /// Pre-calculated next index for shuffle mode
    pub next: Option<usize>,
    /// Pre-calculated prev index for shuffle mode  
    pub prev: Option<usize>,
}

impl ShuffleCache {
    /// Generate new random indices for shuffle mode
    pub fn regenerate(&mut self, queue_len: usize) {
        if queue_len == 0 {
            self.next = None;
            self.prev = None;
            return;
        }

        use rand::Rng;
        let mut rng = rand::rng();
        self.next = Some(rng.random_range(0..queue_len));
        self.prev = Some(rng.random_range(0..queue_len));

        tracing::debug!(
            "ShuffleCache regenerated: next={:?}, prev={:?}",
            self.next,
            self.prev
        );
    }

    /// Clear the cache (call when queue or play mode changes)
    pub fn clear(&mut self) {
        self.next = None;
        self.prev = None;
    }
}

/// 队列导航器 - 根据播放模式计算 next/prev 索引
///
/// 索引计算的唯一数据源
/// All code paths (playback, preloading, UI) should use this.
pub struct QueueNavigator<'a> {
    queue_len: usize,
    current_idx: usize,
    play_mode: PlayMode,
    shuffle_cache: &'a ShuffleCache,
}

impl<'a> QueueNavigator<'a> {
    /// Create a new navigator
    pub fn new(
        queue_len: usize,
        current_idx: Option<usize>,
        play_mode: PlayMode,
        shuffle_cache: &'a ShuffleCache,
    ) -> Self {
        Self {
            queue_len,
            current_idx: current_idx.unwrap_or(0),
            play_mode,
            shuffle_cache,
        }
    }

    /// Calculate the next track index
    pub fn next_index(&self) -> Option<usize> {
        if self.queue_len == 0 {
            return None;
        }

        match self.play_mode {
            PlayMode::Shuffle => {
                // Use cached index if available, otherwise generate new one
                self.shuffle_cache.next.or_else(|| {
                    use rand::Rng;
                    Some(rand::rng().random_range(0..self.queue_len))
                })
            }
            PlayMode::LoopOne => Some(self.current_idx),
            PlayMode::LoopAll => Some((self.current_idx + 1) % self.queue_len),
            PlayMode::Sequential => {
                let next = self.current_idx + 1;
                if next >= self.queue_len {
                    None
                } else {
                    Some(next)
                }
            }
        }
    }

    /// Calculate the previous track index
    pub fn prev_index(&self) -> Option<usize> {
        if self.queue_len == 0 {
            return None;
        }

        match self.play_mode {
            PlayMode::Shuffle => {
                // Use cached index if available, otherwise generate new one
                self.shuffle_cache.prev.or_else(|| {
                    use rand::Rng;
                    Some(rand::rng().random_range(0..self.queue_len))
                })
            }
            PlayMode::LoopOne => Some(self.current_idx),
            PlayMode::LoopAll => {
                if self.current_idx == 0 {
                    Some(self.queue_len - 1)
                } else {
                    Some(self.current_idx - 1)
                }
            }
            PlayMode::Sequential => {
                if self.current_idx == 0 {
                    None
                } else {
                    Some(self.current_idx - 1)
                }
            }
        }
    }

    /// Get both adjacent indices at once (more efficient for preloading)
    pub fn adjacent_indices(&self) -> AdjacentIndices {
        AdjacentIndices {
            next: self.next_index(),
            prev: self.prev_index(),
        }
    }

    /// Check if LoopOne mode (same song repeats)
    pub fn is_loop_one(&self) -> bool {
        self.play_mode == PlayMode::LoopOne
    }
}

/// Result of adjacent index calculation
#[derive(Debug, Clone, Copy)]
pub struct AdjacentIndices {
    pub next: Option<usize>,
    pub prev: Option<usize>,
}

/// Helper to check if a song needs NCM resolution
pub fn needs_ncm_download(song: &DbSong) -> bool {
    let is_ncm = song.id < 0 || song.file_path.is_empty() || song.file_path.starts_with("ncm://");
    if !is_ncm {
        return false;
    }

    // Check if already cached
    let ncm_id = if song.id < 0 {
        (-song.id) as u64
    } else if song.file_path.starts_with("ncm://") {
        song.file_path
            .trim_start_matches("ncm://")
            .parse()
            .unwrap_or(0)
    } else {
        return false;
    };

    let song_cache_dir = crate::utils::songs_cache_dir();
    let stem = ncm_id.to_string();
    crate::utils::find_cached_audio(&song_cache_dir, &stem).is_none()
}

/// Skip to next playable track, handling failures
/// Returns the next index to try, skipping the failed index
/// IMPORTANT: This always moves to a DIFFERENT song, never returns failed_idx
pub fn skip_to_next_playable(
    queue_len: usize,
    failed_idx: usize,
    _play_mode: PlayMode,
    _shuffle_cache: &ShuffleCache,
) -> Option<usize> {
    if queue_len == 0 {
        return None;
    }

    // If only one song in queue, can't skip to another
    if queue_len == 1 {
        return None;
    }

    // Always skip to next sequential song when a song fails
    // This ensures we don't get stuck on the same failed song
    // regardless of play mode
    let next = (failed_idx + 1) % queue_len;

    // Make sure we're not returning the same index
    if next == failed_idx {
        return None;
    }

    Some(next)
}

/// Helper to get local file path for a song (if available)
pub fn get_local_path(song: &DbSong) -> Option<std::path::PathBuf> {
    // Check if it's an NCM song
    let is_ncm = song.id < 0 || song.file_path.is_empty() || song.file_path.starts_with("ncm://");

    if is_ncm {
        // For NCM songs, check cache with any audio extension
        let ncm_id = if song.id < 0 {
            (-song.id) as u64
        } else if song.file_path.starts_with("ncm://") {
            song.file_path.trim_start_matches("ncm://").parse().ok()?
        } else {
            return None;
        };

        let song_cache_dir = crate::utils::songs_cache_dir();
        let stem = ncm_id.to_string();
        return crate::utils::find_cached_audio(&song_cache_dir, &stem);
    }

    // For local songs, check if file exists
    let path = std::path::PathBuf::from(&song.file_path);
    if path.exists() { Some(path) } else { None }
}
