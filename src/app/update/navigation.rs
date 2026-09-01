// src/app/update/navigation.rs
//! Navigation message handlers

use iced::Task;

use crate::app::helpers::open_folder_dialog;
use crate::app::message::Message;
use crate::app::state::{App, NavigationEntry};

impl App {
    /// Handle navigation-related messages
    pub fn handle_navigation(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::Noop => Some(Task::none()),

            Message::NavigateBack => {
                if let Some(NavigationEntry::Route(route)) = self.ui.nav_history.go_back() {
                    Some(self.navigate_to_route(route, false))
                } else {
                    Some(Task::none())
                }
            }

            Message::NavigateForward => {
                if let Some(NavigationEntry::Route(route)) = self.ui.nav_history.go_forward() {
                    Some(self.navigate_to_route(route, false))
                } else {
                    Some(Task::none())
                }
            }

            Message::Navigate(_) => {
                let Some(route) = self.route_for_message(message) else {
                    return Some(Task::none());
                };

                Some(self.navigate_to_route(route, true))
            }

            Message::LibrarySelect(_)
            | Message::OpenSettings
            | Message::OpenSettingsWithCloseLyrics
            | Message::OpenAudioEngine
            | Message::OpenUser(_)
            | Message::OpenAlbum(_) => {
                let Some(route) = self.route_for_message(message) else {
                    return Some(Task::none());
                };
                Some(self.navigate_to_route(route, true))
            }

            Message::SearchChanged(query) => {
                self.ui.search_query = query.clone();
                Some(Task::none())
            }

            Message::PlayHero => {
                tracing::info!("Playing Global Hits 2024");
                Some(Task::none())
            }

            Message::ImportLocalPlaylist => {
                tracing::info!("Import local playlist");
                Some(Task::perform(open_folder_dialog(), Message::FolderSelected))
            }

            Message::MouseMoved(position) => {
                self.core.mouse_position = *position;
                // Update sidebar width if dragging
                if self.ui.sidebar_dragging {
                    const MIN_WIDTH: f32 = 240.0;
                    const MAX_WIDTH: f32 = 440.0;
                    let old_sidebar_width = self.ui.sidebar_width;
                    self.ui.sidebar_width = position.x.clamp(MIN_WIDTH, MAX_WIDTH);
                    let delta = old_sidebar_width - self.ui.sidebar_width;
                    self.ui.discover.content_width =
                        (self.ui.discover.content_width + delta).max(200.0);
                    self.ui.playlist_page.content_width =
                        (self.ui.playlist_page.content_width + delta).max(200.0);
                    self.ui.search.content_width =
                        (self.ui.search.content_width + delta).max(200.0);
                }
                Some(Task::none())
            }

            Message::MouseReleased => {
                // 结束侧边栏拖动
                if self.ui.sidebar_dragging {
                    self.ui.sidebar_dragging = false;
                }
                Some(Task::none())
            }

            Message::MouseWheelScrolled { delta_y } => {
                if self.ui.is_volume_slider_hovered && *delta_y != 0.0 {
                    // 每个滚轮步长 = 2% 音量，signum 保证步长恒定
                    let step = delta_y.signum() * 0.02;
                    let current = self.playback_info().volume;
                    let new_volume = (current + step).clamp(0.0, 1.0);
                    self.set_output_volume(new_volume, true);
                }
                Some(Task::none())
            }

            Message::VolumeSliderHovered(hovered) => {
                self.ui.is_volume_slider_hovered = *hovered;
                Some(Task::none())
            }

            _ => None,
        }
    }
}
