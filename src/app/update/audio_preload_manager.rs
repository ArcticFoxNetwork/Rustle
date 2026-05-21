//! Audio preload state machine - manages track preloading with proper state tracking
//!
//! This module provides:
//! - State tracking for preload operations (prevents duplicate requests)
//! - Request ID tracking for audio thread preloaded sinks
//! - Streaming download support for NCM songs
//! - Retry logic for failed downloads
//!
//! ## Architecture
//! AudioPreloadManager is the SINGLE SOURCE OF TRUTH for all audio preload state.
//! Sinks are created and stored in the audio thread.
//! AudioPreloadSlot contains request_id to reference the preloaded sink.

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

/// State of an audio preload slot
#[derive(Debug, Clone, Default, PartialEq)]
pub enum SlotState {
    #[default]
    Idle,
    Pending,
    Ready,
    Failed {
        retry_count: u8,
    },
}

/// An audio preload slot containing state for a preloaded track
///
/// Contains request_id to reference sink stored in audio thread.
/// When switching tracks, we send PlayPreloaded command with the request_id.
pub struct AudioPreloadSlot {
    pub idx: usize,
    pub path: PathBuf,
    pub state: SlotState,
    pub request_id: Option<u64>,
    pub pending_request_id: Option<u64>,
    pub duration: Duration,
    pub buffer: Option<SharedBuffer>,
}

impl std::fmt::Debug for AudioPreloadSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioPreloadSlot")
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

impl AudioPreloadSlot {
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

    pub fn is_for_index(&self, target_idx: usize) -> bool {
        self.idx == target_idx
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.state, SlotState::Ready) && self.request_id.is_some()
    }

    pub fn has_pending_request(&self, request_id: u64) -> bool {
        self.pending_request_id == Some(request_id)
    }

    pub fn set_pending_request_id(&mut self, request_id: u64) {
        self.pending_request_id = Some(request_id);
    }

    pub fn take_request_id(&mut self) -> Option<u64> {
        self.request_id.take()
    }

    pub fn take_buffer(&mut self) -> Option<SharedBuffer> {
        self.buffer.take()
    }

    pub fn retry_count(&self) -> u8 {
        match &self.state {
            SlotState::Failed { retry_count } => *retry_count,
            _ => 0,
        }
    }
}

/// Manages audio preloading for next and previous tracks
#[derive(Default)]
pub struct AudioPreloadManager {
    next: Option<AudioPreloadSlot>,
    prev: Option<AudioPreloadSlot>,
}

impl std::fmt::Debug for AudioPreloadManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioPreloadManager")
            .field("next", &self.next.as_ref().map(|s| (s.idx, &s.state)))
            .field("prev", &self.prev.as_ref().map(|s| (s.idx, &s.state)))
            .finish()
    }
}

impl AudioPreloadManager {
    fn slot_ref(&self, direction: PreloadDirection) -> &Option<AudioPreloadSlot> {
        match direction {
            PreloadDirection::Next => &self.next,
            PreloadDirection::Previous => &self.prev,
        }
    }

    fn slot_entry_mut(&mut self, direction: PreloadDirection) -> &mut Option<AudioPreloadSlot> {
        match direction {
            PreloadDirection::Next => &mut self.next,
            PreloadDirection::Previous => &mut self.prev,
        }
    }

    fn clear_slot(slot: &mut Option<AudioPreloadSlot>) -> Option<u64> {
        slot.take().and_then(|slot| {
            if let Some(buffer) = slot.buffer {
                buffer.cancel();
            }
            slot.request_id
        })
    }

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

    pub fn mark_pending(&mut self, idx: usize, direction: PreloadDirection) -> Option<u64> {
        let existing_slot = self.slot_ref(direction);
        let release_request_id = existing_slot
            .as_ref()
            .filter(|slot| !slot.is_for_index(idx))
            .and_then(|slot| slot.request_id);

        if release_request_id.is_some() {
            let _ = Self::clear_slot(self.slot_entry_mut(direction));
        }

        let slot = AudioPreloadSlot::pending(idx);
        *self.slot_entry_mut(direction) = Some(slot);
        release_request_id
    }

    pub fn mark_failed(&mut self, idx: usize, direction: PreloadDirection) {
        let retry_count = self
            .slot_ref(direction)
            .as_ref()
            .map(|s| s.retry_count())
            .unwrap_or(0)
            + 1;

        let slot = AudioPreloadSlot::failed(idx, retry_count);
        *self.slot_entry_mut(direction) = Some(slot);
    }

    pub fn take_ready(
        &mut self,
        idx: usize,
        direction: PreloadDirection,
    ) -> Option<AudioPreloadSlot> {
        let slot_ref = self.slot_entry_mut(direction);
        match slot_ref {
            Some(slot) if slot.is_for_index(idx) && slot.is_ready() => slot_ref.take(),
            _ => None,
        }
    }

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

    pub fn slot(&self, direction: PreloadDirection) -> Option<&AudioPreloadSlot> {
        self.slot_ref(direction).as_ref()
    }

    pub fn slot_mut(&mut self, direction: PreloadDirection) -> Option<&mut AudioPreloadSlot> {
        self.slot_entry_mut(direction).as_mut()
    }
}

// ============ Preload Task Creation ============

/// Create a preload task for an NCM song with streaming support
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

    let song_stem = ncm_id.to_string();

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
        let _ = std::fs::remove_file(&cached_path);
    }

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

    let cache_path = song_cache_dir.join(&song_stem);

    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(32);
    let shared_buffer = start_buffer_download(song_url, cache_path.clone(), Some(event_tx));

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
