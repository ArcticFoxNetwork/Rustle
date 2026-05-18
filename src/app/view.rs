// src/app/view.rs
//! Application view rendering

use iced::mouse::Interaction;
use iced::widget::{Space, column, container, mouse_area, opaque, row, stack};
use iced::{Alignment, Element, Fill};

use super::message::Message;
use super::{App, Route};
use crate::ui::{components, pages, theme, widgets};

impl App {
    fn current_song_artist_id(&self) -> Option<u64> {
        self.playback.current_artist_id
    }

    /// Build the view for a specific window
    pub fn view(&self, _window_id: iced::window::Id) -> Element<'_, Message> {
        // Check if lyrics page is open or animating
        let lyrics_progress = self.ui.lyrics.animation.progress();
        let lyrics_animating = self.ui.lyrics.animation.is_animating();
        let lyrics_overlay: Element<'_, Message> =
            if self.ui.lyrics.is_open || lyrics_animating || lyrics_progress > 0.01 {
                if let Some(song) = &self.playback.current_song {
                    let runtime = self.playback_runtime();
                    // Get playback info - same logic as player bar for consistency
                    let (is_playing, position, duration) =
                        if runtime.has_loaded_audio && runtime.info.duration.as_secs_f32() > 0.0 {
                            (
                                runtime.is_playing(),
                                runtime.info.position.as_secs_f32()
                                    / runtime.info.duration.as_secs_f32().max(1.0),
                                runtime.info.duration.as_secs_f32(),
                            )
                        } else {
                            // Player exists but no file loaded yet (e.g., NCM song still resolving)
                            // Use saved state for display
                            let saved_pos = self
                                .playback
                                .saved_state
                                .as_ref()
                                .map(|s| s.position_secs as f32)
                                .unwrap_or(0.0);
                            let song_duration = song.duration_secs.max(1) as f32;
                            (false, saved_pos / song_duration, song_duration)
                        };

                    // Use preview position while seeking, otherwise use actual position
                    let display_position = if self.ui.seek_preview_position.is_some() {
                        self.ui.seek_preview_position.unwrap()
                    } else {
                        position
                    };

                    // Calculate current lyric line based on playback position
                    let position_ms = (position * duration * 1000.0) as u64;
                    let current_line = pages::find_current_line(&self.ui.lyrics.lines, position_ms);

                    pages::lyrics::view(
                        song,
                        self.current_song_artist_id(),
                        is_playing,
                        display_position,
                        duration,
                        self.ui.lyrics.cached_engine_lines.as_ref(), // Use cached engine lines (Rc)
                        current_line,
                        self.core.settings.play_mode,
                        lyrics_progress,
                        &self.ui.lyrics.bg_colors,
                        &self.ui.lyrics.bg_shader,
                        &self.ui.lyrics.textured_bg_shader,
                        self.ui.lyrics.engine.as_ref(),
                        self.core.settings.display.power_saving_mode,
                        // Check if current song is liked
                        if song.id < 0 {
                            let ncm_id = (-song.id) as u64;
                            self.core
                                .user_info
                                .as_ref()
                                .map(|u| u.like_songs.contains(&ncm_id))
                                .unwrap_or(false)
                        } else {
                            false
                        },
                        self.playback_buffer_progress(),
                        self.is_fm_mode(),
                    )
                } else {
                    Space::new().width(0).height(0).into()
                }
            } else {
                Space::new().width(0).height(0).into()
            };

        // Left sidebar
        let sidebar = components::sidebar::view(
            &self.ui.current_route,
            self.core.locale,
            self.core.is_logged_in,
            self.core.user_info.as_ref(),
            self.ui.importing_playlist.as_ref(),
            &self.library.playlists,
            &self.ui.home.user_playlists,
            &self.ui.sidebar_animations,
            self.ui.sidebar_width,
        );

        // Sidebar resize handle (draggable divider)
        let resize_handle = components::sidebar_resize_handle::view(self.ui.sidebar_dragging);

        // Determine main content: playlist page or nav page
        // Get liked songs for playlist view (empty set if not logged in)
        let liked_songs = self
            .core
            .user_info
            .as_ref()
            .map(|u| &u.like_songs)
            .cloned()
            .unwrap_or_default();

        let current_user_id = self.core.user_info.as_ref().map(|u| u.user_id);

        let current_playing_id = self.playback.current_song.as_ref().map(|s| s.id);

        let main_content = match &self.ui.current_route {
            Route::Playlist(_)
            | Route::NcmPlaylist(_)
            | Route::Album(_)
            | Route::RecentlyPlayed => {
                if let Some(playlist) = &self.ui.playlist_page.current {
                    pages::playlist::view(
                        playlist,
                        &self.ui.playlist_page.song_animations,
                        &self.ui.playlist_page.icon_animations,
                        &self.ui.playlist_page.search_animation,
                        self.ui.playlist_page.search_expanded,
                        &self.ui.playlist_page.search_query,
                        liked_songs,
                        self.core.locale,
                        self.ui.playlist_page.scroll_state.clone(),
                        current_user_id,
                        current_playing_id,
                    )
                } else {
                    Space::new().width(Fill).height(Fill).into()
                }
            }
            Route::User(_) => {
                if let Some(playlist) = &self.ui.playlist_page.current {
                    pages::user::view(
                        playlist,
                        &self.ui.playlist_page.song_animations,
                        &self.ui.playlist_page.icon_animations,
                        &self.ui.playlist_page.search_animation,
                        self.ui.playlist_page.search_expanded,
                        &self.ui.playlist_page.search_query,
                        liked_songs,
                        self.core.locale,
                        self.ui.playlist_page.scroll_state.clone(),
                        current_user_id,
                        current_playing_id,
                        self.ui.playlist_page.content_width,
                    )
                } else {
                    Space::new().width(Fill).height(Fill).into()
                }
            }
            Route::Artist(_) => {
                if let Some(playlist) = &self.ui.playlist_page.current {
                    pages::artist::view(
                        playlist,
                        &self.ui.playlist_page.artist_album_covers,
                        &self.ui.playlist_page.song_animations,
                        &self.ui.playlist_page.icon_animations,
                        &self.ui.playlist_page.search_animation,
                        self.ui.playlist_page.search_expanded,
                        &self.ui.playlist_page.search_query,
                        liked_songs,
                        self.core.locale,
                        self.ui.playlist_page.scroll_state.clone(),
                        current_user_id,
                        current_playing_id,
                        self.ui.playlist_page.content_width,
                    )
                } else {
                    Space::new().width(Fill).height(Fill).into()
                }
            }
            Route::Search { .. } => pages::search::view(&self.ui.search, self.core.locale),
            Route::Home => pages::home::view(
                &self.ui.search_query,
                &self.ui.home,
                self.core.locale,
                self.core.is_logged_in,
            ),
            Route::Discover(mode) => {
                let _ = mode;
                pages::discover::view(&self.ui.discover, self.core.locale, self.core.is_logged_in)
            }
            Route::Radio => pages::home::view(
                &self.ui.search_query,
                &self.ui.home,
                self.core.locale,
                self.core.is_logged_in,
            ),
            Route::Settings(section) => {
                let _ = section;
                pages::settings::view(
                    &self.core.settings,
                    self.audio_output_devices(),
                    self.ui.active_settings_section,
                    self.core.locale,
                    self.ui.editing_keybinding,
                    self.core.is_logged_in,
                    self.core.user_info.as_ref(),
                    self.ui.cache_stats.as_ref(),
                )
            }
            Route::AudioEngine => pages::audio_engine::view(
                &self.core.settings,
                self.core.locale,
                Some(self.playback_analysis_data()),
            ),
        };

        let needs_top_padding = !matches!(
            self.ui.current_route,
            Route::Settings(_)
                | Route::AudioEngine
                | Route::Playlist(_)
                | Route::NcmPlaylist(_)
                | Route::User(_)
                | Route::Artist(_)
                | Route::Album(_)
                | Route::RecentlyPlayed
                | Route::Search { .. }
        );

        let main_content = if needs_top_padding {
            container(main_content)
                .padding(iced::Padding::new(0.0).top(70.0))
                .width(Fill)
                .height(Fill)
                .into()
        } else {
            main_content
        };

        let top_bar = components::window_controls::view(
            self.core.locale,
            self.ui.nav_history.can_go_back(),
            self.ui.nav_history.can_go_forward(),
            &self.ui.search_query,
        );
        let controls_overlay = container(top_bar).width(Fill).padding(0);

        // Right panel with content and window controls overlay
        let right_panel = container(
            stack![main_content, controls_overlay,]
                .width(Fill)
                .height(Fill),
        )
        .width(Fill)
        .height(Fill)
        .style(theme::main_content);

        // Build right panel with player bar at bottom (always visible)
        let right_content: Element<'_, Message> = {
            let runtime = self.playback_runtime();
            // Get playback info from audio player, or fall back to saved state
            let (is_playing, position, duration, volume) =
                if runtime.has_loaded_audio && runtime.info.duration.as_secs_f32() > 0.0 {
                    (
                        runtime.is_playing(),
                        runtime.display_position.as_secs_f32(),
                        runtime.info.duration.as_secs_f32().max(1.0),
                        runtime.info.volume,
                    )
                } else {
                    // Player exists but no file loaded - use saved state
                    let saved_pos = self
                        .playback
                        .saved_state
                        .as_ref()
                        .map(|s| s.position_secs as f32)
                        .unwrap_or(0.0);
                    let saved_vol = self
                        .playback
                        .saved_state
                        .as_ref()
                        .map(|s| s.volume as f32)
                        .unwrap_or(1.0);
                    let song_duration = self
                        .playback
                        .current_song
                        .as_ref()
                        .map(|s| s.duration_secs as f32)
                        .unwrap_or(1.0)
                        .max(1.0);
                    (false, saved_pos, song_duration, saved_vol)
                };

            // Use preview position while seeking, otherwise use actual position
            let display_position = if let Some(preview) = self.ui.seek_preview_position {
                preview
            } else {
                position / duration
            };

            let is_buffering = self.playback_is_buffering();

            let is_fm_mode = self.is_fm_mode();
            let is_first_song = self
                .playback
                .current_index
                .map(|idx| idx == 0)
                .unwrap_or(true);

            let player_bar = components::player_bar::view(
                self.playback.current_song.as_ref(),
                self.current_song_artist_id(),
                is_playing,
                display_position,
                duration,
                volume,
                self.core.settings.play_mode,
                is_buffering,
                self.playback_buffer_progress(),
                is_fm_mode,
                is_first_song,
            );

            // Build content with player bar - always use stack to keep layout consistent
            let queue_overlay: Element<'_, Message> = if self.ui.queue_visible {
                let queue_popup = components::queue_panel::view(
                    &self.playback.queue,
                    self.playback.current_index,
                    self.core.locale,
                    is_fm_mode,
                );

                // Position queue popup above player bar, aligned to the right
                // Wrap in opaque + mouse_area to block event/cursor penetration
                // Clicking outside the queue bubble dismisses it
                opaque(
                    mouse_area(
                        container(
                            column![
                                Space::new().height(Fill),
                                container(queue_popup)
                                    .width(Fill)
                                    .align_x(Alignment::End)
                                    .padding(iced::Padding::new(0.0).right(20.0).bottom(8.0)),
                                Space::new().height(components::PLAYER_BAR_HEIGHT),
                            ]
                            .width(Fill)
                            .height(Fill),
                        )
                        .width(Fill)
                        .height(Fill),
                    )
                    .interaction(Interaction::Idle)
                    .on_press(Message::ToggleQueue),
                )
                .into()
            } else {
                // Empty overlay when queue is hidden - keeps layout structure consistent
                Space::new().width(0).height(0).into()
            };

            // Always use stack layout to preserve scrollable state
            stack![
                column![right_panel, player_bar,].width(Fill).height(Fill),
                queue_overlay,
            ]
            .width(Fill)
            .height(Fill)
            .into()
        };

        // Main layout: sidebar + resize handle + right content (with player bar and queue popup)
        let main_layout: Element<'_, Message> = row![sidebar, resize_handle, right_content]
            .width(Fill)
            .height(Fill)
            .into();

        // Build overlays - always use consistent stack structure to preserve scroll

        // Toast overlay (empty space if not visible)
        // Wrapped in opaque to prevent click/cursor penetration through toast area
        let toast_overlay: Element<'_, Message> = if self.ui.toast_visible {
            if let Some(toast) = &self.ui.toast {
                let toast_widget = widgets::view_toast(toast);
                opaque(
                    container(toast_widget)
                        .width(Fill)
                        .padding(20)
                        .align_x(Alignment::Center),
                )
                .into()
            } else {
                Space::new().width(0).height(0).into()
            }
        } else {
            Space::new().width(0).height(0).into()
        };

        // Edit dialog overlay (empty space if not visible)
        let dialog_progress = self.ui.dialogs.edit_animation.progress();
        let edit_dialog_overlay: Element<'_, Message> =
            if self.ui.dialogs.edit_open || dialog_progress > 0.01 {
                components::edit_dialog::view(
                    &self.ui.dialogs.edit_name,
                    &self.ui.dialogs.edit_description,
                    self.ui.dialogs.edit_cover.as_deref(),
                    self.ui.dialogs.edit_watch_available,
                    self.ui.dialogs.edit_watch_enabled,
                    self.ui.dialogs.edit_watch_path.as_deref(),
                    dialog_progress,
                    self.core.locale,
                )
            } else {
                Space::new().width(0).height(0).into()
            };

        // Exit dialog overlay (empty space if not visible)
        let exit_dialog_progress = self.ui.dialogs.exit_animation.progress();
        let exit_dialog_overlay: Element<'_, Message> =
            if self.ui.dialogs.exit_open || exit_dialog_progress > 0.01 {
                components::exit_dialog::view(
                    self.ui.dialogs.exit_remember,
                    exit_dialog_progress,
                    self.core.locale,
                )
            } else {
                Space::new().width(0).height(0).into()
            };

        // Delete playlist dialog overlay
        let delete_dialog_progress = self.ui.dialogs.delete_animation.progress();
        let delete_dialog_overlay: Element<'_, Message> =
            if self.ui.dialogs.delete_pending_id.is_some() || delete_dialog_progress > 0.01 {
                let playlist_name = self
                    .ui
                    .dialogs
                    .delete_pending_id
                    .and_then(|id| self.library.playlists.iter().find(|p| p.id == id))
                    .map(|p| p.name.as_str())
                    .unwrap_or("Unknown");
                components::delete_playlist_dialog::view(
                    playlist_name,
                    delete_dialog_progress,
                    self.core.locale,
                )
            } else {
                Space::new().width(0).height(0).into()
            };

        // Login popup overlay
        let login_popup_overlay = components::login_popup::view(
            self.ui.home.login_popup_open,
            self.ui.home.qr_code_path.as_ref(),
            self.ui.home.qr_status.as_deref(),
            self.core.user_info.as_ref(),
            self.core.is_logged_in,
            self.core.locale,
        );

        // Always use consistent stack structure to preserve scroll position
        stack![
            main_layout,
            lyrics_overlay,
            toast_overlay,
            edit_dialog_overlay,
            exit_dialog_overlay,
            delete_dialog_overlay,
            login_popup_overlay,
        ]
        .width(Fill)
        .height(Fill)
        .into()
    }
}
