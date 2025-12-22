// src/app/update/mpris.rs
//! MPRIS message handlers

use iced::Task;
use std::time::Duration;

use crate::app::message::Message;
use crate::app::state::App;
use crate::features::{MprisCommand, MprisMetadata, MprisPlaybackStatus, MprisState};

impl App {
    /// Handle MPRIS-related messages
    pub fn handle_mpris(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::MprisStartedWithHandle(handle, rx) => {
                tracing::info!("MPRIS service started");
                self.core.mpris_rx = Some(rx.clone());

                // Store the handle globally for updates
                let handle_clone = handle.clone();
                crate::app::helpers::set_mpris_handle(handle.clone());
                self.core.mpris_handle = Some(handle_clone);

                // Start listening for MPRIS commands
                let rx = rx.clone();
                Some(Task::run(
                    async_stream::stream! {
                        loop {
                            let cmd = rx.lock().await.recv().await;
                            if let Some(cmd) = cmd {
                                yield cmd;
                            } else {
                                break;
                            }
                        }
                    },
                    Message::MprisCommand,
                ))
            }

            Message::MprisCommand(cmd) => self.handle_mpris_command(cmd),

            _ => None,
        }
    }

    /// Handle a specific MPRIS command
    fn handle_mpris_command(&mut self, cmd: &MprisCommand) -> Option<Task<Message>> {
        match cmd {
            MprisCommand::Play => {
                if let Some(player) = &mut self.core.audio {
                    if !player.is_playing() {
                        Some(self.update(Message::TogglePlayback))
                    } else {
                        Some(Task::none())
                    }
                } else {
                    Some(Task::none())
                }
            }

            MprisCommand::Pause => {
                if let Some(player) = &mut self.core.audio {
                    if player.is_playing() {
                        Some(self.update(Message::TogglePlayback))
                    } else {
                        Some(Task::none())
                    }
                } else {
                    Some(Task::none())
                }
            }

            MprisCommand::PlayPause => Some(self.update(Message::TogglePlayback)),

            MprisCommand::Stop => {
                if let Some(player) = &mut self.core.audio {
                    player.stop();
                    self.library.current_song = None;
                    self.update_mpris_state();
                }
                Some(Task::none())
            }

            MprisCommand::Next => Some(self.update(Message::NextSong)),

            MprisCommand::Previous => Some(self.update(Message::PrevSong)),

            MprisCommand::Seek(offset_us) => {
                if let Some(player) = &mut self.core.audio {
                    let current_pos = player.get_info().position;
                    let new_pos = current_pos + Duration::from_micros(*offset_us as u64);
                    let _ = player.seek(new_pos);
                }
                Some(Task::none())
            }

            MprisCommand::SetPosition(_track_id, position_us) => {
                if let Some(player) = &mut self.core.audio {
                    let new_pos = Duration::from_micros(*position_us as u64);
                    let _ = player.seek(new_pos);
                }
                Some(Task::none())
            }

            MprisCommand::SetVolume(volume) => {
                Some(self.update(Message::SetVolume(*volume as f32)))
            }

            MprisCommand::Raise => {
                // Show window if hidden
                if self.core.window_hidden {
                    Some(self.update(Message::ShowWindow))
                } else {
                    Some(Task::none())
                }
            }

            MprisCommand::Quit => Some(iced::exit()),
        }
    }

    /// Update MPRIS state when playback changes
    pub fn update_mpris_state(&mut self) {
        if let Some(handle) = &self.core.mpris_handle {
            let status = if let Some(player) = &self.core.audio {
                if player.is_empty() {
                    MprisPlaybackStatus::Stopped
                } else if player.is_playing() {
                    MprisPlaybackStatus::Playing
                } else {
                    MprisPlaybackStatus::Paused
                }
            } else {
                MprisPlaybackStatus::Stopped
            };

            let metadata = if let Some(song) = &self.library.current_song {
                let art_url = song.cover_path.as_ref().map(|path| {
                    if path.starts_with("http") {
                        path.clone()
                    } else {
                        format!("file://{}", path)
                    }
                });

                MprisMetadata {
                    track_id: Some(song.id.to_string()),
                    title: Some(song.title.clone()),
                    artists: vec![song.artist.clone()],
                    album: Some(song.album.clone()),
                    album_artists: vec![],
                    length_us: Some(song.duration_secs as i64 * 1_000_000),
                    art_url,
                }
            } else {
                MprisMetadata::default()
            };

            let position = if let Some(player) = &self.core.audio {
                player.get_info().position.as_micros() as i64
            } else {
                0
            };

            let volume = if let Some(player) = &self.core.audio {
                player.get_info().volume
            } else {
                0.0
            };

            let can_go_next = self
                .library
                .queue_index
                .is_some_and(|i| i + 1 < self.library.queue.len());
            let can_go_previous = self.library.queue_index.is_some_and(|i| i > 0);
            let can_play = self.core.audio.is_some();
            let can_pause = can_play;
            let can_seek = self.core.audio.is_some();

            // Debug logging
            let is_paused = self
                .core
                .audio
                .as_ref()
                .map(|p| !p.is_playing())
                .unwrap_or(true);
            tracing::debug!(
                "MPRIS state update: queue_len={}, queue_index={:?}, can_go_next={}, position_us={}, is_paused={}",
                self.library.queue.len(),
                self.library.queue_index,
                can_go_next,
                position,
                is_paused
            );

            let state = MprisState {
                status,
                metadata,
                position_us: position,
                volume: volume as f64,
                can_go_next,
                can_go_previous,
                can_play,
                can_pause,
                can_seek,
            };

            // Update MPRIS state directly via handle
            handle.update(state);
        }
    }
}
