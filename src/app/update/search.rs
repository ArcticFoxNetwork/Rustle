// src/app/update/search.rs
//! Search message handlers

use iced::Task;

use crate::api::ncm_api::SearchType;
use crate::app::message::{Message, SearchResultsPayload};
use crate::app::state::{App, Route, SearchTab};

/// Default number of results per page
const PAGE_SIZE: u32 = 50;

impl App {
    pub(super) fn clear_search_cover_cache(&mut self) {
        // Search result images are stored in the unified ImageState cache.
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

                match payload.tab {
                    SearchTab::Songs => {
                        self.ui.search.songs = payload.songs.clone();
                        self.ui.search.total_count = payload.total_count;
                    }
                    SearchTab::Artists | SearchTab::Albums => {
                        self.ui.search.albums = payload.albums.clone();
                        self.ui.search.total_count = payload.total_count;
                    }
                    SearchTab::Playlists => {
                        self.ui.search.playlists = payload.playlists.clone();
                        self.ui.search.total_count = payload.total_count;
                    }
                };

                Some(Task::none())
            }

            Message::SearchFailed(error) => {
                self.ui.search.loading = false;
                tracing::error!("Search failed: {}", error);
                Some(Self::toast_error(format!("搜索失败: {}", error)))
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

            Message::PlaySearchSong(song_id) => {
                let Some(song_info) = self
                    .ui
                    .search
                    .songs
                    .iter()
                    .find(|song| song.id == *song_id)
                    .cloned()
                else {
                    return Some(Task::none());
                };

                tracing::info!(
                    "Playing search result: {} - {}",
                    song_info.name,
                    song_info.singer
                );
                Some(Task::done(Message::PlayNcmSong(song_info)))
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
}
