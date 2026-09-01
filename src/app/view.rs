// src/app/view.rs
//! Application view rendering

use iced::mouse::Interaction;
use iced::widget::{
    Space, button, column, container, mouse_area, opaque, row, scrollable, stack, text,
};
use iced::{Alignment, Color, Element, Fill, Length};

use super::message::Message;
use super::{App, Route};
use crate::image::{CoverSize, ImageKind};
use crate::ui::animation::SmoothScrollTarget;
use crate::ui::components::cover_image;
use crate::ui::overlay::{self, ModalKind, OverlayKind};
use crate::ui::theme;
use crate::ui::{components, pages, widgets};

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
                        &self.ui.image_state,
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
                        self.core.window_maximized,
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
            self.ui.importing_playlist.as_ref(),
            &self.library.playlists,
            &self.ui.home.user_playlists,
            &self.ui.image_state,
            &self.ui.sidebar_animations,
            self.ui.sidebar_width,
        );

        // Sidebar resize handle (draggable divider)
        let resize_handle = components::sidebar_resize_handle::view(self.ui.sidebar_dragging);

        // Determine main content: playlist page or nav page
        let liked_songs = self.core.user_info.as_ref().map(|u| &u.like_songs);

        let current_user_id = self.core.user_info.as_ref().map(|u| u.user_id);

        let current_playing_id = self.playback.current_song.as_ref().map(|s| s.id);

        let active_personal_fm_cover = if self.is_fm_mode() {
            self.playback
                .current_song
                .as_ref()
                .and_then(|song| crate::image::song_cover_key(song.id))
                .and_then(|(kind, id)| self.ui.image_state.get(kind, id))
        } else {
            None
        };

        let main_content = match &self.ui.current_route {
            Route::Playlist(_)
            | Route::NcmPlaylist(_)
            | Route::Album(_)
            | Route::RecentlyPlayed => {
                if let Some(playlist) = &self.ui.playlist_page.current {
                    pages::playlist::view(
                        playlist,
                        &self.ui.image_state,
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
                        self.ui.playlist_page.description_expanded,
                        self.ui.playlist_page.gradient_animation.progress(),
                    )
                        self.ui.playlist_page.gradient_source(),
                } else {
                    pages::playlist::gradient_placeholder(self.ui.playlist_page.gradient_source())
                }
            }
            Route::User(_) => {
                if let Some(playlist) = &self.ui.playlist_page.current {
                    pages::user::view(
                        playlist,
                        &self.ui.image_state,
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
                        self.ui.playlist_page.description_expanded,
                        self.ui.playlist_page.gradient_animation.progress(),
                    )
                        self.ui.playlist_page.gradient_source(),
                } else {
                    pages::playlist::gradient_placeholder(self.ui.playlist_page.gradient_source())
                }
            }
            Route::Artist(_) => {
                if let Some(playlist) = &self.ui.playlist_page.current {
                    pages::artist::view(
                        playlist,
                        &self.ui.image_state,
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
                        self.ui.playlist_page.description_expanded,
                        self.ui.playlist_page.gradient_animation.progress(),
                    )
                        self.ui.playlist_page.gradient_source(),
                } else {
                    pages::playlist::gradient_placeholder(self.ui.playlist_page.gradient_source())
                }
            }
            Route::Search { .. } => {
                pages::search::view(&self.ui.search, &self.ui.image_state, self.core.locale)
            }
            Route::Discover(mode) => {
                let _ = mode;
                pages::discover::view(
                    &self.ui.discover,
                    &self.ui.image_state,
                    self.core.locale,
                    active_personal_fm_cover,
                )
            }
            Route::Radio => pages::discover::view(
                &self.ui.discover,
                &self.ui.image_state,
                self.core.locale,
                active_personal_fm_cover,
            ),
            Route::Downloads => crate::ui::components::download_panel::download_panel(
                self.core.locale,
                &self.core.download_manager,
                self.ui.download_tab,
                &self.ui.image_state,
            ),
            Route::Settings(section) => {
                let _ = section;
                pages::settings::view(
                    &self.core.settings,
                    self.audio_output_devices(),
                    self.lyrics_font_families(),
                    self.ui.active_settings_section,
                    self.core.locale,
                    self.ui.editing_keybinding,
                    self.core.is_logged_in,
                    self.core.user_info.as_ref(),
                    &self.ui.image_state,
                    self.ui.cache_stats.as_ref(),
                )
            }
            Route::AudioEngine => pages::audio_engine::view(
                &self.core.settings,
                self.core.locale,
                Some(self.playback_analysis_data()),
            ),
        };

        let top_bar = components::window_controls::view(
            self.core.locale,
            self.ui.nav_history.can_go_back(),
            self.ui.nav_history.can_go_forward(),
            &self.ui.search_query,
            self.core.is_logged_in,
            self.core.user_info.as_ref(),
            &self.ui.image_state,
            !self.ui.current_route.has_gradient_background(),
            self.core.window_maximized,
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

        // Build player bar (always visible, full width at bottom)
        let runtime = self.playback_runtime();
        let (is_playing, position, duration, volume) =
            if runtime.has_loaded_audio && runtime.info.duration.as_secs_f32() > 0.0 {
                (
                    runtime.is_playing(),
                    runtime.display_position.as_secs_f32(),
                    runtime.info.duration.as_secs_f32().max(1.0),
                    runtime.info.volume,
                )
            } else {
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

        let current_song_cover = self.playback.current_song.as_ref().and_then(|song| {
            let (kind, id) = crate::image::song_cover_key(song.id)?;
            self.ui.image_state.get(kind, id)
        });

        let current_favorite = self.playback.current_song.as_ref().and_then(|song| {
            (song.id < 0).then(|| {
                let ncm_id = (-song.id) as u64;
                let is_liked = self
                    .core
                    .user_info
                    .as_ref()
                    .is_some_and(|user| user.like_songs.contains(&ncm_id));
                (ncm_id, is_liked)
            })
        });

        let progress_colors = self.playback.current_song.as_ref().and_then(|song| {
            self.playback
                .preload_coordinator
                .background_colors(song.id)
                .map(|colors| {
                    let to_color = |[r, g, b, a]: [f32; 4]| iced::Color::from_rgba(r, g, b, a);
                    [to_color(colors.0), to_color(colors.1), to_color(colors.2)]
                })
        });

        let player_bar = components::player_bar::view(
            self.playback.current_song.as_ref(),
            self.current_song_artist_id(),
            is_playing,
            display_position,
            duration,
            volume,
            self.core.settings.play_mode,
            current_favorite,
            progress_colors,
            is_buffering,
            self.playback_buffer_progress(),
            is_fm_mode,
            is_first_song,
            current_song_cover,
            self.playback.current_quality.as_ref(),
        );

        // Queue overlay - full width, positioned above player bar
        let queue_overlay: Element<'_, Message> = if self.ui.queue_visible {
            let queue_popup = components::queue_panel::view(
                &self.playback.queue,
                self.playback.current_index,
                self.core.locale,
                is_fm_mode,
            );

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
        } else {
            Space::new().width(0).height(0).into()
        };

        // Content row: sidebar + resize handle + right panel (fills remaining vertical space)
        let content_row = row![sidebar, resize_handle, right_panel]
            .width(Fill)
            .height(Fill);

        // Main layout: content row on top, full-width player bar at bottom
        let main_layout: Element<'_, Message> = stack![
            column![content_row, player_bar].width(Fill).height(Fill),
            queue_overlay,
        ]
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
            } else {
                Space::new().width(0).height(0).into()
            }
        } else {
            Space::new().width(0).height(0).into()
        };

        // Login popup overlay (independent — not managed by overlay stack)
        let login_popup_overlay = components::login_popup::view(
            self.ui.home.login_popup_open,
            self.ui.home.qr_code_path.as_ref(),
            self.ui.home.qr_status.as_deref(),
            self.core.user_info.as_ref(),
            self.core.is_logged_in,
            self.core.locale,
        );

        // Context menu overlay — positioned at cursor (independent)
        let context_menu_overlay: Element<'_, Message> =
            if let Some(ref menu) = self.ui.context_menu {
                components::context_menu::view(
                    menu,
                    self.core.locale,
                    self.core.window_width,
                    self.core.window_height,
                )
            } else {
                Space::new().width(0).height(0).into()
            };

        // ── Unified modal overlay slots ──
        let locale = self.core.locale;
        let modal_slot = |idx: usize| -> Element<'_, Message> {
            match self.ui.overlay_stack.get(idx) {
                Some(entry) => match &entry.kind {
                    OverlayKind::Modal(kind, config) => {
                        let content: Element<'_, Message> = match kind {
                            ModalKind::SongEdit(edit) => {
                                let title = locale.get(crate::i18n::Key::SongEditTitle).to_string();
                                overlay::modal_section(
                                    title,
                                    Message::DismissTopModal,
                                    overlay::modal_body(
                                        components::song_info_dialog::view_edit_body(edit, locale),
                                    ),
                                    overlay::modal_footer(vec![
                                        cancel_btn(locale),
                                        accent_btn(
                                            locale.get(crate::i18n::Key::SongEditSave).to_string(),
                                            Message::SaveSongEdits(edit.song_id),
                                        ),
                                    ]),
                                )
                            }
                            ModalKind::PlaylistEdit {
                                playlist_id: _,
                                name,
                                description,
                                cover_path,
                                watch_enabled,
                                watch_available,
                                watch_path,
                            } => {
                                let body = components::edit_dialog::view_body(
                                    name,
                                    description,
                                    cover_path.as_deref(),
                                    *watch_available,
                                    *watch_enabled,
                                    watch_path.as_deref(),
                                    locale,
                                );
                                let title =
                                    locale.get(crate::i18n::Key::EditPlaylistTitle).to_string();
                                overlay::modal_section(
                                    title,
                                    Message::DismissTopModal,
                                    overlay::modal_body(body),
                                    overlay::modal_footer(vec![
                                        cancel_btn(locale),
                                        save_btn(locale),
                                    ]),
                                )
                            }
                            ModalKind::DeleteConfirm {
                                playlist_id: _,
                                playlist_name,
                            } => {
                                let title = locale
                                    .get(crate::i18n::Key::DeletePlaylistTitle)
                                    .to_string();
                                overlay::modal_section(
                                    title,
                                    Message::DismissTopModal,
                                    overlay::modal_body(
                                        text(format!(
                                            "确定要删除歌单「{}」吗？此操作无法撤销。",
                                            playlist_name
                                        ))
                                        .size(14)
                                        .into(),
                                    ),
                                    overlay::modal_footer(vec![
                                        cancel_btn(locale),
                                        delete_btn(locale),
                                    ]),
                                )
                            }
                            ModalKind::DownloadConfirm {
                                playlist_id: _,
                                playlist_name,
                                song_count,
                            } => {
                                let title = locale
                                    .get(crate::i18n::Key::DownloadPlaylistTitle)
                                    .to_string();
                                let template =
                                    locale.get(crate::i18n::Key::DownloadPlaylistConfirm);
                                let body = template
                                    .replace("{name}", playlist_name)
                                    .replace("{count}", &song_count.to_string());
                                let confirm_label = locale
                                    .get(crate::i18n::Key::DownloadPlaylistConfirmBtn)
                                    .to_string();
                                overlay::modal_section(
                                    title,
                                    Message::DismissTopModal,
                                    overlay::modal_body(text(body).size(14).into()),
                                    overlay::modal_footer(vec![
                                        cancel_btn(locale),
                                        accent_btn(confirm_label, Message::ConfirmDownloadPlaylist),
                                    ]),
                                )
                            }
                            ModalKind::ExitConfirm { remember_choice } => {
                                let body =
                                    components::exit_dialog::view_body(*remember_choice, locale);
                                let title =
                                    locale.get(crate::i18n::Key::ExitDialogTitle).to_string();
                                overlay::modal_section(
                                    title,
                                    Message::DismissTopModal,
                                    overlay::modal_body(body),
                                    overlay::modal_footer(vec![
                                        cancel_btn(locale),
                                        exit_btn(locale),
                                        minimize_btn(locale),
                                    ]),
                                )
                            }
                            ModalKind::PlaylistPicker {
                                song_id,
                                ncm_playlists,
                            } => {
                                let song_id = *song_id;
                                let title = locale
                                    .get(crate::i18n::Key::PlaylistPickerTitle)
                                    .to_string();

                                let list: Element<'_, Message> = match ncm_playlists {
                                    // NCM: show online playlists with music icon
                                    Some(pls) if !pls.is_empty() => {
                                        let mut col = column![].spacing(4);
                                        for pl in pls {
                                            let pl_name = pl.name.clone();
                                            let pl_id = pl.id;
                                            let s = CoverSize::Picker;
                                            let cover_el = cover_image::cover(
                                                self.ui
                                                    .image_state
                                                    .get(ImageKind::PlaylistCover, pl_id),
                                                ImageKind::PlaylistCover,
                                                s,
                                            );
                                            col = col.push(
                                                button(
                                                    row![
                                                        cover_el,
                                                        Space::new().width(12),
                                                        text(pl_name).size(13),
                                                        Space::new().width(Fill),
                                                    ]
                                                    .align_y(Alignment::Center),
                                                )
                                                .padding([8, 14])
                                                .width(Fill)
                                                .style(|theme, status| {
                                                    let bg =
                                                        matches!(status, button::Status::Hovered)
                                                            .then_some(iced::Background::Color(
                                                                crate::ui::theme::hover_bg(theme),
                                                            ));
                                                    button::Style {
                                                        background: bg,
                                                        text_color: crate::ui::theme::text_primary(
                                                            theme,
                                                        ),
                                                        border: iced::Border {
                                                            radius: 8.0.into(),
                                                            ..Default::default()
                                                        },
                                                        ..Default::default()
                                                    }
                                                })
                                                .on_press(Message::AddToNcmPlaylist(
                                                    song_id.unsigned_abs(),
                                                    pl_id,
                                                )),
                                            );
                                        }
                                        widgets::smooth_scroll(
                                            scrollable(container(col).width(Fill))
                                                .height(Length::Fixed(300.0))
                                                .id(iced::widget::Id::new(
                                                    "playlist_picker_scroll",
                                                )),
                                            SmoothScrollTarget::Native("playlist_picker_scroll"),
                                            Message::SmoothScroll,
                                        )
                                        .into()
                                    }
                                    _ => {
                                        let playlists = self.library.playlists.clone();
                                        if playlists.is_empty() {
                                            text("No playlists available")
                                                .size(14)
                                                .style(|theme| text::Style {
                                                    color: Some(crate::ui::theme::text_muted(
                                                        theme,
                                                    )),
                                                })
                                                .into()
                                        } else {
                                            let mut col = column![].spacing(4);
                                            for pl in &playlists {
                                                let pl_id = pl.id;
                                                let pl_name = pl.name.clone();
                                                let s = CoverSize::Picker;
                                                let cover_handle =
                                                    u64::try_from(pl_id).ok().and_then(|id| {
                                                        self.ui
                                                            .image_state
                                                            .get(ImageKind::LocalPlaylistCover, id)
                                                    });
                                                let thumb = cover_image::custom(
                                                    cover_handle,
                                                    ImageKind::LocalPlaylistCover,
                                                    s.px(),
                                                    s.radius(),
                                                );
                                                col = col.push(
                                                    button(
                                                        row![
                                                            thumb,
                                                            Space::new().width(12),
                                                            text(pl_name).size(13),
                                                            Space::new().width(Fill),
                                                        ]
                                                        .align_y(Alignment::Center),
                                                    )
                                                    .padding([8, 14])
                                                    .width(Fill)
                                                    .style(|theme, status| {
                                                        let bg = matches!(
                                                            status,
                                                            button::Status::Hovered
                                                        )
                                                        .then_some(iced::Background::Color(
                                                            crate::ui::theme::hover_bg(theme),
                                                        ));
                                                        button::Style {
                                                            background: bg,
                                                            text_color:
                                                                crate::ui::theme::text_primary(
                                                                    theme,
                                                                ),
                                                            border: iced::Border {
                                                                radius: 8.0.into(),
                                                                ..Default::default()
                                                            },
                                                            ..Default::default()
                                                        }
                                                    })
                                                    .on_press(Message::PlaylistPickerConfirm(
                                                        song_id, pl_id,
                                                    )),
                                                );
                                            }
                                            widgets::smooth_scroll(
                                                scrollable(container(col).width(Fill))
                                                    .height(Length::Fixed(300.0))
                                                    .id(iced::widget::Id::new(
                                                        "playlist_picker_scroll",
                                                    )),
                                                SmoothScrollTarget::Native(
                                                    "playlist_picker_scroll",
                                                ),
                                                Message::SmoothScroll,
                                            )
                                            .into()
                                        }
                                    }
                                };

                                overlay::modal_section(
                                    title,
                                    Message::DismissTopModal,
                                    overlay::modal_body(list),
                                    overlay::modal_footer(vec![cancel_btn(locale)]),
                                )
                            }
                        };
                        overlay::modal_view(config, content, Message::DismissTopModal)
                    }
                },
                _ => Space::new().width(0).height(0).into(),
            }
        };

        let resize_handles = components::window_resize_handles::view();

        stack![
            main_layout,
            lyrics_overlay,
            toast_overlay,
            login_popup_overlay,
            modal_slot(0),
            modal_slot(1),
            modal_slot(2),
            context_menu_overlay,
            resize_handles,
        ]
        .width(Fill)
        .height(Fill)
        .into()
    }
}

// ── Modal button helpers ──

fn cancel_btn(locale: crate::i18n::Locale) -> Element<'static, Message> {
    button(text(locale.get(crate::i18n::Key::Cancel).to_string()).size(14))
        .padding([8, 20])
        .style(|theme, status| {
            let bg = matches!(status, button::Status::Hovered)
                .then_some(iced::Background::Color(theme::hover_bg(theme)));
            button::Style {
                background: bg,
                text_color: theme::text_secondary(theme),
                border: iced::Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: theme::divider(theme),
                },
                ..Default::default()
            }
        })
        .on_press(Message::DismissTopModal)
        .into()
}

fn save_btn(locale: crate::i18n::Locale) -> Element<'static, Message> {
    button(
        text(locale.get(crate::i18n::Key::Save).to_string())
            .size(14)
            .color(Color::BLACK),
    )
    .padding([8, 20])
    .style(|_theme, status| {
        let bg = matches!(status, button::Status::Hovered)
            .then_some(theme::ACCENT_PINK_HOVER)
            .unwrap_or(theme::ACCENT_PINK);
        button::Style {
            background: Some(iced::Background::Color(bg)),
            text_color: Color::BLACK,
            border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .on_press(Message::SavePlaylistEdits)
    .into()
}

fn delete_btn(locale: crate::i18n::Locale) -> Element<'static, Message> {
    button(
        text(locale.get(crate::i18n::Key::Delete).to_string())
            .size(14)
            .color(Color::WHITE),
    )
    .padding([8, 20])
    .style(|theme, status| {
        let bg = match status {
            button::Status::Hovered => theme::danger_hover(theme),
            _ => theme::danger(theme),
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            text_color: Color::WHITE,
            border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .on_press(Message::ConfirmDeletePlaylist)
    .into()
}

fn exit_btn(locale: crate::i18n::Locale) -> Element<'static, Message> {
    button(
        text(locale.get(crate::i18n::Key::ExitDialogExit).to_string())
            .size(14)
            .style(|theme| text::Style {
                color: Some(theme::text_primary(theme)),
            }),
    )
    .padding([8, 20])
    .style(|theme, status| {
        let bg = match status {
            button::Status::Hovered => theme::hover_bg(theme),
            _ => theme::surface_container(theme),
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            text_color: theme::text_primary(theme),
            border: iced::Border {
                radius: 8.0.into(),
                width: 1.0,
                color: theme::divider(theme),
            },
            ..Default::default()
        }
    })
    .on_press(Message::ConfirmExit)
    .into()
}

fn minimize_btn(locale: crate::i18n::Locale) -> Element<'static, Message> {
    button(
        text(locale.get(crate::i18n::Key::ExitDialogMinimize).to_string())
            .size(14)
            .color(Color::BLACK),
    )
    .padding([8, 20])
    .style(|theme, status| {
        let bg = match status {
            button::Status::Hovered => theme::hover_bg(theme),
            _ => theme::text_primary(theme),
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            text_color: Color::BLACK,
            border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .on_press(Message::MinimizeToTray)
    .into()
}

fn accent_btn(label: String, msg: Message) -> Element<'static, Message> {
    button(text(label).size(14).color(Color::WHITE))
        .padding([8, 20])
        .style(|_theme, status| {
            let bg = match status {
                button::Status::Hovered => theme::ACCENT_PINK_HOVER,
                _ => theme::ACCENT_PINK,
            };
            button::Style {
                background: Some(iced::Background::Color(bg)),
                text_color: Color::WHITE,
                border: iced::Border {
                    radius: 8.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .on_press(msg)
        .into()
}
