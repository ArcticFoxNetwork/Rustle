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
                        let db_songs: Vec<crate::database::DbSong> = ncm_songs
                            .iter()
                            .map(|song| crate::database::DbSong {
                                id: -(song.id as i64),
                                file_path: String::new(),
                                title: song.name.clone(),
                                artist: song.singer.clone(),
                                album: song.album.clone(),
                                duration_secs: (song.duration / 1000) as i64,
                                track_number: None,
                                year: None,
                                genre: None,
                                cover_path: if song.pic_url.is_empty() {
                                    None
                                } else {
                                    Some(song.pic_url.clone())
                                },
                                file_hash: None,
                                file_size: 0,
                                format: Some("mp3".to_string()),
                                normalization_gain: None,
                                play_count: 0,
                                last_played: None,
                                last_modified: 0,
                                is_missing: false,
                                created_at: 0,
                            })
                            .collect();

                        self.playback.queue = db_songs.clone();
                        self.persist_queue_snapshot();

                        return Some(self.play_song_at_index(0));
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
                file_path,
                cover_path,
                shared_buffer,
                duration_secs,
            ) => Some(self.handle_song_resolved_streaming(
                *idx,
                file_path.clone(),
                cover_path.clone(),
                shared_buffer.clone(),
                *duration_secs,
            )),

            Message::SongResolveFailed => {
                tracing::error!("Failed to resolve song");
                // Use handle_playback_failure for consistent failure tracking
                if let Some(idx) = self.playback.current_index {
                    return Some(self.handle_playback_failure(idx, "Song resolution failed"));
                }
                Some(Task::done(Message::ShowErrorToast(
                    "无法加载歌曲".to_string(),
                )))
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

                    // Re-preload adjacent tracks after queue change
                    let _ = self.preload_adjacent_tracks_with_ncm();
                }
                Some(Task::none())
            }

            Message::ClearQueue => {
                self.playback.queue.clear();
                self.playback.current_index = None;

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
