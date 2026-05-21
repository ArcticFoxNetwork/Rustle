//! Audio preloading for seamless track switching
//!
//! This module uses the new architecture:
//! - QueueNavigator for consistent index calculations (Single Source of Truth)
//! - AudioPreloadManager for state tracking (prevents duplicate requests)
//! - AudioPreloadSlot contains request_id to reference sink in audio thread
//! - Sinks are created and stored in the audio thread via AudioHandle commands

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use iced::Task;

use crate::app::message::Message;
use crate::app::state::App;

use super::audio_preload_manager::{self, PreloadDirection, SlotState};
use super::player_controller::PlaybackSource;
use super::queue_navigator;

impl App {
    fn release_preload_request_ids<I>(&self, request_ids: I)
    where
        I: IntoIterator<Item = u64>,
    {
        self.release_preload_requests(request_ids);
    }

    /// 触发相邻曲目预加载
    /// 歌曲开始播放后调用。使用 coordinator 窗口作为唯一数据源。
    pub fn preload_adjacent_tracks_with_ncm(&mut self) -> Task<Message> {
        let (next_idx, prev_idx) = self.playback.preload_coordinator.adjacent_indices();

        tracing::info!(
            "Preload scan: current_index={:?}, queue_len={}, play_mode={:?}, effective_mode={:?}, next={:?}, prev={:?}",
            self.playback.current_index,
            self.playback.queue.len(),
            self.core.settings.play_mode,
            self.effective_queue_play_mode(),
            next_idx,
            prev_idx
        );

        // Skip preloading if next/prev are the same as current (LoopOne mode)
        let current_idx = self.playback.current_index;
        if next_idx == current_idx && prev_idx == current_idx {
            tracing::debug!("Preload scan skipped: next and prev are same as current");
            return Task::none();
        }
        let stale_request_ids = self
            .playback
            .audio_preload_manager
            .invalidate_stale(next_idx, prev_idx);
        self.release_preload_request_ids(stale_request_ids);

        let mut tasks = Vec::new();

        for (candidate_idx, direction) in [
            (next_idx, PreloadDirection::Next),
            (prev_idx, PreloadDirection::Previous),
        ] {
            if let Some(idx) = candidate_idx {
                if let Some(task) = self.preload_track(idx, direction) {
                    tasks.push(task);
                }
            }
        }

        if tasks.is_empty() {
            Task::none()
        } else {
            Task::batch(tasks)
        }
    }

    /// Preload a specific track
    /// Returns None if already preloaded or preloading
    fn preload_track(&mut self, idx: usize, direction: PreloadDirection) -> Option<Task<Message>> {
        let song = self.playback.queue.get(idx)?.clone();

        // Check if we should preload
        if !self
            .playback
            .audio_preload_manager
            .should_preload(idx, direction)
        {
            tracing::debug!(
                "Preload skipped ({}): idx={}, reason=slot_not_eligible",
                direction,
                idx
            );
            return None;
        }

        // Check if it's a local song with existing file
        if let Some(local_path) = queue_navigator::get_local_path(&song) {
            let track_gain = self.resolve_track_gain_for_song(
                &song,
                super::player_controller::TrackGainMode::AnalyzeIfMissing,
            );
            if let Ok(request_id) =
                self.create_preload_sink_for_file(local_path.clone(), track_gain)
            {
                tracing::info!(
                    "Preload request ({}): idx={}, source=local, path={:?}, request_id={}",
                    direction,
                    idx,
                    local_path,
                    request_id
                );

                let released_request_id = self
                    .playback
                    .audio_preload_manager
                    .mark_pending(idx, direction);
                if let Some(slot) = self.playback.audio_preload_manager.slot_mut(direction) {
                    slot.set_pending_request_id(request_id);
                }
                self.release_preload_request_ids(released_request_id);
                return Some(Task::none());
            }
            tracing::warn!(
                "Preload request failed ({}): idx={}, source=local, path={:?}",
                direction,
                idx,
                local_path
            );
            return None;
        }

        // NCM song - needs download
        if !queue_navigator::needs_ncm_download(&song) {
            tracing::debug!(
                "Preload skipped ({}): idx={}, reason=no_download_needed, file_path={}",
                direction,
                idx,
                song.file_path
            );
            return None;
        }

        // Mark as pending and create download task
        let released_request_id = self
            .playback
            .audio_preload_manager
            .mark_pending(idx, direction);
        self.release_preload_request_ids(released_request_id);

        // Create async download task
        if let Some(client) = &self.core.ncm_client {
            let client = Arc::new(client.clone());
            tracing::info!(
                "Preload request ({}): idx={}, source=streaming-download, song_id={}, title={}",
                direction,
                idx,
                song.id,
                song.title
            );
            Some(audio_preload_manager::create_preload_task(
                client, idx, song, direction,
            ))
        } else {
            tracing::warn!(
                "Preload skipped ({}): idx={}, reason=no_ncm_client",
                direction,
                idx
            );
            None
        }
    }

    /// Handle preload-related messages
    pub fn handle_preload(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            // Preload ready message
            Message::PreloadReady(idx, file_path, direction) => {
                self.handle_preload_complete(*idx, file_path.clone(), *direction)
            }

            // Preload ready with SharedBuffer for streaming playback
            Message::PreloadBufferReady(idx, file_path, direction, buffer, duration_secs) => self
                .handle_preload_buffer_ready(
                    *idx,
                    file_path.clone(),
                    *direction,
                    buffer.clone(),
                    *duration_secs,
                ),

            Message::PreloadAudioFailed(idx, direction) => {
                tracing::warn!("Preload failed ({}): idx={}", direction, idx);
                self.playback
                    .audio_preload_manager
                    .mark_failed(*idx, *direction);
                Some(Task::none())
            }

            _ => None,
        }
    }

    /// Handle AudioEvent::PreloadReady from audio thread
    pub fn handle_audio_preload_ready(
        &mut self,
        request_id: u64,
        duration: Duration,
        path: PathBuf,
    ) {
        for direction in PreloadDirection::ALL {
            let matches = self
                .playback
                .audio_preload_manager
                .slot(direction)
                .map(|slot| slot.has_pending_request(request_id))
                .unwrap_or(false);

            if !matches {
                continue;
            }

            if let Some(slot) = self.playback.audio_preload_manager.slot_mut(direction) {
                slot.request_id = Some(request_id);
                slot.pending_request_id = None;
                slot.path = path.clone();
                slot.duration = duration;
                slot.state = SlotState::Ready;
                tracing::info!(
                    "Preload ready ({}): request_id={}, path={:?}",
                    direction,
                    request_id,
                    path
                );
            }
            return;
        }

        tracing::debug!(
            "PreloadReady received but no matching pending slot: request_id={} (stale)",
            request_id
        );
        self.release_preload_request(request_id);
    }

    /// Handle successful preload completion
    fn handle_preload_complete(
        &mut self,
        idx: usize,
        file_path: String,
        direction: PreloadDirection,
    ) -> Option<Task<Message>> {
        // Update song info in queue
        if let Some(song) = self.playback.queue.get_mut(idx) {
            song.file_path = file_path.clone();
        }

        // Request preload via audio thread
        let track_gain = self
            .playback
            .queue
            .get(idx)
            .cloned()
            .map(|song| {
                self.resolve_track_gain_for_song(
                    &song,
                    super::player_controller::TrackGainMode::AnalyzeIfMissing,
                )
            })
            .unwrap_or(1.0);
        if let Ok(request_id) =
            self.create_preload_sink_for_file(PathBuf::from(&file_path), track_gain)
        {
            if let Some(slot) = self.playback.audio_preload_manager.slot_mut(direction) {
                slot.set_pending_request_id(request_id);
            }
            tracing::info!(
                "Preload file ready ({}): idx={}, path={}, request_id={}",
                direction,
                idx,
                file_path,
                request_id
            );

            return Some(Task::none());
        }

        tracing::warn!(
            "Preload file handoff failed ({}): idx={}, path={}",
            direction,
            idx,
            file_path
        );
        self.playback
            .audio_preload_manager
            .mark_failed(idx, direction);
        Some(Task::none())
    }

    /// Handle preload ready with SharedBuffer (streaming playback)
    fn handle_preload_buffer_ready(
        &mut self,
        idx: usize,
        file_path: String,
        direction: PreloadDirection,
        buffer: crate::audio::SharedBuffer,
        duration_secs: u64,
    ) -> Option<Task<Message>> {
        // Update song info in queue
        if let Some(song) = self.playback.queue.get_mut(idx) {
            song.file_path = file_path.clone();
        }

        let track_gain = self
            .playback
            .queue
            .get(idx)
            .cloned()
            .map(|song| {
                self.resolve_track_gain_for_song(
                    &song,
                    super::player_controller::TrackGainMode::MetadataOnly,
                )
            })
            .unwrap_or(1.0);
        if let Ok(request_id) = self.create_preload_sink_for_stream(
            buffer.clone(),
            Duration::from_secs(duration_secs),
            track_gain,
        ) {
            if let Some(slot) = self.playback.audio_preload_manager.slot_mut(direction) {
                slot.set_pending_request_id(request_id);
            }
            tracing::info!(
                "NCM streaming track buffer ready at index {}, requesting {} preload: request_id={}",
                idx,
                direction,
                request_id
            );

            if let Some(slot) = self.playback.audio_preload_manager.slot_mut(direction) {
                slot.buffer = Some(buffer);
            }

            return Some(Task::none());
        }

        // No audio handle - mark as failed
        tracing::warn!(
            "Preload buffer handoff failed ({}): idx={}, path={}",
            direction,
            idx,
            file_path
        );
        self.playback
            .audio_preload_manager
            .mark_failed(idx, direction);
        Some(Task::none())
    }

    pub(super) fn take_preloaded_source(
        &mut self,
        idx: usize,
        direction: PreloadDirection,
    ) -> Option<PlaybackSource> {
        if let Some(mut slot) = self
            .playback
            .audio_preload_manager
            .take_ready(idx, direction)
        {
            if let Some(request_id) = slot.take_request_id() {
                let path = slot.path.clone();
                let buffer = slot.take_buffer();
                tracing::info!(
                    "Using {} preloaded track: idx={}, request_id={}, path={:?}, streaming_buffer={}",
                    direction,
                    idx,
                    request_id,
                    path,
                    buffer.is_some()
                );
                return Some(PlaybackSource::Preloaded {
                    request_id,
                    path,
                    buffer,
                });
            }
        }
        tracing::debug!("No {} preloaded track available for idx={}", direction, idx);
        None
    }
}
