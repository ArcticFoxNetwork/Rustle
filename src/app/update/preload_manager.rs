//! Preload state machine - manages track preloading with proper state tracking
//!
//! This module provides:
//! - State tracking for preload operations (prevents duplicate requests)
//! - Request ID tracking for audio thread preloaded sinks
//! - Streaming download support for NCM songs
//! - Retry logic for failed downloads
//!
//! ## Architecture
//! PreloadManager is the SINGLE SOURCE OF TRUTH for all preload state.
//! Sinks are created and stored in the audio thread
//! PreloadSlot contains request_id to reference the preloaded sink.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use iced::Task;

use crate::api::NcmClient;
use crate::app::message::Message;
use crate::audio::streaming::{
    SharedBuffer, estimate_size_from_duration, start_buffer_download, wait_for_playable,
};
use crate::database::DbSong;

/// Maximum retry attempts for failed downloads
const MAX_RETRIES: u8 = 2;

// ============ Core Types ============

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreloadDirection {
    Next,
    Previous,
}

impl PreloadDirection {
    pub const ALL: [Self; 2] = [Self::Next, Self::Previous];

    pub fn label(self) -> &'static str {
        match self {
            Self::Next => "next",
            Self::Previous => "previous",
        }
    }
}

impl std::fmt::Display for PreloadDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// State of a preload slot
#[derive(Debug, Clone, Default, PartialEq)]
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

/// A preload slot containing state for a preloaded track
///
/// Key design: Contains request_id to reference sink stored in audio thread.
/// When switching tracks, we send PlayPreloaded command with the request_id.
pub struct PreloadSlot {
    /// Queue index of the preloaded track
    pub idx: usize,
    /// Local file path (for reference)
    pub path: PathBuf,
    /// Current state
    pub state: SlotState,
    /// Request ID for the preloaded sink in audio thread
    pub request_id: Option<u64>,
    /// Pending request ID
    pub pending_request_id: Option<u64>,
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
            .field("request_id", &self.request_id)
            .field("pending_request_id", &self.pending_request_id)
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
            request_id: None,
            pending_request_id: None,
            duration: Duration::ZERO,
            buffer: None,
        }
    }

    /// Create a slot for NCM streaming song (with request_id and buffer)
    #[allow(dead_code)]
    pub fn from_streaming(
        idx: usize,
        path: PathBuf,
        request_id: u64,
        duration: Duration,
        buffer: SharedBuffer,
    ) -> Self {
        Self {
            idx,
            path,
            state: SlotState::Ready,
            request_id: Some(request_id),
            pending_request_id: None,
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
            request_id: None,
            pending_request_id: None,
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
        matches!(self.state, SlotState::Ready) && self.request_id.is_some()
    }

    /// Check if pending
    #[allow(dead_code)]
    pub fn is_pending(&self) -> bool {
        matches!(self.state, SlotState::Pending)
    }

    /// Check if this slot has a pending request with the given ID
    pub fn has_pending_request(&self, request_id: u64) -> bool {
        self.pending_request_id == Some(request_id)
    }

    /// Set the pending request ID
    pub fn set_pending_request_id(&mut self, request_id: u64) {
        self.pending_request_id = Some(request_id);
    }

    /// Take the request_id (consumes it from the slot)
    pub fn take_request_id(&mut self) -> Option<u64> {
        self.request_id.take()
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
    fn slot_ref(&self, direction: PreloadDirection) -> &Option<PreloadSlot> {
        match direction {
            PreloadDirection::Next => &self.next,
            PreloadDirection::Previous => &self.prev,
        }
    }

    fn slot_entry_mut(&mut self, direction: PreloadDirection) -> &mut Option<PreloadSlot> {
        match direction {
            PreloadDirection::Next => &mut self.next,
            PreloadDirection::Previous => &mut self.prev,
        }
    }

    fn clear_slot(slot: &mut Option<PreloadSlot>) -> Option<u64> {
        slot.take().and_then(|slot| {
            if let Some(buffer) = slot.buffer {
                buffer.cancel();
            }
            slot.request_id
        })
    }

    /// Reset all preload state
    pub fn reset(&mut self) -> Vec<u64> {
        let mut released = Vec::new();

        if let Some(request_id) = Self::clear_slot(&mut self.next) {
            released.push(request_id);
        }
        if let Some(request_id) = Self::clear_slot(&mut self.prev) {
            released.push(request_id);
        }

        released
    }

    /// Check if we should preload for the given index
    pub fn should_preload(&self, idx: usize, direction: PreloadDirection) -> bool {
        let slot = self.slot_ref(direction);

        match slot {
            None => true,
            Some(s) if !s.is_for_index(idx) => true,
            Some(s) => match &s.state {
                SlotState::Failed { retry_count } => *retry_count < MAX_RETRIES,
                SlotState::Idle => true,
                SlotState::Pending => false,
                SlotState::Ready => false,
            },
        }
    }

    /// Mark as pending (download started)
    pub fn mark_pending(&mut self, idx: usize, direction: PreloadDirection) -> Option<u64> {
        let existing_slot = self.slot_ref(direction);
        let release_request_id = existing_slot
            .as_ref()
            .filter(|slot| !slot.is_for_index(idx))
            .and_then(|slot| slot.request_id);

        if release_request_id.is_some() {
            let _ = Self::clear_slot(self.slot_entry_mut(direction));
        }

        let slot = PreloadSlot::pending(idx);
        *self.slot_entry_mut(direction) = Some(slot);

        release_request_id
    }

    /// Mark as failed
    pub fn mark_failed(&mut self, idx: usize, direction: PreloadDirection) {
        let retry_count = self
            .slot_ref(direction)
            .as_ref()
            .map(|s| s.retry_count())
            .unwrap_or(0)
            + 1;

        let slot = PreloadSlot::failed(idx, retry_count);
        *self.slot_entry_mut(direction) = Some(slot);
    }

    /// Take ready preload slot (consumes it)
    /// Returns the full PreloadSlot if ready for the given index
    pub fn take_ready(&mut self, idx: usize, direction: PreloadDirection) -> Option<PreloadSlot> {
        let slot_ref = self.slot_entry_mut(direction);

        match slot_ref {
            Some(slot) if slot.is_for_index(idx) && slot.is_ready() => slot_ref.take(),
            _ => None,
        }
    }

    /// Check if preload is ready for the given index (without consuming)
    #[allow(dead_code)]
    pub fn is_ready_for(&self, idx: usize, direction: PreloadDirection) -> bool {
        let slot = self.slot_ref(direction);
        slot.as_ref()
            .map(|s| s.is_for_index(idx) && s.is_ready())
            .unwrap_or(false)
    }

    /// Invalidate preloads that are no longer relevant
    pub fn invalidate_stale(
        &mut self,
        next_idx: Option<usize>,
        prev_idx: Option<usize>,
    ) -> Vec<u64> {
        let mut released = Vec::new();

        for (direction, expected_idx) in [
            (PreloadDirection::Next, next_idx),
            (PreloadDirection::Previous, prev_idx),
        ] {
            if let Some(request_id) = self.invalidate_direction(direction, expected_idx) {
                released.push(request_id);
            }
        }

        released
    }

    fn invalidate_direction(
        &mut self,
        direction: PreloadDirection,
        expected_idx: Option<usize>,
    ) -> Option<u64> {
        let should_clear = match expected_idx {
            Some(expected_idx) => self
                .slot(direction)
                .map(|slot| !slot.is_for_index(expected_idx))
                .unwrap_or(false),
            None => self.slot(direction).is_some(),
        };

        if should_clear {
            Self::clear_slot(self.slot_entry_mut(direction))
        } else {
            None
        }
    }

    /// Get current slot state (for debugging/UI)
    #[allow(dead_code)]
    pub fn get_state(&self, direction: PreloadDirection) -> Option<&SlotState> {
        let slot = self.slot_ref(direction);
        slot.as_ref().map(|s| &s.state)
    }

    pub fn slot(&self, direction: PreloadDirection) -> Option<&PreloadSlot> {
        self.slot_ref(direction).as_ref()
    }

    pub fn slot_mut(&mut self, direction: PreloadDirection) -> Option<&mut PreloadSlot> {
        self.slot_entry_mut(direction).as_mut()
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
    direction: PreloadDirection,
) -> Task<Message> {
    Task::perform(
        async move { download_audio_streaming(client, idx, song, direction).await },
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
    direction: PreloadDirection,
) -> Message {
    let ncm_id = if song.id < 0 {
        (-song.id) as u64
    } else {
        song.id as u64
    };

    tracing::info!(
        "Preload: downloading audio for song {} (streaming buffer)",
        ncm_id
    );

    let song_cache_dir = crate::utils::songs_cache_dir();
    if std::fs::create_dir_all(&song_cache_dir).is_err() {
        return Message::PreloadAudioFailed(idx, direction);
    }

    // Use stem for cache lookup - actual extension determined by format detection
    let song_stem = ncm_id.to_string();

    // Check if already fully cached (with any audio extension)
    if let Some(cached_path) = crate::utils::find_cached_audio(&song_cache_dir, &song_stem) {
        let file_size = std::fs::metadata(&cached_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let expected_min_size = estimate_size_from_duration(song.duration_secs as u64);
        let is_complete = file_size > 0 && file_size >= expected_min_size * 8 / 10;

        if is_complete {
            tracing::debug!(
                "Preload: song {} fully cached ({} bytes)",
                ncm_id,
                file_size
            );
            return Message::PreloadReady(
                idx,
                cached_path.to_string_lossy().to_string(),
                direction,
            );
        }
        tracing::info!(
            "Preload: song {} cache incomplete ({} bytes), using streaming buffer",
            ncm_id,
            file_size
        );
        // Remove incomplete cache file
        let _ = std::fs::remove_file(&cached_path);
    }

    // Get song URL
    let urls = match client.songs_url(&[ncm_id]).await {
        Ok(urls) => urls,
        Err(e) => {
            tracing::error!("Preload: failed to get song URL for {}: {}", ncm_id, e);
            return Message::PreloadAudioFailed(idx, direction);
        }
    };

    let song_url = match urls.first() {
        Some(u) if !u.url.is_empty() => u.url.clone(),
        _ => {
            tracing::error!("Preload: no valid URL for song {}", ncm_id);
            return Message::PreloadAudioFailed(idx, direction);
        }
    };

    // Use stem-based path - actual extension will be determined during download
    let cache_path = song_cache_dir.join(&song_stem);

    // Use unified download function - content_length will be obtained from GET response
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(32);
    let shared_buffer = start_buffer_download(song_url, cache_path.clone(), Some(event_tx));

    // Wait for playable
    if wait_for_playable(&mut event_rx, 30).await {
        tracing::info!(
            "Preload: returning SharedBuffer for song {} (downloaded: {} bytes)",
            ncm_id,
            shared_buffer.downloaded()
        );
        Message::PreloadBufferReady(
            idx,
            cache_path.to_string_lossy().to_string(),
            direction,
            shared_buffer,
            song.duration_secs as u64,
        )
    } else {
        tracing::error!("Preload: download failed for song {}", ncm_id);
        Message::PreloadAudioFailed(idx, direction)
    }
}

// download_audio_file_based removed - now using unified start_buffer_download with estimate_size_from_duration
