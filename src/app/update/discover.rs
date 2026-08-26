//! Discover page message handlers

use iced::Task;
use rand::SeedableRng;
use rand::seq::SliceRandom;
use tracing::{debug, error};

use crate::api::{PlaylistSummary, UserSummary};
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
fn shuffle_daily(playlists: &mut [PlaylistSummary]) {
    let seed = get_daily_seed();
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    playlists.shuffle(&mut rng);
}

impl App {
    pub(super) fn clear_discover_cover_cache(&mut self) {
        // Discover images are stored in the unified ImageState cache.
    }

    /// Handle discover page related messages
    pub fn handle_discover(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::RecommendedPlaylistsLoaded(playlists) => {
                debug!("Loaded {} recommended playlists", playlists.len());

                let locale = &self.core.locale;
                let mut all_playlists = vec![PlaylistSummary {
                    id: 0, // Special ID for daily recommend
                    name: locale.get(Key::DiscoverDailyRecommend).to_string(),
                    cover_url: String::new(),
                    creator: UserSummary {
                        id: 0,
                        nickname: locale.get(Key::DiscoverDailyRecommendDesc).to_string(),
                        avatar_url: String::new(),
                        vip: crate::api::VipInfo::default(),
                    },
                    subscribed: false,
                }];
                all_playlists.extend(playlists.clone());

                self.ui.discover.recommended_playlists = all_playlists;
                self.ui.discover.recommended_loading = false;

                Some(Task::none())
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
                                match client.recommend_tracks().await {
                                    Ok(tracks) if !tracks.is_empty() => Some(tracks),
                                    Ok(_) => None,
                                    Err(e) => {
                                        error!("Failed to get daily recommend: {}", e);
                                        None
                                    }
                                }
                            },
                            move |tracks_opt| {
                                if let Some(tracks) = tracks_opt {
                                    Message::AddNcmPlaylist(tracks, true)
                                } else {
                                    Message::ShowErrorToast(error_msg)
                                }
                            },
                        ));
                    }

                    return Some(Task::perform(
                        async move {
                            match client.playlist_detail(playlist_id).await {
                                Ok(detail) => {
                                    // Tracks are already included in the detail
                                    if detail.tracks.is_empty() {
                                        return None;
                                    }
                                    Some((detail.id, detail.tracks))
                                }
                                Err(e) => {
                                    error!("Failed to get playlist detail: {}", e);
                                    None
                                }
                            }
                        },
                        move |detail_opt| {
                            if let Some((detail_id, tracks)) = detail_opt {
                                Message::AddNcmPlaylistWithSource(tracks, true, Some(detail_id))
                            } else {
                                Message::ShowErrorToast(error_msg)
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
                            match client.top_playlists("全部", "hot", offset, limit).await {
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
}
