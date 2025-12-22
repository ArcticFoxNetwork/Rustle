// src/app/update/navigation.rs
//! Navigation message handlers

use iced::Task;

use crate::app::helpers::open_folder_dialog;
use crate::app::message::Message;
use crate::app::state::{App, NavigationEntry};
use crate::ui::components::NavItem;

impl App {
    /// Navigate to a specific entry (used by back/forward)
    fn navigate_to_entry(&mut self, entry: &NavigationEntry) -> Task<Message> {
        // Close lyrics page if open
        if self.ui.lyrics.is_open {
            self.ui.lyrics.is_open = false;
            self.ui.lyrics.animation.stop(iced::time::Instant::now());
        }
        // Reset playlist search state
        self.ui.playlist_page.search_expanded = false;
        self.ui.playlist_page.search_query.clear();

        // Clean up completed animations to prevent memory growth
        let now = iced::time::Instant::now();
        self.ui.playlist_page.song_animations.cleanup_completed(now);

        match entry {
            NavigationEntry::Nav(nav) => {
                self.ui.active_nav = nav.clone();
                self.ui.playlist_page.current = None;
                self.ui.playlist_page.viewing_recently_played = false;
                let scroll_id = match nav {
                    NavItem::Home | NavItem::Discover | NavItem::Radio => "home_scroll",
                    NavItem::Settings => "settings_scroll",
                    NavItem::AudioEngine => "audio_engine_scroll",
                };
                iced::widget::operation::snap_to(
                    iced::widget::Id::new(scroll_id),
                    iced::widget::scrollable::RelativeOffset { x: 0.0, y: 0.0 },
                )
            }
            NavigationEntry::Playlist(id) => {
                self.ui.playlist_page.viewing_recently_played = false;
                // Trigger playlist load
                Task::done(Message::OpenPlaylist(*id))
            }
            NavigationEntry::NcmPlaylist(id) => {
                self.ui.playlist_page.viewing_recently_played = false;
                // Trigger NCM playlist load
                Task::done(Message::OpenNcmPlaylist(*id))
            }
            NavigationEntry::RecentlyPlayed => {
                self.ui.playlist_page.viewing_recently_played = true;
                if let Some(db) = &self.core.db {
                    let db = db.clone();
                    Task::perform(
                        async move {
                            match db.get_recently_played(200).await {
                                Ok(songs) => Message::RecentlyPlayedLoaded(songs),
                                Err(e) => {
                                    tracing::error!("Failed to load recently played: {}", e);
                                    Message::Noop
                                }
                            }
                        },
                        |msg| msg,
                    )
                } else {
                    Task::none()
                }
            }
        }
    }

    /// Handle navigation-related messages
    pub fn handle_navigation(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::Noop => Some(Task::none()),

            Message::NavigateBack => {
                if let Some(entry) = self.ui.nav_history.go_back() {
                    Some(self.navigate_to_entry(&entry))
                } else {
                    Some(Task::none())
                }
            }

            Message::NavigateForward => {
                if let Some(entry) = self.ui.nav_history.go_forward() {
                    Some(self.navigate_to_entry(&entry))
                } else {
                    Some(Task::none())
                }
            }

            Message::Navigate(nav) => {
                self.ui.active_nav = nav.clone();
                self.ui.playlist_page.current = None;
                self.ui.playlist_page.viewing_recently_played = false;
                // Reset playlist search state
                self.ui.playlist_page.search_expanded = false;
                self.ui.playlist_page.search_query.clear();
                // Close lyrics page if open
                if self.ui.lyrics.is_open {
                    self.ui.lyrics.is_open = false;
                    self.ui.lyrics.animation.stop(iced::time::Instant::now());
                }
                // Reset discover view mode when navigating to Discover
                if *nav == NavItem::Discover {
                    self.ui.discover.view_mode = crate::app::state::DiscoverViewMode::Overview;
                }
                // Push to history
                self.ui.nav_history.push(NavigationEntry::Nav(nav.clone()));
                // Reset scroll position for the target page
                let scroll_id = match nav {
                    NavItem::Home | NavItem::Radio => "home_scroll",
                    NavItem::Discover => "discover_scroll",
                    NavItem::Settings => "settings_scroll",
                    NavItem::AudioEngine => "audio_engine_scroll",
                };

                // Load discover data if navigating to Discover page
                let load_task = if *nav == NavItem::Discover && !self.ui.discover.data_loaded {
                    self.load_discover_data()
                } else {
                    Task::none()
                };

                Some(Task::batch([
                    iced::widget::operation::snap_to(
                        iced::widget::Id::new(scroll_id),
                        iced::widget::scrollable::RelativeOffset { x: 0.0, y: 0.0 },
                    ),
                    load_task,
                ]))
            }

            Message::LibrarySelect(item) => {
                tracing::info!("Library selected: {:?}", item);
                match item {
                    crate::ui::components::LibraryItem::RecentlyPlayed => {
                        // Set flag to show selected state in sidebar
                        self.ui.playlist_page.viewing_recently_played = true;
                        // Push to history
                        self.ui.nav_history.push(NavigationEntry::RecentlyPlayed);
                        // Load recently played songs - will create a playlist view
                        if let Some(db) = &self.core.db {
                            let db = db.clone();
                            return Some(Task::perform(
                                async move {
                                    match db.get_recently_played(200).await {
                                        Ok(songs) => Message::RecentlyPlayedLoaded(songs),
                                        Err(e) => {
                                            tracing::error!(
                                                "Failed to load recently played: {}",
                                                e
                                            );
                                            Message::Noop
                                        }
                                    }
                                },
                                |msg| msg,
                            ));
                        }
                    }
                }
                Some(Task::none())
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

            Message::OpenSettings => {
                tracing::info!("Open settings");
                self.ui.active_nav = NavItem::Settings;
                self.ui.playlist_page.current = None;
                // Push to history
                self.ui
                    .nav_history
                    .push(NavigationEntry::Nav(NavItem::Settings));
                // Refresh cache stats and reset scroll position
                Some(Task::batch([
                    Task::done(Message::RefreshCacheStats),
                    iced::widget::operation::snap_to(
                        iced::widget::Id::new("settings_scroll"),
                        iced::widget::scrollable::RelativeOffset { x: 0.0, y: 0.0 },
                    ),
                ]))
            }

            Message::OpenSettingsWithCloseLyrics => {
                tracing::info!("Open settings with lyrics close");
                // Close lyrics page if open
                if self.ui.lyrics.is_open {
                    self.ui.lyrics.is_open = false;
                    self.ui.lyrics.animation.stop(iced::time::Instant::now());
                }
                self.ui.active_nav = NavItem::Settings;
                self.ui.playlist_page.current = None;
                // Push to history
                self.ui
                    .nav_history
                    .push(NavigationEntry::Nav(NavItem::Settings));
                // Refresh cache stats and reset scroll position
                Some(Task::batch([
                    Task::done(Message::RefreshCacheStats),
                    iced::widget::operation::snap_to(
                        iced::widget::Id::new("settings_scroll"),
                        iced::widget::scrollable::RelativeOffset { x: 0.0, y: 0.0 },
                    ),
                ]))
            }

            Message::OpenAudioEngine => {
                tracing::info!("Open audio engine");
                // Close lyrics page if open
                if self.ui.lyrics.is_open {
                    self.ui.lyrics.is_open = false;
                    self.ui.lyrics.animation.stop(iced::time::Instant::now());
                }
                self.ui.active_nav = NavItem::AudioEngine;
                self.ui.playlist_page.current = None;
                // Push to history
                self.ui
                    .nav_history
                    .push(NavigationEntry::Nav(NavItem::AudioEngine));
                // Reset scroll position for audio engine page
                Some(iced::widget::operation::snap_to(
                    iced::widget::Id::new("audio_engine_scroll"),
                    iced::widget::scrollable::RelativeOffset { x: 0.0, y: 0.0 },
                ))
            }

            _ => None,
        }
    }
}
