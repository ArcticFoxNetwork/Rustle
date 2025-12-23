//! Audio preloading for seamless track switching
//!
//! This module uses the new architecture:
//! - QueueNavigator for consistent index calculations (Single Source of Truth)
//! - PreloadManager for state tracking (prevents duplicate requests)
//! - Separated audio/cover downloads (audio is priority, cover is background)

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;

use crate::app::message::Message;
use crate::app::state::App;

use super::preload_manager;
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
            .invalidate_if_stale(adjacent.next, adjacent.prev);

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
        let should_preload = if is_next {
            self.library.preload_manager.should_preload_next(idx)
        } else {
            self.library.preload_manager.should_preload_prev(idx)
        };

        if !should_preload {
            return None;
        }

        // Check if it's a local song with existing file
        if let Some(local_path) = queue_navigator::get_local_path(song) {
            // Local song - preload directly into player
            if let Some(player) = &mut self.core.audio {
                if is_next {
                    player.preload_next(local_path.clone());
                    self.library
                        .preload_manager
                        .mark_next_ready(idx, local_path);
                } else {
                    player.preload_prev(local_path.clone());
                    self.library
                        .preload_manager
                        .mark_prev_ready(idx, local_path);
                }
            }
            return None;
        }

        // NCM song - needs download
        if !queue_navigator::needs_ncm_download(song) {
            return None;
        }

        // Mark as pending and create download task
        if is_next {
            self.library.preload_manager.mark_next_pending(idx);
        } else {
            self.library.preload_manager.mark_prev_pending(idx);
        }

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

    /// Check if we can use preloaded track for next song
    pub fn can_use_preloaded_next(&self, expected_idx: usize) -> bool {
        // First check PreloadManager
        if self
            .library
            .preload_manager
            .is_next_ready_for(expected_idx)
            .is_some()
        {
            return true;
        }

        // Fallback: check player's preload buffer
        if let Some(player) = &self.core.audio {
            if let Some(preloaded_path) = player.preloaded_next_path() {
                if let Some(song) = self.library.queue.get(expected_idx) {
                    if let Some(expected_path) = queue_navigator::get_local_path(song) {
                        return preloaded_path == &expected_path;
                    }
                }
            }
        }
        false
    }

    /// Check if we can use preloaded track for previous song
    pub fn can_use_preloaded_prev(&self, expected_idx: usize) -> bool {
        // First check PreloadManager
        if self
            .library
            .preload_manager
            .is_prev_ready_for(expected_idx)
            .is_some()
        {
            return true;
        }

        // Fallback: check player's preload buffer
        if let Some(player) = &self.core.audio {
            if let Some(preloaded_path) = player.preloaded_prev_path() {
                if let Some(song) = self.library.queue.get(expected_idx) {
                    if let Some(expected_path) = queue_navigator::get_local_path(song) {
                        return preloaded_path == &expected_path;
                    }
                }
            }
        }
        false
    }

    /// Handle preload-related messages
    pub fn handle_preload(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            // New architecture messages
            Message::PreloadAudioReady(idx, file_path, is_next) => {
                self.handle_preload_complete(*idx, file_path.clone(), None, *is_next)
            }

            Message::PreloadAudioFailed(idx, is_next) => {
                let idx = *idx;
                let is_next = *is_next;

                if is_next {
                    self.library.preload_manager.mark_next_failed(idx);
                } else {
                    self.library.preload_manager.mark_prev_failed(idx);
                }

                Some(Task::none())
            }

            _ => None,
        }
    }

    /// Handle successful preload completion
    fn handle_preload_complete(
        &mut self,
        idx: usize,
        file_path: String,
        cover_path: Option<String>,
        is_next: bool,
    ) -> Option<Task<Message>> {
        // Update song info in queue
        if let Some(song) = self.library.queue.get_mut(idx) {
            song.file_path = file_path.clone();
            if let Some(cover) = cover_path {
                song.cover_path = Some(cover);
            }
        }

        // Preload audio into player
        let path = PathBuf::from(&file_path);
        if path.exists() {
            if let Some(player) = &mut self.core.audio {
                if is_next {
                    player.preload_next(path.clone());
                    self.library.preload_manager.mark_next_ready(idx, path);
                    tracing::info!("Preloaded next track at index {}", idx);
                } else {
                    player.preload_prev(path.clone());
                    self.library.preload_manager.mark_prev_ready(idx, path);
                    tracing::info!("Preloaded prev track at index {}", idx);
                }
            }
        }

        Some(Task::none())
    }
}
