// src/app/update/search.rs
//! Search message handlers

use iced::Task;

use crate::api::SongList;
use crate::api::ncm_api::SearchType;
use crate::app::message::{Message, SearchResultsPayload};
use crate::app::state::{App, Route, SearchTab};

/// Default number of results per page
const PAGE_SIZE: u32 = 50;

impl App {
    pub(super) fn clear_search_cover_cache(&mut self) {
        self.ui.search.result_covers.clear();
        self.ui.search.result_cover_allocations.clear();
    }

    fn is_active_search_cover(&self, tab: SearchTab, id: u64) -> bool {
        matches!(self.ui.current_route, Route::Search { .. })
            && match tab {
                SearchTab::Albums | SearchTab::Artists => {
                    self.ui.search.albums.iter().any(|item| item.id == id)
                }
                SearchTab::Playlists => self.ui.search.playlists.iter().any(|item| item.id == id),
                SearchTab::Songs => false,
            }
    }

    /// Handle search-related messages
    pub fn handle_search(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::SearchSubmit => {
                let Some(route) = self.route_for_message(message) else {
                    return Some(Task::none());
                };

                Some(self.navigate_to_route(route, true))
            }

            Message::SearchTabChanged(tab) => {
                if self.ui.search.active_tab == *tab {
                    return Some(Task::none());
                }

                let route = Route::Search {
                    keyword: self.ui.search.keyword.clone(),
                    tab: *tab,
                    page: 0,
                };
                Some(self.navigate_to_route(route, false))
            }

            Message::SearchResultsLoaded(payload) => {
                self.ui.search.loading = false;
                self.clear_search_cover_cache();

                let cover_task = match payload.tab {
                    SearchTab::Songs => {
                        self.ui.search.songs = payload.songs.clone();
                        self.ui.search.total_count = payload.total_count;
                        Task::none()
                    }
                    SearchTab::Artists | SearchTab::Albums => {
                        self.ui.search.albums = payload.albums.clone();
                        self.ui.search.total_count = payload.total_count;
                        self.load_result_covers(payload.tab, &payload.albums)
                    }
                    SearchTab::Playlists => {
                        self.ui.search.playlists = payload.playlists.clone();
                        self.ui.search.total_count = payload.total_count;
                        self.load_result_covers(payload.tab, &payload.playlists)
                    }
                };

                Some(cover_task)
            }

            Message::SearchFailed(error) => {
                self.ui.search.loading = false;
                tracing::error!("Search failed: {}", error);
                Some(Task::done(Message::ShowErrorToast(format!(
                    "搜索失败: {}",
                    error
                ))))
            }

            Message::SearchPageChanged(page) => {
                if self.ui.search.current_page == *page {
                    return Some(Task::none());
                }

                let route = Route::Search {
                    keyword: self.ui.search.keyword.clone(),
                    tab: self.ui.search.active_tab,
                    page: *page,
                };
                Some(self.navigate_to_route(route, false))
            }

            Message::HoverSearchSong(id) => {
                self.ui.search.song_animations.set_hovered_exclusive(*id);
                Some(Task::none())
            }

            Message::HoverSearchCard(id) => {
                self.ui.search.card_animations.set_hovered_exclusive(*id);
                Some(Task::none())
            }

            Message::SearchCoverLoaded(tab, id, path) => {
                let key = (*tab, *id);
                if !self.is_active_search_cover(*tab, *id) {
                    return Some(Task::none());
                }
                let handle = iced::widget::image::Handle::from_path(path);
                self.ui.search.result_covers.insert(key, handle.clone());
                let tab = *tab;
                let id = *id;

                Some(
                    iced::widget::image::allocate(handle)
                        .map(move |result| Message::SearchCoverAllocated(tab, id, result)),
                )
            }

            Message::SearchCoverAllocated(tab, id, result) => {
                if !self.is_active_search_cover(*tab, *id) {
                    return Some(Task::none());
                }
                if let Ok(allocation) = result.clone() {
                    self.ui
                        .search
                        .result_cover_allocations
                        .insert((*tab, *id), allocation);
                }
                Some(Task::none())
            }

            Message::PlaySearchSong(song_info) => {
                // Convert SongInfo to playable format and play
                tracing::info!(
                    "Playing search result: {} - {}",
                    song_info.name,
                    song_info.singer
                );
                Some(Task::done(Message::PlayNcmSong(song_info.clone())))
            }

            Message::OpenSearchResult(id, tab) => {
                match tab {
                    SearchTab::Albums => {
                        tracing::info!("Open album: {}", id);
                        return Some(Task::done(Message::OpenAlbum(*id)));
                    }
                    SearchTab::Playlists => {
                        // Open NCM playlist
                        return Some(Task::done(Message::OpenNcmPlaylist(*id)));
                    }
                    SearchTab::Artists => {
                        tracing::info!("Open artist: {}", id);
                        return Some(Task::done(Message::OpenArtist(*id)));
                    }
                    _ => {}
                }
                Some(Task::none())
            }

            _ => None,
        }
    }

    /// Fetch search results from NCM API
    pub(super) fn fetch_results(
        &self,
        keyword: String,
        tab: SearchTab,
        page: u32,
    ) -> Task<Message> {
        let Some(client) = &self.core.ncm_client else {
            return Task::done(Message::SearchFailed("未登录".to_string()));
        };

        let api = client.client.clone();
        let search_type = tab.to_search_type();
        let offset = page * PAGE_SIZE;

        Task::perform(
            async move {
                match api.search(&keyword, search_type, PAGE_SIZE, offset).await {
                    Ok(response) => {
                        let (songs, albums, playlists, total_count) = match search_type {
                            SearchType::Songs => {
                                (response.songs, vec![], vec![], response.song_count)
                            }
                            SearchType::Albums => {
                                (vec![], response.albums, vec![], response.album_count)
                            }
                            SearchType::Artists => {
                                // Artists are stored in albums field
                                (vec![], response.albums, vec![], response.album_count)
                            }
                            SearchType::Playlists => {
                                (vec![], vec![], response.playlists, response.playlist_count)
                            }
                        };

                        Message::SearchResultsLoaded(SearchResultsPayload {
                            tab,
                            songs,
                            albums,
                            playlists,
                            total_count,
                        })
                    }
                    Err(e) => Message::SearchFailed(e.to_string()),
                }
            },
            |msg| msg,
        )
    }

    fn load_result_covers(&mut self, tab: SearchTab, items: &[SongList]) -> Task<Message> {
        let preload_task = self.preload_cached_result_covers(tab, items);
        let download_task = self.download_result_covers(tab, items);
        Task::batch([preload_task, download_task])
    }

    fn preload_cached_result_covers(
        &mut self,
        tab: SearchTab,
        items: &[SongList],
    ) -> Task<Message> {
        let covers_dir = crate::utils::covers_cache_dir();
        let mut allocation_tasks = Vec::new();

        for item in items {
            let key = (tab, item.id);
            if self.ui.search.result_covers.contains_key(&key) {
                continue;
            }

            let Some(cover_stem) = search_cover_stem(tab, item.id) else {
                continue;
            };

            if let Some(cover_path) = crate::utils::find_cached_image(&covers_dir, &cover_stem) {
                let handle = iced::widget::image::Handle::from_path(&cover_path);
                self.ui.search.result_covers.insert(key, handle.clone());
                let item_id = item.id;
                allocation_tasks.push(
                    iced::widget::image::allocate(handle)
                        .map(move |result| Message::SearchCoverAllocated(tab, item_id, result)),
                );
            }
        }

        if allocation_tasks.is_empty() {
            Task::none()
        } else {
            Task::batch(allocation_tasks)
        }
    }

    fn download_result_covers(&self, tab: SearchTab, items: &[SongList]) -> Task<Message> {
        let Some(client) = &self.core.ncm_client else {
            return Task::none();
        };

        let covers_dir = crate::utils::covers_cache_dir();
        let mut tasks = Vec::new();

        for item in items {
            if item.cover_img_url.is_empty() {
                continue;
            }

            let key = (tab, item.id);
            if self.ui.search.result_covers.contains_key(&key) {
                continue;
            }

            let Some(cover_stem) = search_cover_stem(tab, item.id) else {
                continue;
            };

            if crate::utils::find_cached_image(&covers_dir, &cover_stem).is_some() {
                continue;
            }

            let client = client.clone();
            let item_id = item.id;
            let cover_url = item.cover_img_url.clone();
            let cover_path = covers_dir.join(format!("{}.jpg", cover_stem));

            tasks.push(Task::perform(
                async move {
                    crate::utils::download_img(&client, &cover_url, cover_path, 300, 300)
                        .await
                        .map(|path| (item_id, path))
                },
                move |result| {
                    if let Some((id, path)) = result {
                        Message::SearchCoverLoaded(tab, id, path)
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
    }
}

fn search_cover_stem(tab: SearchTab, item_id: u64) -> Option<String> {
    match tab {
        SearchTab::Albums => Some(format!("search_album_{}", item_id)),
        SearchTab::Artists => Some(format!("search_artist_{}", item_id)),
        SearchTab::Playlists => Some(format!("playlist_{}", item_id)),
        SearchTab::Songs => None,
    }
}
