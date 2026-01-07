//! Audio preloading for seamless track switching
//!
//! This module uses the new architecture:
//! - QueueNavigator for consistent index calculations (Single Source of Truth)
//! - PreloadManager for state tracking (prevents duplicate requests)
//! - PreloadSlot contains request_id to reference sink in audio thread
//! - Sinks are created and stored in the audio thread via AudioHandle commands

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use iced::Task;

use crate::app::message::Message;
use crate::app::state::App;

use super::preload_manager::{self};
use super::queue_navigator::{self, QueueNavigator};

impl App {
    /// Create a QueueNavigator for the current state
    fn queue_navigator(&self) -> QueueNavigator<'_> {
        QueueNavigator::new(
            self.library.queue.len(),
            self.library.queue_index,
            self.core.settings.play_mode,
            &self.library.shuffle_cache,
        )
    }

    /// 触发相邻曲目预加载
    /// 歌曲开始播放后调用
    pub fn preload_adjacent_tracks_with_ncm(&mut self) -> Task<Message> {
        let nav = self.queue_navigator();
        let adjacent = nav.adjacent_indices();

        // Skip preloading for LoopOne mode (same song)
        if nav.is_loop_one() {
            return Task::none();
        }

        // Invalidate stale preloads
        self.library
            .preload_manager
            .invalidate_stale(adjacent.next, adjacent.prev);

        let mut tasks = Vec::new();

        // Preload next track (higher priority)
        if let Some(next_idx) = adjacent.next {
            if let Some(task) = self.preload_track(next_idx, true) {
                tasks.push(task);
            }
        }

        // Preload prev track
        if let Some(prev_idx) = adjacent.prev {
            if let Some(task) = self.preload_track(prev_idx, false) {
                tasks.push(task);
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
    fn preload_track(&mut self, idx: usize, is_next: bool) -> Option<Task<Message>> {
        let song = self.library.queue.get(idx)?;

        // Check if we should preload
        if !self.library.preload_manager.should_preload(idx, is_next) {
            return None;
        }

        // Check if it's a local song with existing file
        if let Some(local_path) = queue_navigator::get_local_path(song) {
            if let Some(audio) = &self.core.audio {
                let request_id = audio.create_preload_sink(local_path.clone());
                tracing::debug!(
                    "Requesting preload for local track at index {}, request_id={}",
                    idx,
                    request_id
                );

                self.library.preload_manager.mark_pending(idx, is_next);
                return Some(Task::done(Message::PreloadRequestSent(
                    idx, is_next, request_id, local_path,
                )));
            }
            return None;
        }

        // NCM song - needs download
        if !queue_navigator::needs_ncm_download(song) {
            return None;
        }

        // Mark as pending and create download task
        self.library.preload_manager.mark_pending(idx, is_next);

        // Create async download task
        if let Some(client) = &self.core.ncm_client {
            let client = Arc::new(client.clone());
            let song = song.clone();
            Some(preload_manager::create_preload_task(
                client, idx, song, is_next,
            ))
        } else {
            None
        }
    }

    /// Handle preload-related messages
    pub fn handle_preload(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            // Preload request sent to audio thread
            Message::PreloadRequestSent(idx, is_next, request_id, path) => {
                let slot = if *is_next {
                    self.library.preload_manager.next_slot_mut()
                } else {
                    self.library.preload_manager.prev_slot_mut()
                };

                if let Some(slot) = slot {
                    if slot.is_for_index(*idx) {
                        slot.set_pending_request_id(*request_id);
                        tracing::debug!(
                            "Preload request sent: idx={}, is_next={}, request_id={}, path={:?}",
                            idx,
                            is_next,
                            request_id,
                            path
                        );
                    }
                }
                Some(Task::none())
            }

            // Preload ready message
            Message::PreloadReady(idx, file_path, is_next) => {
                self.handle_preload_complete(*idx, file_path.clone(), *is_next)
            }

            // Preload ready with SharedBuffer for streaming playback
            Message::PreloadBufferReady(idx, file_path, is_next, buffer, duration_secs) => self
                .handle_preload_buffer_ready(
                    *idx,
                    file_path.clone(),
                    *is_next,
                    buffer.clone(),
                    *duration_secs,
                ),

            Message::PreloadAudioFailed(idx, is_next) => {
                self.library.preload_manager.mark_failed(*idx, *is_next);
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
        let next_matches = self
            .library
            .preload_manager
            .next_slot()
            .map(|s| s.has_pending_request(request_id))
            .unwrap_or(false);
        let prev_matches = self
            .library
            .preload_manager
            .prev_slot()
            .map(|s| s.has_pending_request(request_id))
            .unwrap_or(false);

        if next_matches {
            if let Some(slot) = self.library.preload_manager.next_slot_mut() {
                slot.request_id = Some(request_id);
                slot.pending_request_id = None; // Clear pending
                slot.path = path.clone();
                slot.duration = duration;
                slot.state = preload_manager::SlotState::Ready;
                tracing::info!(
                    "Preload ready (next): request_id={}, path={:?}",
                    request_id,
                    path
                );
            }
        } else if prev_matches {
            if let Some(slot) = self.library.preload_manager.prev_slot_mut() {
                slot.request_id = Some(request_id);
                slot.pending_request_id = None; // Clear pending
                slot.path = path.clone();
                slot.duration = duration;
                slot.state = preload_manager::SlotState::Ready;
                tracing::info!(
                    "Preload ready (prev): request_id={}, path={:?}",
                    request_id,
                    path
                );
            }
        } else {
            tracing::debug!(
                "PreloadReady received but no matching pending slot: request_id={} (stale)",
                request_id
            );
        }
    }

    /// Handle successful preload completion
    fn handle_preload_complete(
        &mut self,
        idx: usize,
        file_path: String,
        is_next: bool,
    ) -> Option<Task<Message>> {
        // Update song info in queue
        if let Some(song) = self.library.queue.get_mut(idx) {
            song.file_path = file_path.clone();
        }

        // Request preload via audio thread
        if let Some(audio) = &self.core.audio {
            let path = PathBuf::from(&file_path);
            let request_id = audio.create_preload_sink(path.clone());
            tracing::info!(
                "NCM track downloaded at index {}, requesting preload: request_id={}",
                idx,
                request_id
            );

            return Some(Task::done(Message::PreloadRequestSent(
                idx, is_next, request_id, path,
            )));
        }

        self.library.preload_manager.mark_failed(idx, is_next);
        Some(Task::none())
    }

    /// Handle preload ready with SharedBuffer (streaming playback)
    fn handle_preload_buffer_ready(
        &mut self,
        idx: usize,
        file_path: String,
        is_next: bool,
        buffer: crate::audio::SharedBuffer,
        duration_secs: u64,
    ) -> Option<Task<Message>> {
        // Update song info in queue
        if let Some(song) = self.library.queue.get_mut(idx) {
            song.file_path = file_path.clone();
        }

        if let Some(audio) = &self.core.audio {
            let duration = Duration::from_secs(duration_secs);
            let streaming_buffer = crate::audio::StreamingBuffer::new(buffer.clone());
            let request_id = audio.create_preload_sink_streaming(streaming_buffer, duration);
            tracing::info!(
                "NCM streaming track buffer ready at index {}, requesting preload: request_id={}",
                idx,
                request_id
            );

            if is_next {
                if let Some(slot) = self.library.preload_manager.next_slot_mut() {
                    slot.buffer = Some(buffer);
                }
            } else {
                if let Some(slot) = self.library.preload_manager.prev_slot_mut() {
                    slot.buffer = Some(buffer);
                }
            }

            return Some(Task::done(Message::PreloadRequestSent(
                idx,
                is_next,
                request_id,
                PathBuf::from(file_path),
            )));
        }

        // No audio handle - mark as failed
        self.library.preload_manager.mark_failed(idx, is_next);
        Some(Task::none())
    }

    pub fn try_play_preloaded(&mut self, idx: usize, is_next: bool) -> bool {
        if let Some(mut slot) = self.library.preload_manager.take_ready(idx, is_next) {
            if let (Some(request_id), Some(audio)) = (slot.take_request_id(), &self.core.audio) {
                let path = slot.path.clone();
                audio.play_preloaded(request_id, path);

                if let Some(buffer) = slot.take_buffer() {
                    self.library.streaming_buffer = Some(buffer);
                }

                tracing::info!("Playing preloaded track at index {}", idx);
                return true;
            }
        }
        false
    }
}
