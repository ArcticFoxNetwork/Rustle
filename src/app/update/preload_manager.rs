//! Preload state machine - manages track preloading with proper state tracking
//!
//! This module provides:
//! - State tracking for preload operations (prevents duplicate requests)
//! - Pre-decoded Sink storage for seamless playback
//! - Streaming download support for NCM songs
//! - Retry logic for failed downloads
//!
//! ## Architecture
//! PreloadManager is the SINGLE SOURCE OF TRUTH for all preload state.
//! AudioPlayer no longer tracks preload paths - it only handles playback.
//! PreloadSlot contains pre-decoded Sink for zero-delay track switching.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use iced::Task;
use rodio::Sink;

use crate::api::NcmClient;
use crate::app::message::Message;
use crate::audio::streaming::{
    SharedBuffer, start_buffer_download, wait_for_playable, estimate_size_from_duration,
};
use crate::database::DbSong;

/// Maximum retry attempts for failed downloads
const MAX_RETRIES: u8 = 2;

// ============ Core Types ============

/// State of a preload slot
#[derive(Debug, Clone, Default)]
pub enum SlotState {
    /// No preload in progress
    #[default]
    Idle,
    /// Download/preparation in progress
    Pending,
    /// Ready for playback (Sink created)
    Ready,
    /// Failed with retry count
    Failed { retry_count: u8 },
}

/// A preload slot containing all state for a preloaded track
/// 
/// Key design: Contains pre-decoded Sink for zero-delay playback.
/// When switching tracks, we just call sink.play() instead of re-decoding.
pub struct PreloadSlot {
    /// Queue index of the preloaded track
    pub idx: usize,
    /// Local file path (for reference)
    pub path: PathBuf,
    /// Current state
    pub state: SlotState,
    /// Pre-decoded Sink (paused, ready to play)
    pub sink: Option<Sink>,
    /// Track duration
    pub duration: Duration,
    /// Streaming buffer for NCM songs (continues downloading in background)
    pub buffer: Option<SharedBuffer>,
}

impl std::fmt::Debug for PreloadSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreloadSlot")
            .field("idx", &self.idx)
            .field("path", &self.path)
            .field("state", &self.state)
            .field("has_sink", &self.sink.is_some())
            .field("duration", &self.duration)
            .field("has_buffer", &self.buffer.is_some())
            .finish()
    }
}

impl PreloadSlot {
    /// Create a pending slot (download in progress)
    pub fn pending(idx: usize) -> Self {
        Self {
            idx,
            path: PathBuf::new(),
            state: SlotState::Pending,
            sink: None,
            duration: Duration::ZERO,
            buffer: None,
        }
    }

    /// Create a slot for local song (with pre-decoded Sink)
    pub fn from_local(idx: usize, path: PathBuf, sink: Sink, duration: Duration) -> Self {
        Self {
            idx,
            path,
            state: SlotState::Ready,
            sink: Some(sink),
            duration,
            buffer: None,
        }
    }

    /// Create a slot for NCM streaming song (with pre-decoded Sink and buffer)
    #[allow(dead_code)]
    pub fn from_streaming(
        idx: usize,
        path: PathBuf,
        sink: Sink,
        duration: Duration,
        buffer: SharedBuffer,
    ) -> Self {
        Self {
            idx,
            path,
            state: SlotState::Ready,
            sink: Some(sink),
            duration,
            buffer: Some(buffer),
        }
    }

    /// Create a failed slot
    pub fn failed(idx: usize, retry_count: u8) -> Self {
        Self {
            idx,
            path: PathBuf::new(),
            state: SlotState::Failed { retry_count },
            sink: None,
            duration: Duration::ZERO,
            buffer: None,
        }
    }

    /// Check if this slot is for a specific index
    pub fn is_for_index(&self, target_idx: usize) -> bool {
        self.idx == target_idx
    }

    /// Check if ready for playback
    pub fn is_ready(&self) -> bool {
        matches!(self.state, SlotState::Ready) && self.sink.is_some()
    }

    /// Check if pending
    #[allow(dead_code)]
    pub fn is_pending(&self) -> bool {
        matches!(self.state, SlotState::Pending)
    }

    /// Take the Sink (consumes it from the slot)
    pub fn take_sink(&mut self) -> Option<Sink> {
        self.sink.take()
    }

    /// Take the buffer (consumes it from the slot)
    pub fn take_buffer(&mut self) -> Option<SharedBuffer> {
        self.buffer.take()
    }

    /// Get retry count if failed
    pub fn retry_count(&self) -> u8 {
        match &self.state {
            SlotState::Failed { retry_count } => *retry_count,
            _ => 0,
        }
    }
}

/// Manages preloading for next and previous tracks
/// This is the SINGLE SOURCE OF TRUTH for preload state.
#[derive(Default)]
pub struct PreloadManager {
    next: Option<PreloadSlot>,
    prev: Option<PreloadSlot>,
}

impl std::fmt::Debug for PreloadManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreloadManager")
            .field("next", &self.next.as_ref().map(|s| (s.idx, &s.state)))
            .field("prev", &self.prev.as_ref().map(|s| (s.idx, &s.state)))
            .finish()
    }
}

impl PreloadManager {
    /// Reset all preload state
    pub fn reset(&mut self) {
        // Cancel any ongoing downloads
        if let Some(slot) = &self.next {
            if let Some(buffer) = &slot.buffer {
                buffer.cancel();
            }
        }
        if let Some(slot) = &self.prev {
            if let Some(buffer) = &slot.buffer {
                buffer.cancel();
            }
        }
        self.next = None;
        self.prev = None;
    }

    /// Check if we should preload for the given index
    pub fn should_preload(&self, idx: usize, is_next: bool) -> bool {
        let slot = if is_next { &self.next } else { &self.prev };
        
        match slot {
            None => true,
            Some(s) if !s.is_for_index(idx) => true,
            Some(s) => match &s.state {
                SlotState::Failed { retry_count } => *retry_count < MAX_RETRIES,
                SlotState::Idle => true,
                _ => false,
            },
        }
    }

    /// Mark as pending (download started)
    pub fn mark_pending(&mut self, idx: usize, is_next: bool) {
        let slot = PreloadSlot::pending(idx);
        if is_next {
            self.next = Some(slot);
        } else {
            self.prev = Some(slot);
        }
    }

    /// Mark as ready with a complete PreloadSlot
    pub fn mark_ready(&mut self, slot: PreloadSlot, is_next: bool) {
        if is_next {
            self.next = Some(slot);
        } else {
            self.prev = Some(slot);
        }
    }

    /// Mark as failed
    pub fn mark_failed(&mut self, idx: usize, is_next: bool) {
        let retry_count = if is_next {
            self.next.as_ref().map(|s| s.retry_count()).unwrap_or(0) + 1
        } else {
            self.prev.as_ref().map(|s| s.retry_count()).unwrap_or(0) + 1
        };
        
        let slot = PreloadSlot::failed(idx, retry_count);
        if is_next {
            self.next = Some(slot);
        } else {
            self.prev = Some(slot);
        }
    }

    /// Take ready preload slot (consumes it)
    /// Returns the full PreloadSlot if ready for the given index
    pub fn take_ready(&mut self, idx: usize, is_next: bool) -> Option<PreloadSlot> {
        let slot_ref = if is_next { &mut self.next } else { &mut self.prev };
        
        match slot_ref {
            Some(slot) if slot.is_for_index(idx) && slot.is_ready() => {
                slot_ref.take()
            }
            _ => None,
        }
    }

    /// Check if preload is ready for the given index (without consuming)
    #[allow(dead_code)]
    pub fn is_ready_for(&self, idx: usize, is_next: bool) -> bool {
        let slot = if is_next { &self.next } else { &self.prev };
        slot.as_ref().map(|s| s.is_for_index(idx) && s.is_ready()).unwrap_or(false)
    }

    /// Invalidate preloads that are no longer relevant
    pub fn invalidate_stale(&mut self, next_idx: Option<usize>, prev_idx: Option<usize>) {
        // Check next slot
        if let Some(expected) = next_idx {
            if let Some(slot) = &self.next {
                if !slot.is_for_index(expected) {
                    if let Some(buffer) = &slot.buffer {
                        buffer.cancel();
                    }
                    self.next = None;
                }
            }
        }

        // Check prev slot
        if let Some(expected) = prev_idx {
            if let Some(slot) = &self.prev {
                if !slot.is_for_index(expected) {
                    if let Some(buffer) = &slot.buffer {
                        buffer.cancel();
                    }
                    self.prev = None;
                }
            }
        }
    }

    /// Get current slot state (for debugging/UI)
    #[allow(dead_code)]
    pub fn get_state(&self, is_next: bool) -> Option<&SlotState> {
        let slot = if is_next { &self.next } else { &self.prev };
        slot.as_ref().map(|s| &s.state)
    }
}

// ============ Preload Task Creation ============

/// Create a preload task for an NCM song with streaming support
/// 
/// This downloads the audio file and returns a message when ready.
/// The Sink is created in the main thread (since Sink is not Send).
pub fn create_preload_task(
    client: Arc<NcmClient>,
    idx: usize,
    song: DbSong,
    is_next: bool,
) -> Task<Message> {
    Task::perform(
        async move { download_audio_streaming(client, idx, song, is_next).await },
        |result| result,
    )
}

/// Download audio with streaming support for preload
/// 
/// For NCM songs, we use SharedBuffer for streaming playback:
/// 1. Get content length via HEAD request (or estimate from duration)
/// 2. Use unified start_buffer_download() function
/// 3. Wait for playable threshold
/// 4. Return PreloadBufferReady when ready
async fn download_audio_streaming(
    client: Arc<NcmClient>,
    idx: usize,
    song: DbSong,
    is_next: bool,
) -> Message {
    let ncm_id = if song.id < 0 {
        (-song.id) as u64
    } else {
        song.id as u64
    };

    tracing::info!("Preload: downloading audio for song {} (streaming buffer)", ncm_id);

    let song_cache_dir = crate::utils::songs_cache_dir();
    if std::fs::create_dir_all(&song_cache_dir).is_err() {
        return Message::PreloadAudioFailed(idx, is_next);
    }

    // Use stem for cache lookup - actual extension determined by format detection
    let song_stem = ncm_id.to_string();

    // Check if already fully cached (with any audio extension)
    if let Some(cached_path) = crate::utils::find_cached_audio(&song_cache_dir, &song_stem) {
        let file_size = std::fs::metadata(&cached_path).map(|m| m.len()).unwrap_or(0);
        let expected_min_size = estimate_size_from_duration(song.duration_secs as u64);
        let is_complete = file_size > 0 && file_size >= expected_min_size * 8 / 10;
        
        if is_complete {
            tracing::debug!("Preload: song {} fully cached ({} bytes)", ncm_id, file_size);
            return Message::PreloadReady(
                idx,
                cached_path.to_string_lossy().to_string(),
                is_next,
            );
        }
        tracing::info!("Preload: song {} cache incomplete ({} bytes), using streaming buffer", ncm_id, file_size);
        // Remove incomplete cache file
        let _ = std::fs::remove_file(&cached_path);
    }

    // Get song URL
    let urls = match client.songs_url(&[ncm_id]).await {
        Ok(urls) => urls,
        Err(e) => {
            tracing::error!("Preload: failed to get song URL for {}: {}", ncm_id, e);
            return Message::PreloadAudioFailed(idx, is_next);
        }
    };

    let song_url = match urls.first() {
        Some(u) if !u.url.is_empty() => u.url.clone(),
        _ => {
            tracing::error!("Preload: no valid URL for song {}", ncm_id);
            return Message::PreloadAudioFailed(idx, is_next);
        }
    };

    // Use stem-based path - actual extension will be determined during download
    let cache_path = song_cache_dir.join(&song_stem);

    // Use unified download function - content_length will be obtained from GET response
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(32);
    let shared_buffer = start_buffer_download(
        song_url,
        cache_path.clone(),
        Some(event_tx),
    );

    // Wait for playable
    if wait_for_playable(&mut event_rx, 30).await {
        tracing::info!("Preload: returning SharedBuffer for song {} (downloaded: {} bytes)", 
            ncm_id, shared_buffer.downloaded());
        Message::PreloadBufferReady(
            idx,
            cache_path.to_string_lossy().to_string(),
            is_next,
            shared_buffer,
            song.duration_secs as u64,
        )
    } else {
        tracing::error!("Preload: download failed for song {}", ncm_id);
        Message::PreloadAudioFailed(idx, is_next)
    }
}

// download_audio_file_based removed - now using unified start_buffer_download with estimate_size_from_duration
