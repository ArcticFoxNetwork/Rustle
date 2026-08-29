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
use crate::audio::identity::PreloadIdentity;
use crate::audio::streaming::{
    SharedBuffer, StreamingIdentity, estimate_size_from_duration, start_buffer_download,
    wait_for_buffer_playable,
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
    pub request_id: Option<PreloadIdentity>,
    pub pending_request_id: Option<PreloadIdentity>,
    pub duration: Duration,
    pub buffer: Option<SharedBuffer>,
    pub quality: Option<super::song_resolver::ResolvedAudioQuality>,
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
            .field("quality", &self.quality)
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
            quality: None,
        }
    }

    pub fn is_for_index(&self, target_idx: usize) -> bool {
        self.idx == target_idx
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.state, SlotState::Ready) && self.request_id.is_some()
    }

    pub fn has_pending_request(&self, identity: &PreloadIdentity) -> bool {
        self.pending_request_id.as_ref() == Some(identity)
    }

    pub fn set_pending_request_id(&mut self, identity: PreloadIdentity) {
        self.pending_request_id = Some(identity);
    }

    pub fn take_request_id(&mut self) -> Option<PreloadIdentity> {
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

    fn clear_slot(slot: &mut Option<AudioPreloadSlot>) -> Vec<PreloadIdentity> {
        let Some(slot) = slot.take() else {
            return Vec::new();
        };

        if let Some(buffer) = slot.buffer {
            buffer.cancel();
        }

        [slot.request_id, slot.pending_request_id]
            .into_iter()
            .flatten()
            .collect()
    }

    pub fn reset(&mut self) -> Vec<PreloadIdentity> {
        let mut released = Self::clear_slot(&mut self.next);
        released.extend(Self::clear_slot(&mut self.prev));
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

    pub fn is_ready(&self, idx: usize, direction: PreloadDirection) -> bool {
        self.slot_ref(direction)
            .as_ref()
            .is_some_and(|slot| slot.is_for_index(idx) && slot.is_ready())
    }

    pub fn mark_pending(
        &mut self,
        idx: usize,
        direction: PreloadDirection,
    ) -> Vec<PreloadIdentity> {
        let should_replace = self
            .slot_ref(direction)
            .as_ref()
            .is_some_and(|slot| !slot.is_for_index(idx));

        let released = if should_replace {
            Self::clear_slot(self.slot_entry_mut(direction))
        } else {
            Vec::new()
        };

        *self.slot_entry_mut(direction) = Some(AudioPreloadSlot::pending(idx));
        released
    }

    pub fn mark_failed_if_pending(
        &mut self,
        idx: usize,
        direction: PreloadDirection,
        identity: &PreloadIdentity,
    ) -> bool {
        let Some(slot) = self.slot_mut(direction) else {
            return false;
        };
        if slot.idx != idx || slot.pending_request_id.as_ref() != Some(identity) {
            return false;
        }

        let retry_count = slot.retry_count().saturating_add(1);
        slot.pending_request_id = None;
        slot.buffer.take().inspect(|buffer| buffer.cancel());
        slot.state = SlotState::Failed { retry_count };
        true
    }

    pub fn mark_failed_by_identity(&mut self, identity: &PreloadIdentity) -> bool {
        for direction in PreloadDirection::ALL {
            let Some(slot) = self.slot_mut(direction) else {
                continue;
            };
            if slot.pending_request_id.as_ref() != Some(identity) {
                continue;
            }

            let retry_count = slot.retry_count().saturating_add(1);
            slot.pending_request_id = None;
            slot.buffer.take().inspect(|buffer| buffer.cancel());
            slot.state = SlotState::Failed { retry_count };
            return true;
        }
        false
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
    ) -> Vec<PreloadIdentity> {
        let mut released = Vec::new();
        for (direction, expected_idx) in [
            (PreloadDirection::Next, next_idx),
            (PreloadDirection::Previous, prev_idx),
        ] {
            released.extend(self.invalidate_direction(direction, expected_idx));
        }
        released
    }

    fn invalidate_direction(
        &mut self,
        direction: PreloadDirection,
        expected_idx: Option<usize>,
    ) -> Vec<PreloadIdentity> {
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
            Vec::new()
        }
    }

    pub fn has_pending_request(
        &self,
        idx: usize,
        direction: PreloadDirection,
        identity: &PreloadIdentity,
    ) -> bool {
        self.slot(direction)
            .is_some_and(|slot| slot.idx == idx && slot.has_pending_request(identity))
    }

    pub fn replace_pending_request(
        &mut self,
        direction: PreloadDirection,
        parent: &PreloadIdentity,
        handoff: PreloadIdentity,
    ) -> bool {
        let Some(slot) = self.slot_mut(direction) else {
            return false;
        };
        if slot.pending_request_id.as_ref() != Some(parent) {
            return false;
        }
        slot.pending_request_id = Some(handoff);
        true
    }

    pub fn accepts_identity(&self, identity: &PreloadIdentity) -> bool {
        PreloadDirection::ALL.iter().any(|&direction| {
            self.slot(direction).is_some_and(|slot| {
                slot.request_id.as_ref() == Some(identity)
                    || slot.pending_request_id.as_ref() == Some(identity)
            })
        })
    }

    pub fn slot(&self, direction: PreloadDirection) -> Option<&AudioPreloadSlot> {
        self.slot_ref(direction).as_ref()
    }

    pub fn slot_mut(&mut self, direction: PreloadDirection) -> Option<&mut AudioPreloadSlot> {
        self.slot_entry_mut(direction).as_mut()
    }

    #[cfg(test)]
    fn set_slot_for_test(&mut self, direction: PreloadDirection, slot: AudioPreloadSlot) {
        *self.slot_entry_mut(direction) = Some(slot);
    }
}

// ============ Preload Task Creation ============

/// Create a preload task for an NCM song with streaming support
pub fn create_preload_task(
    client: Arc<NcmClient>,
    idx: usize,
    song: DbSong,
    direction: PreloadDirection,
    identity: PreloadIdentity,
) -> Task<Message> {
    Task::perform(
        async move { download_audio_streaming(client, idx, song, direction, identity).await },
        |result| result,
    )
}

async fn download_audio_streaming(
    client: Arc<NcmClient>,
    idx: usize,
    song: DbSong,
    direction: PreloadDirection,
    identity: PreloadIdentity,
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
        return Message::PreloadAudioFailed(idx, direction, identity);
    }

    let requested_level = client.current_quality_level();
    let requested_stem = format!("{}_{}", ncm_id, requested_level.api_level());

    if let Some(cached_path) = crate::utils::find_cached_audio(&song_cache_dir, &requested_stem) {
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
                Some(super::song_resolver::ResolvedAudioQuality {
                    requested: requested_level,
                    actual: requested_level,
                    bitrate: None,
                    size: Some(file_size),
                    format: cached_path
                        .extension()
                        .and_then(|value| value.to_str())
                        .map(ToString::to_string),
                    sample_rate: None,
                    bit_depth: None,
                    channels: None,
                    channel_layout: None,
                    immerse_type: None,
                }),
                identity,
            );
        }
        tracing::info!(
            "Preload: song {} cache incomplete ({} bytes), using streaming buffer",
            ncm_id,
            file_size
        );
        let _ = std::fs::remove_file(&cached_path);
    }

    let url = match client.resolve_track_url(ncm_id, requested_level).await {
        Ok(url) => url,
        Err(e) => {
            tracing::error!("Preload: failed to get song URL for {}: {}", ncm_id, e);
            return Message::PreloadAudioFailed(idx, direction, identity);
        }
    };

    let quality = super::song_resolver::ResolvedAudioQuality::from(&url);
    let actual_stem = format!("{}_{}", ncm_id, url.level.api_level());
    if actual_stem != requested_stem
        && let Some(cached_path) = crate::utils::find_cached_audio(&song_cache_dir, &actual_stem)
    {
        let file_size = std::fs::metadata(&cached_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        let expected_min_size = estimate_size_from_duration(song.duration_secs as u64);
        if file_size > 0 && file_size >= expected_min_size * 8 / 10 {
            return Message::PreloadReady(
                idx,
                cached_path.to_string_lossy().to_string(),
                direction,
                Some(quality),
                identity,
            );
        }
        let _ = std::fs::remove_file(cached_path);
    }
    let cache_path = song_cache_dir.join(actual_stem);
    let song_url = url.url;

    let streaming_identity = StreamingIdentity::Preload(identity.clone());
    let shared_buffer = start_buffer_download(song_url, cache_path, streaming_identity, None);

    // Buffer health, not a short-lived event receiver, remains authoritative
    // after the first startup watermark. The audio thread rechecks the same
    // buffer before promotion and rejects Ready-then-failed preloads.
    if wait_for_buffer_playable(&shared_buffer, 30).await {
        tracing::info!(
            "Preload: returning SharedBuffer for song {} (downloaded: {} bytes)",
            ncm_id,
            shared_buffer.downloaded()
        );
        Message::PreloadBufferReady(
            idx,
            None,
            direction,
            shared_buffer,
            song.duration_secs as u64,
            Some(quality),
            identity,
        )
    } else {
        tracing::error!("Preload: download failed for song {}", ncm_id);
        shared_buffer.cancel();
        Message::PreloadAudioFailed(idx, direction, identity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::identity::PlaybackGenerationController;

    fn identities() -> (
        PreloadIdentity,
        PreloadIdentity,
        PreloadIdentity,
        PreloadIdentity,
    ) {
        let controller = PlaybackGenerationController::new();
        controller.activate_generation();
        (
            controller.reserve_preload_identity().unwrap(),
            controller.reserve_preload_identity().unwrap(),
            controller.reserve_preload_identity().unwrap(),
            controller.reserve_preload_identity().unwrap(),
        )
    }

    #[test]
    fn reset_releases_ready_and_pending_identities_from_both_directions() {
        let (next_ready, next_pending, prev_ready, prev_pending) = identities();
        let mut manager = AudioPreloadManager::default();

        let mut next = AudioPreloadSlot::pending(1);
        next.state = SlotState::Ready;
        next.request_id = Some(next_ready.clone());
        next.pending_request_id = Some(next_pending.clone());
        let mut prev = AudioPreloadSlot::pending(2);
        prev.state = SlotState::Ready;
        prev.request_id = Some(prev_ready.clone());
        prev.pending_request_id = Some(prev_pending.clone());
        manager.set_slot_for_test(PreloadDirection::Next, next);
        manager.set_slot_for_test(PreloadDirection::Previous, prev);

        let released = manager.reset();

        assert_eq!(released.len(), 4);
        assert!(released.contains(&next_ready));
        assert!(released.contains(&next_pending));
        assert!(released.contains(&prev_ready));
        assert!(released.contains(&prev_pending));
        assert!(manager.slot(PreloadDirection::Next).is_none());
        assert!(manager.slot(PreloadDirection::Previous).is_none());
    }

    #[test]
    fn stale_failure_does_not_modify_newer_pending_identity() {
        let (stale, current, _, _) = identities();
        let mut manager = AudioPreloadManager::default();
        let mut slot = AudioPreloadSlot::pending(7);
        slot.pending_request_id = Some(current.clone());
        manager.set_slot_for_test(PreloadDirection::Next, slot);

        assert!(!manager.mark_failed_by_identity(&stale));
        assert!(manager.has_pending_request(7, PreloadDirection::Next, &current));
        assert_eq!(
            manager.slot(PreloadDirection::Next).unwrap().state,
            SlotState::Pending
        );
    }

    #[test]
    fn exact_failure_transitions_pending_slot_and_clears_identity() {
        let (identity, _, _, _) = identities();
        let mut manager = AudioPreloadManager::default();
        let mut slot = AudioPreloadSlot::pending(9);
        slot.pending_request_id = Some(identity.clone());
        manager.set_slot_for_test(PreloadDirection::Previous, slot);

        assert!(manager.mark_failed_if_pending(9, PreloadDirection::Previous, &identity));
        let slot = manager.slot(PreloadDirection::Previous).unwrap();
        assert_eq!(slot.state, SlotState::Failed { retry_count: 1 });
        assert!(slot.pending_request_id.is_none());
    }

    #[test]
    fn replacement_requires_exact_parent_identity() {
        let (parent, other, handoff, _) = identities();
        let mut manager = AudioPreloadManager::default();
        let mut slot = AudioPreloadSlot::pending(3);
        slot.pending_request_id = Some(parent.clone());
        manager.set_slot_for_test(PreloadDirection::Next, slot);

        assert!(!manager.replace_pending_request(PreloadDirection::Next, &other, handoff.clone()));
        assert!(manager.has_pending_request(3, PreloadDirection::Next, &parent));
        assert!(manager.replace_pending_request(PreloadDirection::Next, &parent, handoff.clone()));
        assert!(manager.has_pending_request(3, PreloadDirection::Next, &handoff));
    }
}
