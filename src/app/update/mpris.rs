// src/app/update/mpris.rs
//! Media controls message handlers

use iced::Task;
use std::time::Duration;

use crate::app::message::Message;
use crate::app::state::App;
use crate::platform::media_controls::{
    MediaCommand, MediaMetadata, MediaPlaybackStatus, MediaState, is_available,
};

impl App {
    /// Handle media controls related messages
    pub fn handle_mpris(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::MprisStartedWithHandle(handle, rx) => {
                // Only process if media controls are available on this platform
                if !is_available() {
                    return Some(Task::none());
                }

                tracing::info!("Media controls service started");
                self.core.mpris_rx = Some(rx.clone());

                // Store the handle globally for updates
                let handle_clone = handle.clone();
                crate::app::helpers::set_mpris_handle(handle.clone());
                self.core.mpris_handle = Some(handle_clone);

                // Start listening for media control commands
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

    /// Handle a specific media control command
    fn handle_mpris_command(&mut self, cmd: &MediaCommand) -> Option<Task<Message>> {
        match cmd {
            MediaCommand::Play => {
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

            MediaCommand::Pause => {
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

            MediaCommand::PlayPause => Some(self.update(Message::TogglePlayback)),

            MediaCommand::Stop => {
                if let Some(player) = &mut self.core.audio {
                    player.stop();
                    self.library.current_song = None;
                    self.update_mpris_state();
                }
                Some(Task::none())
            }

            MediaCommand::Next => Some(self.update(Message::NextSong)),

            MediaCommand::Previous => Some(self.update(Message::PrevSong)),

            MediaCommand::Seek(offset_us) => {
                if let Some(player) = &mut self.core.audio {
                    let current_pos = player.get_info().position;
                    let new_pos = current_pos + Duration::from_micros(*offset_us as u64);
                    let _ = player.seek(new_pos);
                }
                Some(Task::none())
            }

            MediaCommand::SetPosition(_track_id, position_us) => {
                if let Some(player) = &mut self.core.audio {
                    let new_pos = Duration::from_micros(*position_us as u64);
                    let _ = player.seek(new_pos);
                }
                Some(Task::none())
            }

            MediaCommand::SetVolume(volume) => {
                Some(self.update(Message::SetVolume(*volume as f32)))
            }

            MediaCommand::Raise => {
                // Show window if hidden
                if self.core.window_hidden {
                    Some(self.update(Message::ShowWindow))
                } else {
                    Some(Task::none())
                }
            }

            MediaCommand::Quit => Some(iced::exit()),
        }
    }

    /// Update media controls state when playback changes
    pub fn update_mpris_state(&mut self) {
        if let Some(handle) = &self.core.mpris_handle {
            let status = if let Some(player) = &self.core.audio {
                if player.is_empty() {
                    MediaPlaybackStatus::Stopped
                } else if player.is_playing() {
                    MediaPlaybackStatus::Playing
                } else {
                    MediaPlaybackStatus::Paused
                }
            } else {
                MediaPlaybackStatus::Stopped
            };

            let metadata = if let Some(song) = &self.library.current_song {
                let art_url = song.cover_path.as_ref().map(|path| {
                    if path.starts_with("http") {
                        path.clone()
                    } else {
                        format!("file://{}", path)
                    }
                });

                MediaMetadata {
                    track_id: Some(song.id.to_string()),
                    title: Some(song.title.clone()),
                    artists: vec![song.artist.clone()],
                    album: Some(song.album.clone()),
                    album_artists: vec![],
                    length_us: Some(song.duration_secs as i64 * 1_000_000),
                    art_url,
                }
            } else {
                MediaMetadata::default()
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

            let state = MediaState {
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

            // Update media controls state directly via handle
            handle.update(state);
        }
    }
}
