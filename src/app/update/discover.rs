//! Discover page message handlers

use iced::Task;
use rand::SeedableRng;
use rand::seq::SliceRandom;
use tracing::{debug, error};

use crate::api::SongList;
use crate::app::message::Message;
use crate::app::state::{App, Route};
use crate::i18n::Key;

/// Get a daily seed based on current date
fn get_daily_seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    // 使用天数作为种子（每天变化）
    now.as_secs() / 86400
}

/// Shuffle playlists using a daily seed for consistent daily randomization
fn shuffle_daily(playlists: &mut [SongList]) {
    let seed = get_daily_seed();
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    playlists.shuffle(&mut rng);
}

impl App {
    /// Handle discover page related messages
    pub fn handle_discover(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::RecommendedPlaylistsLoaded(playlists) => {
                debug!("Loaded {} recommended playlists", playlists.len());

                let locale = &self.core.locale;
                let mut all_playlists = vec![SongList {
                    id: 0, // Special ID for daily recommend
                    name: locale.get(Key::DiscoverDailyRecommend).to_string(),
                    cover_img_url: String::new(),
                    author: locale.get(Key::DiscoverDailyRecommendDesc).to_string(),
                }];
                all_playlists.extend(playlists.clone());

                self.ui.discover.recommended_playlists = all_playlists;
                self.ui.discover.recommended_loading = false;

                // Pre-populate covers from local cache (sync check) and request GPU allocations
                let allocation_task = self.preload_cached_covers(playlists);

                // Download missing covers asynchronously
                let download_task = self.download_discover_covers(playlists);

                Some(Task::batch([allocation_task, download_task]))
            }

            Message::HotPlaylistsLoaded(playlists, has_more) => {
                debug!(
                    "Loaded {} hot playlists, has_more: {}",
                    playlists.len(),
                    has_more
                );

                // For the first batch (offset 0), shuffle with daily seed
                let is_first_batch = self.ui.discover.hot_playlists.is_empty();

                if is_first_batch {
                    // First batch: shuffle and set
                    let mut shuffled = playlists.clone();
                    shuffle_daily(&mut shuffled);
                    self.ui.discover.hot_playlists = shuffled;
                } else {
                    // Subsequent batches: append without shuffling (pagination)
                    self.ui.discover.hot_playlists.extend(playlists.clone());
                }

                self.ui.discover.hot_loading = false;
                self.ui.discover.hot_has_more = *has_more;

                // Update offset for next page
                if *has_more {
                    self.ui.discover.hot_offset += playlists.len() as u16;
                }

                // Pre-populate covers from local cache (sync check) and request GPU allocations
                let allocation_task = self.preload_cached_covers(playlists);

                // Download missing covers asynchronously
                let download_task = self.download_discover_covers(playlists);

                Some(Task::batch([allocation_task, download_task]))
            }

            Message::DiscoverPlaylistCoverLoaded(playlist_id, path) => {
                // Create image handle from path for instant rendering
                let handle = iced::widget::image::Handle::from_path(path);
                self.ui
                    .discover
                    .playlist_covers
                    .insert(*playlist_id, handle.clone());

                // Request GPU allocation to keep the image in GPU memory
                // This prevents re-loading from disk when returning to the page
                let playlist_id = *playlist_id;
                Some(
                    iced::widget::image::allocate(handle)
                        .map(move |result| Message::DiscoverCoverAllocated(playlist_id, result)),
                )
            }

            Message::DiscoverCoverAllocated(playlist_id, result) => {
                // Store the allocation to keep the image in GPU memory
                if let Ok(allocation) = result.clone() {
                    self.ui
                        .discover
                        .playlist_cover_allocations
                        .insert(*playlist_id, allocation);
                }
                Some(Task::none())
            }

            Message::HoverDiscoverPlaylist(playlist_id) => {
                if let Some(id) = playlist_id {
                    self.ui
                        .discover
                        .card_animations
                        .set_hovered_exclusive(Some(*id));
                } else {
                    self.ui.discover.card_animations.set_hovered_exclusive(None);
                }
                Some(Task::none())
            }

            Message::PlayDiscoverPlaylist(playlist_id) => {
                debug!("Playing discover playlist: {}", playlist_id);
                let playlist_id = *playlist_id;

                // Load and play the playlist
                if let Some(client) = &self.core.ncm_client {
                    let client = client.clone();
                    let error_msg = self
                        .core
                        .locale
                        .get(Key::DiscoverPlaylistLoadFailed)
                        .to_string();

                    if playlist_id == 0 {
                        return Some(Task::perform(
                            async move {
                                match client.client.recommend_songs().await {
                                    Ok(songs) if !songs.is_empty() => Some(songs),
                                    Ok(_) => None,
                                    Err(e) => {
                                        error!("Failed to get daily recommend: {}", e);
                                        None
                                    }
                                }
                            },
                            move |songs_opt| {
                                if let Some(songs) = songs_opt {
                                    Message::AddNcmPlaylist(songs, true)
                                } else {
                                    Message::ShowToast(error_msg)
                                }
                            },
                        ));
                    }

                    return Some(Task::perform(
                        async move {
                            match client.client.song_list_detail(playlist_id).await {
                                Ok(detail) => {
                                    // Songs are already included in the detail
                                    if detail.songs.is_empty() {
                                        return None;
                                    }
                                    Some(detail.songs)
                                }
                                Err(e) => {
                                    error!("Failed to get playlist detail: {}", e);
                                    None
                                }
                            }
                        },
                        move |songs_opt| {
                            if let Some(songs) = songs_opt {
                                Message::AddNcmPlaylist(songs, true)
                            } else {
                                Message::ShowToast(error_msg)
                            }
                        },
                    ));
                }
                Some(Task::none())
            }

            Message::LoadMoreHotPlaylists => {
                if self.ui.discover.hot_loading || !self.ui.discover.hot_has_more {
                    return Some(Task::none());
                }

                self.ui.discover.hot_loading = true;

                if let Some(client) = &self.core.ncm_client {
                    let client = client.clone();
                    let offset = self.ui.discover.hot_offset;
                    let limit = 30u16;

                    return Some(Task::perform(
                        async move {
                            match client
                                .client
                                .top_song_list("全部", "hot", offset, limit)
                                .await
                            {
                                Ok(playlists) => {
                                    let has_more = playlists.len() >= limit as usize;
                                    (playlists, has_more)
                                }
                                Err(e) => {
                                    error!("Failed to load more hot playlists: {}", e);
                                    (Vec::new(), false)
                                }
                            }
                        },
                        |(playlists, has_more)| Message::HotPlaylistsLoaded(playlists, has_more),
                    ));
                }
                Some(Task::none())
            }

            Message::SeeAllRecommended => {
                let route = Route::Discover(crate::app::state::DiscoverViewMode::AllRecommended);
                Some(self.navigate_to_route(route, true))
            }

            Message::SeeAllHot => {
                let route = Route::Discover(crate::app::state::DiscoverViewMode::AllHot);
                let needs_more =
                    self.ui.discover.hot_playlists.len() < 30 && self.ui.discover.hot_has_more;
                let nav_task = self.navigate_to_route(route, true);
                if needs_more {
                    Some(Task::batch([
                        nav_task,
                        Task::done(Message::LoadMoreHotPlaylists),
                    ]))
                } else {
                    Some(nav_task)
                }
            }

            _ => None,
        }
    }

    /// Download covers for discover playlists
    fn download_discover_covers(&self, playlists: &[SongList]) -> Task<Message> {
        if let Some(client) = &self.core.ncm_client {
            let mut tasks = Vec::new();
            let covers_dir = crate::utils::covers_cache_dir();

            for playlist in playlists.iter() {
                // Skip if already in memory cache
                if self.ui.discover.playlist_covers.contains_key(&playlist.id) {
                    continue;
                }

                // Skip if already cached on disk (will be loaded by preload_cached_covers)
                let cover_stem = format!("playlist_{}", playlist.id);
                if crate::utils::find_cached_image(&covers_dir, &cover_stem).is_some() {
                    continue;
                }

                let client = client.clone();
                let playlist_id = playlist.id;
                let cover_url = playlist.cover_img_url.clone();

                tasks.push(Task::perform(
                    async move {
                        crate::utils::download_playlist_cover(&client, playlist_id, &cover_url)
                            .await
                            .map(|path| (playlist_id, path))
                    },
                    |result| {
                        if let Some((id, path)) = result {
                            Message::DiscoverPlaylistCoverLoaded(id, path)
                        } else {
                            Message::NoOp
                        }
                    },
                ));
            }

            if tasks.is_empty() {
                Task::none()
            } else {
                Task::batch(tasks)
            }
        } else {
            Task::none()
        }
    }

    /// Pre-populate covers from local disk cache (synchronous)
    /// Returns a task to allocate the images in GPU memory
    fn preload_cached_covers(&mut self, playlists: &[SongList]) -> Task<Message> {
        let covers_dir = crate::utils::covers_cache_dir();
        let mut allocation_tasks = Vec::new();

        for playlist in playlists.iter() {
            // Skip if already in memory
            if self.ui.discover.playlist_covers.contains_key(&playlist.id) {
                continue;
            }

            // Check disk cache and create image handle
            let cover_stem = format!("playlist_{}", playlist.id);
            if let Some(cover_path) = crate::utils::find_cached_image(&covers_dir, &cover_stem) {
                let handle = iced::widget::image::Handle::from_path(&cover_path);
                self.ui
                    .discover
                    .playlist_covers
                    .insert(playlist.id, handle.clone());

                // Request GPU allocation to keep the image in GPU memory
                let playlist_id = playlist.id;
                allocation_tasks.push(
                    iced::widget::image::allocate(handle)
                        .map(move |result| Message::DiscoverCoverAllocated(playlist_id, result)),
                );
            }
        }

        if allocation_tasks.is_empty() {
            Task::none()
        } else {
            Task::batch(allocation_tasks)
        }
    }
}
