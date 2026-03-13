// src/app/update/search.rs
//! Search message handlers

use iced::Task;

use crate::api::ncm_api::SearchType;
use crate::app::message::{Message, SearchResultsPayload};
use crate::app::state::{App, NavigationEntry, SearchTab};

/// Default number of results per page
const PAGE_SIZE: u32 = 50;

impl App {
    /// Handle search-related messages
    pub fn handle_search(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::SearchSubmit => {
                let keyword = self.ui.search_query.trim().to_string();
                if keyword.is_empty() {
                    return Some(Task::none());
                }

                // Update search state
                self.ui.search.keyword = keyword.clone();
                self.ui.search.current_page = 0;
                self.ui.search.loading = true;
                self.ui.search.active_tab = SearchTab::Songs;

                // Clear previous results
                self.ui.search.songs.clear();
                self.ui.search.albums.clear();
                self.ui.search.playlists.clear();

                // Clear playlist page state
                self.ui.playlist_page.current = None;
                self.ui.playlist_page.viewing_recently_played = false;

                // Push to navigation history
                self.ui
                    .nav_history
                    .push(NavigationEntry::Search(keyword.clone()));

                // Fetch search results
                Some(self.fetch_search_results(keyword, SearchTab::Songs, 0))
            }

            Message::SearchTabChanged(tab) => {
                if self.ui.search.active_tab == *tab {
                    return Some(Task::none());
                }

                self.ui.search.active_tab = *tab;
                self.ui.search.current_page = 0;
                self.ui.search.loading = true;

                let keyword = self.ui.search.keyword.clone();
                Some(self.fetch_search_results(keyword, *tab, 0))
            }

            Message::SearchResultsLoaded(payload) => {
                self.ui.search.loading = false;

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
                }

                Some(Task::none())
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

                self.ui.search.current_page = *page;
                self.ui.search.loading = true;

                let keyword = self.ui.search.keyword.clone();
                let tab = self.ui.search.active_tab;
                Some(self.fetch_search_results(keyword, tab, *page))
            }

            Message::HoverSearchSong(id) => {
                self.ui.search.song_animations.set_hovered_exclusive(*id);
                Some(Task::none())
            }

            Message::HoverSearchCard(id) => {
                self.ui.search.card_animations.set_hovered_exclusive(*id);
                Some(Task::none())
            }

            Message::PlaySearchSong(song_info) => {
                // Convert SongInfo to playable format and play
                tracing::info!("Playing search result: {} - {}", song_info.name, song_info.singer);
                Some(Task::done(Message::PlayNcmSong(song_info.clone())))
            }

            Message::OpenSearchResult(id, tab) => {
                match tab {
                    SearchTab::Albums => {
                        // TODO: Open album detail page
                        tracing::info!("Open album: {}", id);
                    }
                    SearchTab::Playlists => {
                        // Open NCM playlist
                        return Some(Task::done(Message::OpenNcmPlaylist(*id)));
                    }
                    SearchTab::Artists => {
                        // TODO: Open artist page
                        tracing::info!("Open artist: {}", id);
                    }
                    _ => {}
                }
                Some(Task::none())
            }

            _ => None,
        }
    }

    /// Fetch search results from NCM API
    fn fetch_search_results(
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
