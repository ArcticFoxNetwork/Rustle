// src/app/update/tray.rs
//! System tray message handlers

use iced::Task;

use crate::app::helpers::update_tray_state_full;
use crate::app::message::Message;
use crate::app::state::App;
use crate::features::TrayCommand;

impl App {
    /// Handle tray-related messages
    pub fn handle_tray(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::TrayStarted(rx) => {
                tracing::info!("Tray service started");
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
                    Message::TrayCommand,
                ))
            }

            Message::TrayCommand(cmd) => {
                match cmd {
                    TrayCommand::ShowOrFocusWindow => {
                        return Some(self.update(Message::ShowWindow));
                    }
                    TrayCommand::ToggleWindow => {
                        return Some(self.update(Message::ToggleWindow));
                    }
                    TrayCommand::PlayPause => {
                        return Some(self.update(Message::TogglePlayback));
                    }
                    TrayCommand::NextTrack => {
                        return Some(self.update(Message::NextSong));
                    }
                    TrayCommand::PrevTrack => {
                        return Some(self.update(Message::PrevSong));
                    }
                    TrayCommand::SetPlayMode(mode) => {
                        self.core.settings.play_mode = *mode;
                        let _ = self.core.settings.save();
                        // Clear shuffle cache and re-calculate for new mode
                        self.clear_shuffle_cache();
                        self.cache_shuffle_indices();
                        let _ = self.preload_adjacent_tracks_with_ncm();
                        let (title, artist) = self
                            .library
                            .current_song
                            .as_ref()
                            .map(|s| (Some(s.title.clone()), Some(s.artist.clone())))
                            .unwrap_or((None, None));
                        let is_playing = self
                            .core
                            .audio
                            .as_ref()
                            .map(|p| p.is_playing())
                            .unwrap_or(false);
                        update_tray_state_full(is_playing, title, artist, *mode);
                    }
                    TrayCommand::ToggleFavorite => {
                        // Toggle favorite for current NCM song
                        if let Some(song) = &self.library.current_song {
                            if song.id < 0 {
                                let ncm_id = (-song.id) as u64;
                                return Some(self.update(Message::ToggleFavorite(ncm_id)));
                            }
                        }
                    }
                    TrayCommand::Quit => {
                        return Some(self.update(Message::ConfirmExit));
                    }
                }
                Some(Task::none())
            }

            _ => None,
        }
    }
}
