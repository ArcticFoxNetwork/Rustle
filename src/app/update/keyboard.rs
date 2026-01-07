// src/app/update/keyboard.rs
//! Keyboard and action message handlers

use iced::Task;

use crate::app::message::Message;
use crate::app::state::App;
use crate::features::Action;
use crate::ui::components::NavItem;

impl App {
    /// Handle keyboard-related messages
    pub fn handle_keyboard(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::KeyPressed(key, modifiers) => {
                // If editing a keybinding, capture the key press for that
                if self.ui.editing_keybinding.is_some() {
                    return Some(
                        self.update(Message::KeybindingKeyPressed(key.clone(), *modifiers)),
                    );
                }

                // Otherwise, check for keybinding actions
                if let Some(action) = self.core.settings.keybindings.find_action(key, modifiers) {
                    return Some(self.update(Message::ExecuteAction(action)));
                }
                Some(Task::none())
            }

            Message::ExecuteAction(action) => Some(self.execute_action(action.clone())),

            _ => None,
        }
    }

    /// Execute a keybinding action
    fn execute_action(&mut self, action: Action) -> Task<Message> {
        match action {
            Action::PlayPause => {
                return self.update(Message::TogglePlayback);
            }
            Action::NextTrack => {
                return self.update(Message::NextSong);
            }
            Action::PrevTrack => {
                return self.update(Message::PrevSong);
            }
            Action::VolumeUp => {
                if let Some(player) = &self.core.audio {
                    let current = player.get_info().volume;
                    player.set_volume((current + 0.05).min(1.0));
                }
            }
            Action::VolumeDown => {
                if let Some(player) = &self.core.audio {
                    let current = player.get_info().volume;
                    player.set_volume((current - 0.05).max(0.0));
                }
            }
            Action::VolumeMute => {
                if let Some(player) = &self.core.audio {
                    let current = player.get_info().volume;
                    if current > 0.0 {
                        // Save current volume before muting
                        self.core.volume_before_mute = Some(current);
                        player.set_volume(0.0);
                    } else {
                        // Restore previous volume or default to 0.5
                        let restore_vol = self.core.volume_before_mute.unwrap_or(0.5);
                        player.set_volume(restore_vol);
                        self.core.volume_before_mute = None;
                    }
                }
            }
            Action::SeekForward => {
                if let Some(player) = &self.core.audio {
                    let info = player.get_info();
                    let new_pos = info.position + std::time::Duration::from_secs(10);
                    if new_pos < info.duration {
                        player.seek(new_pos);
                    }
                }
            }
            Action::SeekBackward => {
                if let Some(player) = &self.core.audio {
                    let info = player.get_info();
                    let new_pos = info
                        .position
                        .saturating_sub(std::time::Duration::from_secs(10));
                    player.seek(new_pos);
                }
            }
            Action::GoHome => {
                // Close lyrics page if open
                if self.ui.lyrics.is_open {
                    self.ui.lyrics.is_open = false;
                }
                self.ui.playlist_page.current = None;
                self.ui.active_nav = NavItem::Home;
            }
            Action::GoSearch => {
                // TODO: Implement search page navigation
                tracing::info!("Go to search - not yet implemented");
            }
            Action::ToggleQueue => {
                self.ui.queue_visible = !self.ui.queue_visible;
            }
            Action::ToggleFullscreen => {
                // Toggle window fullscreen mode
                self.core.is_fullscreen = !self.core.is_fullscreen;
                let mode = if self.core.is_fullscreen {
                    iced::window::Mode::Fullscreen
                } else {
                    iced::window::Mode::Windowed
                };
                return iced::window::latest().and_then(move |id| iced::window::set_mode(id, mode));
            }
        }
        Task::none()
    }
}
