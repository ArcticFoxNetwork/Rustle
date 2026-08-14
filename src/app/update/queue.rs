// src/app/update/queue.rs
//! Queue management message handlers

use iced::Task;

use crate::app::message::Message;
use crate::app::state::App;

impl App {
    /// Handle queue-related messages
    pub fn handle_queue(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::ToggleQueue => {
                self.ui.queue_visible = !self.ui.queue_visible;

                // When opening the queue, scroll to center the current song
                if self.ui.queue_visible {
                    let offset = crate::ui::components::queue_panel::calculate_scroll_offset(
                        self.playback.queue.len(),
                        self.playback.current_index,
                    );
                    return Some(iced::widget::operation::snap_to(
                        iced::widget::Id::new(
                            crate::ui::components::queue_panel::QUEUE_SCROLLABLE_ID,
                        ),
                        iced::widget::scrollable::RelativeOffset { x: 0.0, y: offset },
                    ));
                }
                Some(Task::none())
            }

            Message::PlayPlaylist(playlist_id) => {
                self.exit_fm_mode();
                let id = *playlist_id;

                // For recently played (id = -1), use the recently_played list
                if id == -1 {
                    if !self.library.recently_played.is_empty() {
                        let db_songs = self.library.recently_played.clone();
                        self.playback.queue = db_songs.clone();
                        self.persist_queue_snapshot();

                        return Some(self.play_song_at_index(0));
                    }
                    return Some(Task::none());
                }

                // For NCM playlists (negative ID), use the cached NCM playlist songs
                if id <= 0 {
                    let ncm_songs = &self.ui.home.current_ncm_playlist_songs;
                    if !ncm_songs.is_empty() {
                        let source_id = if self.is_fm_mode() {
                            None
                        } else {
                            self.current_route_ncm_scrobble_source()
                        };
                        return Some(Task::done(Message::AddNcmPlaylistWithSource(
                            ncm_songs.clone(),
                            true,
                            source_id,
                        )));
                    }
                    return Some(Task::none());
                }

                // For local playlists, load from database
                if let Some(db) = &self.core.db {
                    let db = db.clone();
                    return Some(Task::perform(
                        async move { db.get_playlist_songs(id).await.unwrap_or_default() },
                        Message::QueueLoaded,
                    ));
                }
                Some(Task::none())
            }

            Message::QueueLoaded(songs) => {
                self.exit_fm_mode();
                self.store_db_song_cover_paths(songs);
                if !songs.is_empty() {
                    self.playback.queue = songs.clone();
                    self.persist_queue_snapshot();
                    return Some(self.play_song_at_index(0));
                }
                Some(Task::none())
            }

            Message::PlayQueueIndex(idx) => Some(self.play_song_at_index(*idx)),

            Message::SongResolvedStreaming(
                idx,
                finalized_cache_path,
                cover_path,
                shared_buffer,
                duration_secs,
                context,
            ) => Some(self.handle_song_resolved_streaming(
                *idx,
                finalized_cache_path.clone(),
                cover_path.clone(),
                shared_buffer.clone(),
                *duration_secs,
                context.clone(),
            )),

            Message::SongResolveFailed(context) => {
                if !self.accepts_audio_context(context) {
                    tracing::debug!(
                        generation = context.generation.0,
                        "Ignoring stale song resolution failure"
                    );
                    return Some(Task::none());
                }

                tracing::error!("Failed to resolve song");
                // Use handle_playback_failure for consistent failure tracking
                if let Some(idx) = self.playback.current_index {
                    return Some(self.handle_playback_failure(idx, "Song resolution failed"));
                }
                Some(Self::toast_error("无法加载歌曲".to_string()))
            }

            Message::RemoveFromQueue(idx) => {
                if *idx < self.playback.queue.len() {
                    self.playback.queue.remove(*idx);
                    if let Some(current_idx) = self.playback.current_index {
                        if *idx < current_idx {
                            self.playback.current_index = Some(current_idx - 1);
                        } else if *idx == current_idx {
                            if self.playback.queue.is_empty() {
                                self.playback.current_index = None;
                            } else if current_idx >= self.playback.queue.len() {
                                self.playback.current_index = Some(self.playback.queue.len() - 1);
                            }
                        }
                    }

                    if let Some(db) = &self.core.db {
                        let db = db.clone();
                        let position = *idx as i64;
                        tokio::spawn(async move {
                            let _ = db.remove_from_queue(position).await;
                        });
                    }

                    // Refresh coordinator window and re-preload adjacent tracks
                    self.refresh_preload_window();
                    return Some(self.preload_adjacent_tracks_with_ncm());
                }
                Some(Task::none())
            }

            Message::ClearQueue => {
                self.playback.queue.clear();
                self.playback.current_index = None;
                self.playback.preload_coordinator.clear_window();
                // Release audio preload sinks
                let released = self.playback.audio_preload_manager.reset();
                self.release_preload_requests(released);

                if let Some(db) = &self.core.db {
                    let db = db.clone();
                    tokio::spawn(async move {
                        let _ = db.clear_queue().await;
                    });
                }
                Some(Task::none())
            }

            _ => None,
        }
    }
}
