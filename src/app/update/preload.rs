//! Audio preloading for seamless track switching
//!
//! This module uses the new architecture:
//! - QueueNavigator for consistent index calculations (Single Source of Truth)
//! - PreloadManager for state tracking (prevents duplicate requests)
//! - PreloadSlot contains pre-decoded Sink for zero-delay playback
//! - AudioPlayer no longer manages preloading internally

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;

use crate::app::message::Message;
use crate::app::state::App;

use super::preload_manager::{self, PreloadSlot};
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
            // Local song - create Sink immediately and mark as ready
            if let Some(player) = &self.core.audio {
                match player.create_preload_sink(&local_path) {
                    Ok((sink, duration)) => {
                        let slot = PreloadSlot::from_local(idx, local_path, sink, duration);
                        self.library.preload_manager.mark_ready(slot, is_next);
                        tracing::info!("Preloaded local track at index {} ({})", idx, if is_next { "next" } else { "prev" });
                    }
                    Err(e) => {
                        tracing::warn!("Failed to create preload sink for local track {}: {}", idx, e);
                        self.library.preload_manager.mark_failed(idx, is_next);
                    }
                }
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
            // Preload ready message (file downloaded, need to create Sink in main thread)
            Message::PreloadReady(idx, file_path, is_next) => {
                self.handle_preload_complete(*idx, file_path.clone(), *is_next)
            }

            // Preload ready with SharedBuffer for streaming playback
            Message::PreloadBufferReady(idx, file_path, is_next, buffer, duration_secs) => {
                self.handle_preload_buffer_ready(*idx, file_path.clone(), *is_next, buffer.clone(), *duration_secs)
            }

            Message::PreloadAudioFailed(idx, is_next) => {
                self.library.preload_manager.mark_failed(*idx, *is_next);
                Some(Task::none())
            }

            _ => None,
        }
    }

    /// Handle successful preload completion (file-based)
    /// Creates Sink in main thread (since Sink is not Send)
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

        // Create Sink in main thread
        let path = PathBuf::from(&file_path);
        if !path.exists() {
            tracing::warn!("Preload file not found: {}", file_path);
            self.library.preload_manager.mark_failed(idx, is_next);
            return Some(Task::none());
        }

        if let Some(player) = &self.core.audio {
            match player.create_preload_sink(&path) {
                Ok((sink, duration)) => {
                    let slot = PreloadSlot::from_local(idx, path, sink, duration);
                    self.library.preload_manager.mark_ready(slot, is_next);
                    tracing::info!("Preloaded NCM track at index {} ({})", idx, if is_next { "next" } else { "prev" });
                }
                Err(e) => {
                    tracing::warn!("Failed to create preload sink for NCM track {}: {}", idx, e);
                    self.library.preload_manager.mark_failed(idx, is_next);
                }
            }
        }

        Some(Task::none())
    }

    /// Handle preload ready with SharedBuffer (streaming playback)
    /// Creates Sink from StreamingBuffer in main thread
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

        if let Some(player) = &self.core.audio {
            // Create Sink from StreamingBuffer (not file!)
            let streaming_buffer = crate::audio::StreamingBuffer::new(buffer.clone());
            let duration = std::time::Duration::from_secs(duration_secs);
            
            match player.create_preload_sink_streaming(streaming_buffer, duration) {
                Ok((sink, duration)) => {
                    let path = PathBuf::from(&file_path);
                    let downloaded = buffer.downloaded();
                    let slot = PreloadSlot::from_streaming(idx, path, sink, duration, buffer);
                    self.library.preload_manager.mark_ready(slot, is_next);
                    tracing::info!("Preloaded NCM streaming track at index {} ({}) - buffer: {} bytes downloaded", 
                        idx, if is_next { "next" } else { "prev" }, downloaded);
                }
                Err(e) => {
                    tracing::warn!("Failed to create streaming preload sink for NCM track {}: {}", idx, e);
                    self.library.preload_manager.mark_failed(idx, is_next);
                }
            }
        }

        Some(Task::none())
    }
}
