// src/app/update/playlist.rs
//! Playlist page and edit dialog message handlers

use iced::Task;
use iced::time::Instant;

use crate::app::helpers::load_playlist_view;
use crate::app::message::Message;
use crate::app::state::App;
use crate::ui::widgets::Toast;

impl App {
    /// Handle playlist-related messages
    pub fn handle_playlist(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::OpenPlaylist(id) => {
                let playlist_id = *id;

                // Check if already viewing this playlist - skip redundant load
                if self.is_viewing_playlist(playlist_id) {
                    tracing::debug!("Already viewing playlist {}, skipping load", playlist_id);
                    return Some(Task::none());
                }

                tracing::info!("Opening playlist: {}", playlist_id);
                // Reset playlist search state when opening a new playlist
                self.ui.playlist_page.search_expanded = false;
                self.ui.playlist_page.search_query.clear();
                self.ui.playlist_page.viewing_recently_played = false;

                // 清除旧动画状态，防止内存增长
                self.ui.clear_playlist_animations();

                // Set loading state
                self.ui.playlist_page.load_state =
                    crate::app::update::page_loader::PlaylistLoadState::Loading;

                // Close lyrics page if open
                if self.ui.lyrics.is_open {
                    self.ui.lyrics.is_open = false;
                    self.ui.lyrics.animation.stop();
                }
                // Push to navigation history
                self.ui
                    .nav_history
                    .push(crate::app::state::NavigationEntry::Playlist(playlist_id));
                if let Some(db) = &self.core.db {
                    let db = db.clone();
                    return Some(Task::perform(
                        load_playlist_view(db, playlist_id),
                        |result| match result {
                            Some(view) => Message::PlaylistViewLoaded(view),
                            None => Message::DatabaseError("Playlist not found".into()),
                        },
                    ));
                }
                Some(Task::none())
            }

            Message::RequestDeletePlaylist(id) => {
                tracing::info!("Requesting delete for playlist: {}", id);
                self.ui.dialogs.delete_pending_id = Some(*id);
                self.ui.dialogs.delete_animation.start();
                Some(Task::none())
            }

            Message::ConfirmDeletePlaylist => {
                if let Some(playlist_id) = self.ui.dialogs.delete_pending_id.take() {
                    tracing::info!("Confirming delete for playlist: {}", playlist_id);
                    self.ui.dialogs.delete_animation.stop();
                    if let Some(db) = &self.core.db {
                        let db = db.clone();
                        return Some(Task::perform(
                            async move {
                                db.delete_playlist(playlist_id).await.ok();
                                playlist_id
                            },
                            Message::PlaylistDeleted,
                        ));
                    }
                }
                Some(Task::none())
            }

            Message::CancelDeletePlaylist => {
                tracing::info!("Cancelled playlist deletion");
                self.ui.dialogs.delete_pending_id = None;
                self.ui.dialogs.delete_animation.stop();
                Some(Task::none())
            }

            Message::PlaylistDeleted(id) => {
                tracing::info!("Playlist {} deleted", id);
                // Remove from sidebar list
                self.library.playlists.retain(|p| p.id != *id);
                // Clear current playlist if it was the deleted one
                if self.ui.playlist_page.current.as_ref().map(|p| p.id) == Some(*id) {
                    self.ui.playlist_page.current = None;
                }
                // Show toast with auto-dismiss after 3 seconds
                self.ui.toast = Some(Toast::success("歌单已删除".to_string()));
                self.ui.toast_visible = true;
                Some(Task::perform(
                    async {
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    },
                    |_| Message::HideToast,
                ))
            }

            Message::PlaylistViewLoaded(view) => {
                tracing::info!("Playlist view loaded: {}", view.name);
                self.ui.playlist_page.current = Some(view.clone());
                self.ui.playlist_page.load_state =
                    crate::app::update::page_loader::PlaylistLoadState::Ready;
                // Reset scroll position for playlist page
                Some(iced::widget::operation::snap_to(
                    iced::widget::Id::new("playlist_scroll"),
                    iced::widget::scrollable::RelativeOffset { x: 0.0, y: 0.0 },
                ))
            }

            Message::HoverSong(id) => {
                self.ui
                    .playlist_page
                    .song_animations
                    .set_hovered_exclusive(*id);
                Some(Task::none())
            }

            Message::HoverIcon(id) => {
                self.ui
                    .playlist_page
                    .icon_animations
                    .set_hovered_exclusive(*id);
                Some(Task::none())
            }

            Message::HoverSidebar(id) => {
                self.ui.sidebar_animations.set_hovered_exclusive(*id);
                Some(Task::none())
            }

            Message::AnimationTick => {
                let now = Instant::now();

                // Update audio fade state for smooth pause/resume transitions
                self.update_audio_fade();

                // Check if lyrics page close animation is complete
                self.check_lyrics_page_close();

                // Update lyrics animations if lyrics page is open
                if self.ui.lyrics.is_open {
                    let _ = self.update_lyrics_animations();
                }

                // 清理已完成的淡出动画
                self.ui.cleanup_animations(now);

                // Lazy load covers for visible songs in playlist
                if let Some(task) = self.check_visible_song_covers() {
                    return Some(task);
                }

                Some(Task::none())
            }

            Message::EditPlaylist(id) => {
                tracing::info!("Edit playlist: {}", id);
                if let Some(playlist) = &self.ui.playlist_page.current {
                    self.ui.dialogs.edit_open = true;
                    self.ui.dialogs.editing_playlist_id = Some(*id);
                    self.ui.dialogs.edit_name = playlist.name.clone();
                    self.ui.dialogs.edit_description =
                        playlist.description.clone().unwrap_or_default();
                    self.ui.dialogs.edit_cover = playlist.cover_path.clone();
                    self.ui.dialogs.edit_animation.start();
                }
                Some(Task::none())
            }

            Message::CloseEditDialog => {
                self.ui.dialogs.edit_animation.stop();
                self.ui.dialogs.edit_open = false;
                self.ui.dialogs.editing_playlist_id = None;
                Some(Task::none())
            }

            Message::EditPlaylistNameChanged(name) => {
                self.ui.dialogs.edit_name = name.clone();
                Some(Task::none())
            }

            Message::EditPlaylistDescriptionChanged(desc) => {
                self.ui.dialogs.edit_description = desc.clone();
                Some(Task::none())
            }

            Message::PickCoverImage => Some(Task::perform(
                async {
                    let result = rfd::AsyncFileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                        .pick_file()
                        .await;
                    result.map(|f| f.path().to_string_lossy().to_string())
                },
                Message::CoverImagePicked,
            )),

            Message::CoverImagePicked(path) => {
                if let Some(p) = path {
                    self.ui.dialogs.edit_cover = Some(p.clone());
                }
                Some(Task::none())
            }

            Message::SavePlaylistEdits => {
                if let (Some(db), Some(playlist_id)) =
                    (&self.core.db, self.ui.dialogs.editing_playlist_id)
                {
                    let db = db.clone();
                    let name = self.ui.dialogs.edit_name.clone();
                    let description = if self.ui.dialogs.edit_description.is_empty() {
                        None
                    } else {
                        Some(self.ui.dialogs.edit_description.clone())
                    };
                    let cover = self.ui.dialogs.edit_cover.clone();

                    self.ui.dialogs.edit_animation.stop();
                    self.ui.dialogs.edit_open = false;
                    self.ui.dialogs.editing_playlist_id = None;

                    return Some(Task::perform(
                        async move {
                            db.update_playlist_full(
                                playlist_id,
                                &name,
                                description.as_deref(),
                                cover.as_deref(),
                            )
                            .await
                            .ok();
                            playlist_id
                        },
                        |id| Message::PlaylistUpdated(id),
                    ));
                }
                Some(Task::none())
            }

            Message::PlaylistUpdated(playlist_id) => {
                if let Some(db) = &self.core.db {
                    let db1 = db.clone();
                    let db2 = db.clone();
                    let id = *playlist_id;
                    return Some(Task::batch([
                        Task::perform(load_playlist_view(db1, id), |result| match result {
                            Some(view) => Message::PlaylistViewLoaded(view),
                            None => Message::DatabaseError("Playlist not found".into()),
                        }),
                        Task::perform(
                            async move { db2.get_all_playlists().await.unwrap_or_default() },
                            Message::PlaylistsLoaded,
                        ),
                    ]));
                }
                Some(Task::none())
            }

            Message::ShowToast(msg) => {
                self.ui.toast = Some(Toast::success(msg.clone()));
                self.ui.toast_visible = true;
                // Auto-hide toast after 3 seconds
                Some(Task::perform(
                    async {
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    },
                    |_| Message::HideToast,
                ))
            }

            Message::ShowErrorToast(msg) => {
                self.ui.toast = Some(Toast::error(msg.clone()));
                self.ui.toast_visible = true;
                // Auto-hide error toast after 4 seconds
                Some(Task::perform(
                    async {
                        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                    },
                    |_| Message::HideToast,
                ))
            }

            Message::TogglePlaylistSearch => {
                self.ui.playlist_page.search_expanded = !self.ui.playlist_page.search_expanded;
                if self.ui.playlist_page.search_expanded {
                    self.ui.playlist_page.search_animation.start();
                    // Focus the search input
                    Some(iced::widget::operation::focus(iced::widget::Id::new(
                        "playlist_search_input",
                    )))
                } else {
                    self.ui.playlist_page.search_animation.stop();
                    self.ui.playlist_page.search_query.clear();
                    Some(Task::none())
                }
            }

            Message::PlaylistSearchChanged(query) => {
                self.ui.playlist_page.search_query = query.clone();
                Some(Task::none())
            }

            Message::PlaylistSearchSubmit => {
                // Search is already applied via filtering in view
                // This just handles the Enter key press
                Some(Task::none())
            }

            Message::PlaylistSearchBlur => {
                // If search query is empty and input loses focus, collapse the search
                if self.ui.playlist_page.search_query.is_empty()
                    && self.ui.playlist_page.search_expanded
                {
                    self.ui.playlist_page.search_expanded = false;
                    self.ui.playlist_page.search_animation.stop();
                }
                Some(Task::none())
            }

            _ => None,
        }
    }

    /// Check visible songs and request cover downloads for those missing covers
    /// Returns a Task if there are covers to download, None otherwise
    fn check_visible_song_covers(&mut self) -> Option<Task<Message>> {
        // Only check if we have a playlist and it's an NCM playlist (negative ID)
        let playlist = self.ui.playlist_page.current.as_ref()?;
        if playlist.id >= 0 {
            return None; // Local playlist, no lazy loading needed
        }

        // Get visible range from scroll state
        let scroll_state = self.ui.playlist_page.scroll_state.borrow();
        let (start, end) = scroll_state.visible_range();
        drop(scroll_state);

        // Collect songs that need cover download
        let mut songs_to_download: Vec<(i64, String)> = Vec::new();

        for idx in start..end.min(playlist.songs.len()) {
            let song = &playlist.songs[idx];

            // Skip if already has cover_handle (cover loaded)
            if song.cover_handle.is_some() {
                continue;
            }

            // Skip if no pic_url available
            let pic_url = match &song.pic_url {
                Some(url) if !url.is_empty() => url.clone(),
                _ => continue,
            };

            // Skip if already pending download
            if self
                .ui
                .playlist_page
                .pending_cover_downloads
                .contains(&song.id)
            {
                continue;
            }

            songs_to_download.push((song.id, pic_url));
        }

        if songs_to_download.is_empty() {
            return None;
        }

        // Limit batch size to avoid too many concurrent downloads
        let batch: Vec<_> = songs_to_download.into_iter().take(5).collect();

        Some(Task::done(Message::RequestSongCoversLazy(batch)))
    }
}
