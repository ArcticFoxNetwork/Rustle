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

            Message::WindowMinimize => {
                Some(iced::window::latest().and_then(|id| iced::window::minimize(id, true)))
            }

            Message::WindowMaximize => {
                Some(iced::window::latest().and_then(|id| iced::window::toggle_maximize(id)))
            }

            Message::MouseMoved(position) => {
                self.core.mouse_position = *position;
                // Update sidebar width if dragging
                if self.ui.sidebar_dragging {
                    const MIN_WIDTH: f32 = 200.0;
                    const MAX_WIDTH: f32 = 400.0;
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

            Message::MousePressed => {
                // Drag window if mouse is in top 48px area (title bar)
                const DRAG_AREA_HEIGHT: f32 = 48.0;
                if self.core.mouse_position.y < DRAG_AREA_HEIGHT {
                    Some(iced::window::latest().and_then(|id| iced::window::drag(id)))
                } else {
                    Some(Task::none())
                }
            }

            Message::MouseReleased => {
                // 结束侧边栏拖动
                if self.ui.sidebar_dragging {
                    self.ui.sidebar_dragging = false;
                }
                Some(Task::none())
            }

            _ => None,
        }
    }
}
