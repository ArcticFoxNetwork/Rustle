//! NCM (Netease Cloud Music) related message handlers

use iced::Task;
use std::time::Duration;
use tracing::{debug, error, info};

use crate::api::{LoginInfo, NcmClient};
use crate::app::message::QrLoginStatus;
use crate::app::state::UserInfo;
use crate::app::{App, Message, Route};
use crate::i18n::Key;

fn artist_page_id(artist_id: u64) -> i64 {
    i64::MIN + artist_id as i64
}

fn user_page_id(user_id: u64) -> i64 {
    (i64::MIN / 2) + user_id as i64
}

fn album_page_id(album_id: u64) -> i64 {
    (i64::MIN / 4) + album_id as i64
}

fn find_cached_album_cover(album_id: u64) -> Option<String> {
    let covers_dir = crate::utils::covers_cache_dir();
    [
        format!("search_album_{}", album_id),
        format!("album_{}", album_id),
    ]
    .into_iter()
    .find_map(|stem| crate::utils::find_cached_image(&covers_dir, &stem))
    .map(|path| path.to_string_lossy().to_string())
}

fn format_social_count(value: u64) -> String {
    if value >= 10_000 {
        format!("{:.1}万", value as f64 / 10_000.0)
    } else {
        value.to_string()
    }
}

impl App {
    fn ncm_song_to_db_song(song_info: &crate::api::SongInfo) -> crate::database::DbSong {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        crate::database::DbSong {
            id: -(song_info.id as i64),
            file_path: String::new(),
            title: song_info.name.clone(),
            artist: song_info.singer.clone(),
            album: song_info.album.clone(),
            duration_secs: (song_info.duration / 1000) as i64,
            track_number: None,
            year: None,
            genre: None,
            cover_path: if song_info.pic_url.is_empty() {
                None
            } else {
                Some(song_info.pic_url.clone())
            },
            file_hash: None,
            file_size: 0,
            format: Some("mp3".to_string()),
            normalization_gain: None,
            play_count: 0,
            last_played: Some(now),
            last_modified: now,
            is_missing: false,
            created_at: now,
        }
    }

    /// Set the NCM client and sync quality settings
    fn set_ncm_client(&mut self, client: NcmClient) {
        client.set_quality(self.core.settings.playback.music_quality.to_api_rate());
        self.core.ncm_client = Some(client);
    }

    pub(super) fn start_personal_fm_route(&mut self) -> Task<Message> {
        debug!("Starting Personal FM");

        self.enter_fm_mode();

        if let Some(client) = &self.core.ncm_client {
            let client = client.clone();
            let not_logged_in_msg = self.core.locale.get(Key::NotLoggedIn).to_string();

            Task::perform(
                async move {
                    match client.client.personal_fm().await {
                        Ok(songs) if !songs.is_empty() => Some(songs),
                        Ok(_) => None,
                        Err(e) => {
                            error!("Failed to get personal FM: {}", e);
                            None
                        }
                    }
                },
                move |songs_opt| {
                    if let Some(songs) = songs_opt {
                        Message::AddNcmPlaylist(songs, true)
                    } else {
                        Message::ShowWarningToast(not_logged_in_msg)
                    }
                },
            )
        } else {
            self.exit_fm_mode();
            let msg = self.core.locale.get(Key::NotLoggedIn).to_string();
            Task::done(Message::ShowWarningToast(msg))
        }
    }

    pub(super) fn open_ncm_playlist_route(&mut self, playlist_id: u64) -> Task<Message> {
        let is_daily_recommend = playlist_id == 0;

        if !is_daily_recommend && self.is_viewing_ncm_playlist(playlist_id) {
            debug!(
                "Already viewing NCM playlist {}, skipping load",
                playlist_id
            );
            return Task::none();
        }

        if matches!(
            self.ui.playlist_page.load_state,
            crate::app::update::page_loader::PlaylistLoadState::Loading
        ) {
            debug!("Playlist already loading, skipping");
            return Task::none();
        }

        debug!("Opening NCM playlist: {}", playlist_id);
        self.reset_playlist_page_state();

        let (name, owner, cover_url) = if is_daily_recommend {
            let locale = &self.core.locale;
            (
                locale
                    .get(crate::i18n::Key::DiscoverDailyRecommend)
                    .to_string(),
                locale
                    .get(crate::i18n::Key::DiscoverDailyRecommendCreator)
                    .to_string(),
                String::new(),
            )
        } else {
            self.ui
                .home
                .user_playlists
                .iter()
                .find(|p| p.id == playlist_id)
                .map(|p| (p.name.clone(), p.author.clone(), p.cover_img_url.clone()))
                .unwrap_or_else(|| ("加载中...".to_string(), String::new(), String::new()))
        };

        let internal_id = if is_daily_recommend {
            0
        } else {
            -(playlist_id as i64)
        };

        let skeleton_view = crate::ui::pages::PlaylistView {
            kind: crate::ui::pages::playlist::DetailPageKind::Playlist,
            id: internal_id,
            name,
            description: None,
            profile_stats: None,
            artist_tab: crate::ui::pages::playlist::ArtistPageTab::TopSongs,
            artist_albums: Vec::new(),
            user_playlists: Vec::new(),
            cover_path: None,
            owner,
            owner_artist_id: None,
            owner_avatar_path: None,
            creator_id: 0,
            song_count: 0,
            total_duration: String::new(),
            like_count: String::new(),
            songs: Vec::new(),
            palette: crate::utils::ColorPalette::default(),
            is_local: false,
            is_subscribed: false,
            watched_folder_path: None,
            watch_enabled: false,
        };

        self.ui.playlist_page.current = Some(skeleton_view);
        self.ui.playlist_page.load_state =
            crate::app::update::page_loader::PlaylistLoadState::Loading;

        let cover_task = if !cover_url.is_empty() {
            if let Some(client) = &self.core.ncm_client {
                let client = client.clone();
                Task::perform(
                    async move {
                        crate::utils::download_playlist_cover(&client, playlist_id, &cover_url)
                            .await
                            .map(|p| (internal_id, p.to_string_lossy().to_string()))
                    },
                    |result| {
                        if let Some((id, path)) = result {
                            Message::NcmPlaylistCoverLoaded(id, path)
                        } else {
                            Message::NoOp
                        }
                    },
                )
            } else {
                Task::none()
            }
        } else {
            Task::none()
        };

        let api_task = if let Some(client) = &self.core.ncm_client {
            let client = client.clone();
            if is_daily_recommend {
                let locale = &self.core.locale;
                let name = locale
                    .get(crate::i18n::Key::DiscoverDailyRecommend)
                    .to_string();
                let desc = locale
                    .get(crate::i18n::Key::DiscoverDailyRecommendDesc)
                    .to_string();
                let creator = locale
                    .get(crate::i18n::Key::DiscoverDailyRecommendCreator)
                    .to_string();
                Task::perform(
                    async move {
                        match client.client.recommend_songs().await {
                            Ok(songs) => Some(crate::api::PlayListDetail {
                                id: 0,
                                name,
                                cover_img_url: String::new(),
                                description: desc,
                                create_time: 0,
                                track_update_time: 0,
                                creator_id: 0,
                                creator_nickname: creator,
                                creator_avatar_url: String::new(),
                                track_count: songs.len() as u64,
                                subscribed: false,
                                songs,
                            }),
                            Err(e) => {
                                error!("Failed to load daily recommend: {:?}", e);
                                None
                            }
                        }
                    },
                    move |result| {
                        if let Some(detail) = result {
                            Message::NcmPlaylistDetailLoaded(detail)
                        } else {
                            Message::ShowErrorToast("加载每日推荐失败".to_string())
                        }
                    },
                )
            } else {
                Task::perform(
                    async move {
                        match client.client.song_list_detail(playlist_id).await {
                            Ok(detail) => Some(detail),
                            Err(e) => {
                                error!("Failed to load NCM playlist detail: {:?}", e);
                                None
                            }
                        }
                    },
                    move |result| {
                        if let Some(detail) = result {
                            Message::NcmPlaylistDetailLoaded(detail)
                        } else {
                            Message::ShowErrorToast("加载歌单失败".to_string())
                        }
                    },
                )
            }
        } else {
            Task::none()
        };

        Task::batch([cover_task, api_task])
    }

    pub(super) fn open_album_route(&mut self, album_id: u64) -> Task<Message> {
        let route = Route::Album(album_id);
        if self.ui.current_route != route {
            return self.navigate_to_route(route, true);
        }

        let page_id = album_page_id(album_id);
        if self.ui.playlist_page.current.as_ref().map(|page| page.id) == Some(page_id)
            && !matches!(
                self.ui.playlist_page.load_state,
                crate::app::update::page_loader::PlaylistLoadState::Loading
            )
        {
            debug!("Already viewing album {}, skipping load", album_id);
            return Task::none();
        }

        if matches!(
            self.ui.playlist_page.load_state,
            crate::app::update::page_loader::PlaylistLoadState::Loading
        ) {
            debug!("Album page already loading, skipping");
            return Task::none();
        }

        let preview = self
            .ui
            .playlist_page
            .current
            .as_ref()
            .and_then(|page| {
                page.artist_albums
                    .iter()
                    .find(|album| album.id == album_id)
                    .map(|album| {
                        (
                            album.name.clone(),
                            album.author.clone(),
                            album.cover_img_url.clone(),
                            page.owner_artist_id,
                        )
                    })
            })
            .or_else(|| {
                self.ui
                    .search
                    .albums
                    .iter()
                    .find(|album| album.id == album_id)
                    .map(|album| {
                        (
                            album.name.clone(),
                            album.author.clone(),
                            album.cover_img_url.clone(),
                            None,
                        )
                    })
            });

        let (name, owner, cover_url, owner_artist_id) = preview
            .unwrap_or_else(|| ("加载中...".to_string(), String::new(), String::new(), None));

        debug!("Opening album page: {}", album_id);
        self.reset_playlist_page_state();

        let cover_path = find_cached_album_cover(album_id);
        let palette = cover_path
            .as_deref()
            .map(|path| crate::utils::ColorPalette::from_image_path(std::path::Path::new(path)))
            .unwrap_or_default();

        let skeleton_view = crate::ui::pages::PlaylistView {
            kind: crate::ui::pages::playlist::DetailPageKind::Album,
            id: page_id,
            name,
            description: None,
            profile_stats: None,
            artist_tab: crate::ui::pages::playlist::ArtistPageTab::TopSongs,
            artist_albums: Vec::new(),
            user_playlists: Vec::new(),
            cover_path,
            owner,
            owner_artist_id,
            owner_avatar_path: None,
            creator_id: 0,
            song_count: 0,
            total_duration: String::new(),
            like_count: String::new(),
            songs: Vec::new(),
            palette,
            is_local: false,
            is_subscribed: false,
            watched_folder_path: None,
            watch_enabled: false,
        };

        self.ui.playlist_page.current = Some(skeleton_view);
        self.ui.playlist_page.load_state =
            crate::app::update::page_loader::PlaylistLoadState::Loading;

        let cover_task = if self
            .ui
            .playlist_page
            .current
            .as_ref()
            .and_then(|page| page.cover_path.as_ref())
            .is_none()
            && !cover_url.is_empty()
        {
            if let Some(client) = &self.core.ncm_client {
                let client = client.clone();
                let cover_path =
                    crate::utils::covers_cache_dir().join(format!("search_album_{}.jpg", album_id));
                Task::perform(
                    async move {
                        crate::utils::download_img(&client, &cover_url, cover_path, 300, 300)
                            .await
                            .map(|path| (page_id, path.to_string_lossy().to_string()))
                    },
                    |result| {
                        if let Some((page_id, path)) = result {
                            Message::AlbumCoverLoaded(page_id, path)
                        } else {
                            Message::NoOp
                        }
                    },
                )
            } else {
                Task::none()
            }
        } else {
            Task::none()
        };

        if let Some(client) = &self.core.ncm_client {
            let client = client.clone();
            let api_task = Task::perform(
                async move { client.client.album_detail(album_id).await.ok() },
                |result| {
                    if let Some(detail) = result {
                        Message::AlbumDetailLoaded(detail)
                    } else {
                        Message::ShowErrorToast("加载专辑失败".to_string())
                    }
                },
            );
            return Task::batch([cover_task, api_task]);
        }

        Task::done(Message::ShowWarningToast("请先登录".to_string()))
    }

    pub(super) fn open_artist_route(&mut self, artist_id: u64) -> Task<Message> {
        let route = Route::Artist(artist_id);
        if self.ui.current_route != route {
            return self.navigate_to_route(route, true);
        }

        if matches!(
            self.ui.playlist_page.load_state,
            crate::app::update::page_loader::PlaylistLoadState::Loading
        ) {
            debug!("Artist page already loading, skipping");
            return Task::none();
        }

        debug!("Opening artist page: {}", artist_id);
        self.reset_playlist_page_state();

        let internal_id = artist_page_id(artist_id);
        let skeleton_view = crate::ui::pages::PlaylistView {
            kind: crate::ui::pages::playlist::DetailPageKind::Artist,
            id: internal_id,
            name: "加载中...".to_string(),
            description: None,
            profile_stats: Some("歌手".to_string()),
            artist_tab: crate::ui::pages::playlist::ArtistPageTab::TopSongs,
            artist_albums: Vec::new(),
            user_playlists: Vec::new(),
            cover_path: None,
            owner: "网易云音乐".to_string(),
            owner_artist_id: Some(artist_id),
            owner_avatar_path: None,
            creator_id: 0,
            song_count: 0,
            total_duration: String::new(),
            like_count: String::new(),
            songs: Vec::new(),
            palette: crate::utils::ColorPalette::default(),
            is_local: false,
            is_subscribed: false,
            watched_folder_path: None,
            watch_enabled: false,
        };

        self.ui.playlist_page.current = Some(skeleton_view);
        self.ui.playlist_page.load_state =
            crate::app::update::page_loader::PlaylistLoadState::Loading;

        if let Some(client) = &self.core.ncm_client {
            let client = client.clone();
            return Task::perform(
                async move { client.client.artist_detail(artist_id).await.ok() },
                |result| {
                    if let Some(detail) = result {
                        Message::ArtistDetailLoaded(detail)
                    } else {
                        Message::ShowErrorToast("加载歌手失败".to_string())
                    }
                },
            );
        }

        Task::done(Message::ShowWarningToast("请先登录".to_string()))
    }

    pub(super) fn open_user_route(&mut self, user_id: u64) -> Task<Message> {
        let route = Route::User(user_id);
        if self.ui.current_route != route {
            return self.navigate_to_route(route, true);
        }

        if matches!(
            self.ui.playlist_page.load_state,
            crate::app::update::page_loader::PlaylistLoadState::Loading
        ) {
            debug!("User page already loading, skipping");
            return Task::none();
        }

        debug!("Opening user page: {}", user_id);
        self.reset_playlist_page_state();

        let page_id = user_page_id(user_id);
        let skeleton_view = crate::ui::pages::PlaylistView {
            kind: crate::ui::pages::playlist::DetailPageKind::User,
            id: page_id,
            name: "加载中...".to_string(),
            description: None,
            profile_stats: Some("关注 0 · 粉丝 0".to_string()),
            artist_tab: crate::ui::pages::playlist::ArtistPageTab::TopSongs,
            artist_albums: Vec::new(),
            user_playlists: Vec::new(),
            cover_path: None,
            owner: "网易云用户".to_string(),
            owner_artist_id: None,
            owner_avatar_path: None,
            creator_id: user_id,
            song_count: 0,
            total_duration: String::new(),
            like_count: String::new(),
            songs: Vec::new(),
            palette: crate::utils::ColorPalette::default(),
            is_local: false,
            is_subscribed: false,
            watched_folder_path: None,
            watch_enabled: false,
        };

        self.ui.playlist_page.current = Some(skeleton_view);
        self.ui.playlist_page.load_state =
            crate::app::update::page_loader::PlaylistLoadState::Loading;

        if let Some(client) = &self.core.ncm_client {
            let client = client.clone();
            return Task::perform(
                async move {
                    client
                        .client
                        .user_detail(user_id)
                        .await
                        .ok()
                        .map(|detail| (page_id, detail))
                },
                |result| {
                    if let Some((page_id, detail)) = result {
                        Message::UserPageDetailLoaded(page_id, detail)
                    } else {
                        Message::ShowErrorToast("加载用户失败".to_string())
                    }
                },
            );
        }

        Task::done(Message::ShowWarningToast("请先登录".to_string()))
    }

    /// Handle NCM-related messages
    pub fn handle_ncm(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::TryAutoLogin(retry_count) => {
                let retry = *retry_count;
                let proxy_url = self.core.settings.network.proxy_url();
                if let Some((cookie_jar, csrf_token)) = NcmClient::load_cookie_jar_from_file() {
                    let client =
                        NcmClient::from_cookie_jar_with_proxy(cookie_jar, csrf_token, proxy_url);
                    self.set_ncm_client(client.clone());

                    Some(Task::perform(
                        async move {
                            match client.client.login_status().await {
                                Ok(login_info) => Some(login_info),
                                Err(e) => {
                                    error!("Auto login failed (attempt {}): {:?}", retry + 1, e);
                                    None
                                }
                            }
                        },
                        move |result| Message::AutoLoginResult(result, retry),
                    ))
                } else {
                    self.set_ncm_client(NcmClient::with_proxy(proxy_url));
                    Some(self.load_homepage_data())
                }
            }

            Message::AutoLoginResult(login_info_opt, retry_count) => {
                if let Some(login_info) = login_info_opt {
                    debug!("Auto login successful: {:?}", login_info);
                    self.core.is_logged_in = true;

                    if let Some(client) = &self.core.ncm_client {
                        client.save_cookie_jar_to_file();
                    }

                    let mut user_info = UserInfo::new(
                        login_info.uid,
                        login_info.nickname.clone(),
                        login_info.avatar_url.clone(),
                    );
                    user_info.vip_type = login_info.vip_type;
                    self.core.user_info = Some(user_info);

                    let client = self.core.ncm_client.clone();
                    let uid = login_info.uid;
                    let avatar_url = login_info.avatar_url.clone();

                    Some(Task::batch([
                        self.load_homepage_data(),
                        Task::perform(
                            {
                                let client = client.clone();
                                async move {
                                    if let Some(client) = client {
                                        if let Ok(song_ids) =
                                            client.client.user_song_id_list(uid).await
                                        {
                                            let mut user_info =
                                                UserInfo::new(uid, String::new(), String::new());
                                            user_info.like_songs = song_ids.into_iter().collect();
                                            return user_info;
                                        }
                                    }
                                    UserInfo::new(uid, String::new(), String::new())
                                }
                            },
                            Message::UserInfoLoaded,
                        ),
                        Task::perform(
                            {
                                let client = client.clone();
                                async move {
                                    if let Some(client) = client {
                                        crate::utils::download_avatar(&client, uid, &avatar_url)
                                            .await
                                    } else {
                                        None
                                    }
                                }
                            },
                            |path_opt| {
                                if let Some(path) = path_opt {
                                    Message::UserAvatarLoaded(path)
                                } else {
                                    Message::NoOp
                                }
                            },
                        ),
                        self.load_user_playlists(),
                    ]))
                } else {
                    // Auto login failed - retry up to 3 times
                    const MAX_RETRIES: u8 = 3;
                    let retry = *retry_count;
                    if retry < MAX_RETRIES {
                        info!(
                            "Auto login failed, retrying ({}/{})",
                            retry + 1,
                            MAX_RETRIES
                        );
                        // Wait 1 seconds before retry
                        Some(Task::perform(
                            async move {
                                tokio::time::sleep(Duration::from_secs(1)).await;
                            },
                            move |_| Message::TryAutoLogin(retry + 1),
                        ))
                    } else {
                        info!(
                            "Auto login failed after {} retries, keeping cookie for next launch",
                            MAX_RETRIES
                        );
                        Some(self.load_homepage_data())
                    }
                }
            }

            Message::RequestQrCode => {
                self.ui.home.login_popup_open = true;
                self.ui.home.qr_status = Some("正在生成二维码...".to_string());
                // Clear old QR code data to force refresh
                self.ui.home.qr_code_path = None;
                self.ui.home.qr_unikey = None;

                let client = self.core.ncm_client.clone().unwrap_or_default();

                Some(Task::perform(
                    async move {
                        match client.create_qrcode().await {
                            Ok((path, unikey)) => Some((path, unikey)),
                            Err(e) => {
                                error!("Failed to create QR code: {:?}", e);
                                None
                            }
                        }
                    },
                    |result| {
                        if let Some((path, unikey)) = result {
                            Message::QrCodeReady(path, unikey)
                        } else {
                            Message::ShowErrorToast("生成二维码失败".to_string())
                        }
                    },
                ))
            }

            Message::QrCodeReady(path, unikey) => {
                self.ui.home.qr_code_path = Some(path.clone());
                self.ui.home.qr_unikey = Some(unikey.clone());
                self.ui.home.qr_status = Some("请使用网易云音乐App扫码登录".to_string());

                let unikey = unikey.clone();
                Some(Task::done(Message::CheckQrStatus(unikey)))
            }

            Message::CheckQrStatus(unikey) => {
                let current_unikey = self.ui.home.qr_unikey.clone();
                if current_unikey.as_ref() != Some(unikey) {
                    return Some(Task::none());
                }

                let client = self.core.ncm_client.clone().unwrap_or_default();
                let unikey = unikey.clone();

                Some(Task::perform(
                    async move {
                        match client.client.login_qr_check(unikey.clone()).await {
                            Ok(msg) => match msg.code {
                                800 => QrLoginStatus::Expired,
                                801 => QrLoginStatus::WaitingForScan,
                                802 => QrLoginStatus::WaitingForConfirm,
                                803 => QrLoginStatus::Success,
                                _ => QrLoginStatus::Error(format!("Unknown code: {}", msg.code)),
                            },
                            Err(e) => QrLoginStatus::Error(e.to_string()),
                        }
                    },
                    Message::QrLoginResult,
                ))
            }

            Message::QrLoginResult(status) => match status {
                QrLoginStatus::WaitingForScan => {
                    self.ui.home.qr_status = Some("等待扫码...".to_string());
                    let unikey = self.ui.home.qr_unikey.clone();
                    if let Some(unikey) = unikey {
                        Some(Task::perform(
                            async move {
                                tokio::time::sleep(Duration::from_secs(2)).await;
                                unikey
                            },
                            Message::CheckQrStatus,
                        ))
                    } else {
                        Some(Task::none())
                    }
                }
                QrLoginStatus::WaitingForConfirm => {
                    self.ui.home.qr_status = Some("已扫码，请在App中确认登录".to_string());
                    let unikey = self.ui.home.qr_unikey.clone();
                    if let Some(unikey) = unikey {
                        Some(Task::perform(
                            async move {
                                tokio::time::sleep(Duration::from_secs(2)).await;
                                unikey
                            },
                            Message::CheckQrStatus,
                        ))
                    } else {
                        Some(Task::none())
                    }
                }
                QrLoginStatus::Expired => {
                    self.ui.home.qr_status = Some("二维码已过期，请刷新".to_string());
                    self.ui.home.login_popup_open = false;
                    Some(Task::done(Message::ShowErrorToast(
                        "二维码已过期".to_string(),
                    )))
                }
                QrLoginStatus::Success => {
                    self.ui.home.qr_status = Some("登录成功！".to_string());

                    if let Some(client) = &self.core.ncm_client {
                        let client = client.clone();
                        return Some(Task::perform(
                            async move {
                                match client.client.login_status().await {
                                    Ok(login_info) => {
                                        client.save_cookie_jar_to_file();
                                        login_info
                                    }
                                    Err(e) => {
                                        error!("Failed to get login status: {:?}", e);
                                        LoginInfo::default()
                                    }
                                }
                            },
                            Message::LoginSuccess,
                        ));
                    }
                    Some(Task::none())
                }
                QrLoginStatus::Error(err) => {
                    self.ui.home.qr_status = Some(format!("登录错误: {}", err));
                    self.ui.home.login_popup_open = false;
                    Some(Task::done(Message::ShowErrorToast(format!(
                        "登录失败: {}",
                        err
                    ))))
                }
            },

            Message::LoginSuccess(login_info) => {
                debug!("Login successful: {:?}", login_info);
                self.core.is_logged_in = true;
                self.ui.home.login_popup_open = false;

                let mut user_info = UserInfo::new(
                    login_info.uid,
                    login_info.nickname.clone(),
                    login_info.avatar_url.clone(),
                );
                user_info.vip_type = login_info.vip_type;
                self.core.user_info = Some(user_info);

                let client = self.core.ncm_client.clone();
                let uid = login_info.uid;
                let avatar_url = login_info.avatar_url.clone();

                Some(Task::batch([
                    Task::done(Message::ShowSuccessToast("登录成功！".to_string())),
                    self.load_homepage_data(),
                    Task::perform(
                        async move {
                            if let Some(client) = client {
                                crate::utils::download_avatar(&client, uid, &avatar_url).await
                            } else {
                                None
                            }
                        },
                        |path_opt| {
                            if let Some(path) = path_opt {
                                Message::UserAvatarLoaded(path)
                            } else {
                                Message::NoOp
                            }
                        },
                    ),
                    self.load_user_playlists(),
                ]))
            }

            Message::UserAvatarLoaded(path) => {
                if let Some(user_info) = &mut self.core.user_info {
                    user_info.avatar_path = Some(path.clone());
                    // Create image handle for instant rendering
                    user_info.avatar_handle = Some(iced::widget::image::Handle::from_path(path));
                }
                Some(Task::none())
            }

            Message::Logout => {
                if let Some(client) = &self.core.ncm_client {
                    let client = client.clone();
                    tokio::spawn(async move {
                        client.client.logout().await;
                    });
                }

                NcmClient::clean_cookie_file();
                self.core.is_logged_in = false;
                self.core.user_info = None;
                let proxy_url = self.core.settings.network.proxy_url();
                self.set_ncm_client(NcmClient::with_proxy(proxy_url));

                Some(Task::done(Message::ShowSuccessToast(
                    "已退出登录".to_string(),
                )))
            }

            Message::UserInfoLoaded(user_info) => {
                if let Some(existing) = &mut self.core.user_info {
                    existing.like_songs = user_info.like_songs.clone();
                } else {
                    self.core.user_info = Some(user_info.clone());
                }
                Some(Task::none())
            }

            Message::ToggleLoginPopup => {
                self.ui.home.login_popup_open = !self.ui.home.login_popup_open;
                if self.ui.home.login_popup_open && self.ui.home.qr_code_path.is_none() {
                    Some(Task::done(Message::RequestQrCode))
                } else {
                    Some(Task::none())
                }
            }

            Message::BannersLoaded(banners) => {
                self.ui.home.banners = banners.clone();
                self.ui.home.current_banner = 0;

                if let Some(client) = &self.core.ncm_client {
                    let mut tasks = Vec::new();
                    for (index, banner) in banners.iter().enumerate() {
                        let client = client.clone();
                        let pic_url = banner.pic.clone();
                        let target_id = banner.target_id;

                        tasks.push(Task::perform(
                            async move {
                                if let Some(path) =
                                    crate::utils::download_banner(&client, target_id, &pic_url)
                                        .await
                                {
                                    match image::ImageReader::open(&path)
                                        .and_then(|r| r.with_guessed_format())
                                    {
                                        Ok(reader) => match reader.into_dimensions() {
                                            Ok((w, h)) => Some((index, path, w, h)),
                                            Err(e) => {
                                                error!("Failed to get banner dimensions: {}", e);
                                                Some((index, path, 0, 0))
                                            }
                                        },
                                        Err(e) => {
                                            error!("Failed to open banner for dimensions: {}", e);
                                            Some((index, path, 0, 0))
                                        }
                                    }
                                } else {
                                    None
                                }
                            },
                            |result| {
                                if let Some((idx, path, w, h)) = result {
                                    Message::BannerImageLoaded(idx, path, w, h)
                                } else {
                                    Message::NoOp
                                }
                            },
                        ));
                    }
                    Some(Task::batch(tasks))
                } else {
                    Some(Task::none())
                }
            }

            Message::BannerImageLoaded(index, path, width, height) => {
                // Canvas uses PathBuf directly, iced handles caching internally
                self.ui
                    .home
                    .banner_images
                    .insert(*index, (path.clone(), *width, *height));
                Some(Task::none())
            }

            Message::BannerPlay(index) => {
                if let Some(banner) = self.ui.home.banners.get(*index) {
                    debug!(
                        "Playing banner {}: {} (Type: {:?}, ID: {})",
                        index, banner.type_title, banner.target_type, banner.target_id
                    );

                    match banner.target_type {
                        crate::api::TargetType::Song => {
                            let song_id = banner.target_id;
                            if let Some(client) = &self.core.ncm_client {
                                let client = client.clone();
                                return Some(Task::perform(
                                    async move {
                                        match client.song_detail(&[song_id]).await {
                                            Ok(songs) => songs.first().cloned(),
                                            Err(e) => {
                                                error!("Failed to get banner song detail: {}", e);
                                                None
                                            }
                                        }
                                    },
                                    |song_opt| {
                                        if let Some(song) = song_opt {
                                            Message::PlayNcmSong(song)
                                        } else {
                                            Message::ShowErrorToast("无法获取歌曲信息".to_string())
                                        }
                                    },
                                ));
                            }
                        }
                        crate::api::TargetType::Album => {
                            debug!("Album playback from banner not implemented yet");
                        }
                        _ => {
                            debug!("Unsupported banner target type: {:?}", banner.target_type);
                        }
                    }
                }
                Some(Task::none())
            }

            Message::ToggleBannerFavorite(index) => {
                if let Some(banner) = self.ui.home.banners.get(*index) {
                    match banner.target_type {
                        crate::api::TargetType::Song => {
                            return Some(self.update(Message::ToggleFavorite(banner.target_id)));
                        }
                        _ => {
                            debug!(
                                "Favorite not implemented for banner type: {:?}",
                                banner.target_type
                            );
                        }
                    }
                }
                Some(Task::none())
            }

            Message::CarouselTick => {
                if !self.ui.home.banners.is_empty() {
                    let now = iced::time::Instant::now();

                    self.ui.home.last_banner = self.ui.home.current_banner;
                    self.ui.home.current_banner =
                        (self.ui.home.current_banner + 1) % self.ui.home.banners.len();

                    self.ui.home.carousel_direction = 1;
                    self.ui.home.carousel_animation = iced::animation::Animation::new(false).slow();
                    self.ui.home.carousel_animation.go_mut(true, now);
                }
                Some(Task::none())
            }

            Message::CarouselNavigate(delta) => {
                if !self.ui.home.banners.is_empty() {
                    let now = iced::time::Instant::now();

                    self.ui.home.last_banner = self.ui.home.current_banner;
                    let len = self.ui.home.banners.len() as i32;
                    let current = self.ui.home.current_banner as i32;
                    let new_index = ((current + *delta) % len + len) % len;
                    self.ui.home.current_banner = new_index as usize;

                    self.ui.home.carousel_direction = *delta;
                    self.ui.home.carousel_animation = iced::animation::Animation::new(false).slow();
                    self.ui.home.carousel_animation.go_mut(true, now);
                }
                Some(Task::none())
            }

            Message::TopPicksLoaded(playlists) => {
                self.ui.home.top_picks = playlists.clone();
                Some(Task::none())
            }

            Message::TrendingSongsLoaded(songs) => {
                self.ui.home.trending_songs = songs.clone();

                if let Some(client) = &self.core.ncm_client {
                    let mut tasks = Vec::new();
                    for song in songs.iter().take(10) {
                        let client = client.clone();
                        let pic_url = song.pic_url.clone();
                        let song_id = song.id;

                        tasks.push(Task::perform(
                            async move {
                                if let Some(path) =
                                    crate::utils::download_cover(&client, song_id, &pic_url).await
                                {
                                    Some((song_id, path))
                                } else {
                                    None
                                }
                            },
                            |result| {
                                if let Some((id, path)) = result {
                                    Message::SongCoverLoaded(id, path)
                                } else {
                                    Message::NoOp
                                }
                            },
                        ));
                    }
                    Some(Task::batch(tasks))
                } else {
                    Some(Task::none())
                }
            }

            Message::OpenTrendingSongs => Some(self.navigate_to_route(
                Route::Discover(crate::app::state::DiscoverViewMode::AllHot),
                true,
            )),

            Message::SongCoverLoaded(song_id, path) => {
                // Create image handle for instant rendering (no disk IO in render loop)
                let handle = iced::widget::image::Handle::from_path(path);
                self.ui.home.song_covers.insert(*song_id, handle);
                Some(Task::none())
            }

            Message::ToggleFavorite(song_id) => {
                if !self.core.is_logged_in {
                    return Some(Task::done(Message::ShowWarningToast(
                        "请先登录".to_string(),
                    )));
                }

                if let Some(client) = &self.core.ncm_client {
                    let client = client.clone();
                    let is_liked = if let Some(ref user_info) = self.core.user_info {
                        user_info.like_songs.contains(&song_id)
                    } else {
                        false
                    };
                    let song_id = *song_id;

                    Some(Task::perform(
                        async move {
                            match client.client.like_song(song_id, !is_liked).await {
                                Ok(_) => Some(!is_liked),
                                Err(e) => {
                                    error!("Failed to toggle like: {}", e);
                                    None
                                }
                            }
                        },
                        move |result| {
                            if let Some(liked) = result {
                                Message::FavoriteStatusChanged(song_id, liked)
                            } else {
                                Message::ShowErrorToast("操作失败".to_string())
                            }
                        },
                    ))
                } else {
                    Some(Task::none())
                }
            }

            Message::FavoriteStatusChanged(song_id, liked) => {
                if let Some(ref mut user_info) = self.core.user_info {
                    if *liked {
                        user_info.like_songs.insert(*song_id);
                    } else {
                        user_info.like_songs.remove(song_id);
                    }
                }

                // Update tray state if this is the current song
                if let Some(current) = &self.playback.current_song {
                    if current.id < 0 && (-current.id) as u64 == *song_id {
                        let is_playing = self.playback_is_playing();
                        crate::app::helpers::update_tray_state_with_favorite(
                            is_playing,
                            Some(current.title.clone()),
                            Some(current.artist.clone()),
                            self.core.settings.play_mode,
                            Some(*song_id),
                            *liked,
                        );
                    }
                }

                Some(Task::none())
            }

            Message::PlayNcmSong(song_info) => {
                debug!("Playing NCM song: {}", song_info.name);

                if self.core.ncm_client.is_none() {
                    return Some(Task::done(Message::ShowWarningToast(
                        "请先登录".to_string(),
                    )));
                }

                self.exit_fm_mode();
                self.ui.home.current_ncm_playlist_songs = vec![song_info.clone()];
                self.playback.queue.clear();
                self.playback
                    .queue
                    .push(Self::ncm_song_to_db_song(song_info));
                self.persist_queue_snapshot();

                Some(self.play_song_at_index(0))
            }

            Message::AddNcmPlaylist(songs, play_now) => {
                debug!(
                    "Adding {} NCM songs to playlist, play_now: {}",
                    songs.len(),
                    play_now
                );
                self.ui.home.current_ncm_playlist_songs = songs.clone();

                let db_songs: Vec<crate::database::DbSong> = songs
                    .iter()
                    .map(|song| crate::database::DbSong {
                        id: -(song.id as i64),
                        file_path: String::new(),
                        title: song.name.clone(),
                        artist: song.singer.clone(),
                        album: song.album.clone(),
                        duration_secs: (song.duration / 1000) as i64,
                        track_number: None,
                        year: None,
                        genre: None,
                        cover_path: if song.pic_url.is_empty() {
                            None
                        } else {
                            Some(song.pic_url.clone())
                        },
                        file_hash: None,
                        file_size: 0,
                        format: Some("mp3".to_string()),
                        normalization_gain: None,
                        play_count: 0,
                        last_played: None,
                        last_modified: 0,
                        is_missing: false,
                        created_at: 0,
                    })
                    .collect();

                if self.is_fm_mode() && !*play_now {
                    debug!("FM mode: appending {} songs to queue", db_songs.len());
                    self.playback.queue.extend(db_songs.clone());
                    self.persist_queue_snapshot();
                    return Some(Task::none());
                }

                if *play_now {
                    self.playback.queue = db_songs.clone();
                    self.playback.current_index = Some(0);
                    self.persist_queue_snapshot();

                    return Some(self.update(Message::PlayQueueIndex(0)));
                } else {
                    self.playback.queue.extend(db_songs);
                    self.persist_queue_snapshot();
                }

                Some(Task::none())
            }

            Message::UserPlaylistsLoaded(playlists) => {
                self.ui.home.user_playlists = playlists.clone();
                Some(Task::none())
            }

            Message::HoverTrendingSong(song_id_opt) => {
                self.ui
                    .home
                    .song_hover_animations
                    .set_hovered_exclusive(*song_id_opt);
                Some(Task::none())
            }

            Message::OpenNcmPlaylist(playlist_id) => {
                let route = Route::NcmPlaylist(*playlist_id);
                if self.ui.current_route != route {
                    return Some(self.navigate_to_route(route, true));
                }

                Some(self.open_ncm_playlist_route(*playlist_id))
            }

            Message::OpenArtist(artist_id) => Some(self.open_artist_route(*artist_id)),

            Message::SwitchArtistTab(tab) => {
                if let Some(playlist) = &mut self.ui.playlist_page.current {
                    playlist.artist_tab = *tab;
                }
                Some(Task::none())
            }

            Message::OpenArtistByName(name) => {
                let keyword = name.trim().to_string();
                if keyword.is_empty() {
                    return Some(Task::none());
                }

                if let Some(client) = &self.core.ncm_client {
                    let client = client.clone();
                    Some(Task::perform(
                        async move {
                            client
                                .client
                                .search(&keyword, crate::api::ncm_api::SearchType::Artists, 1, 0)
                                .await
                                .ok()
                                .and_then(|response| {
                                    response.albums.first().map(|artist| artist.id)
                                })
                        },
                        |result| {
                            if let Some(artist_id) = result {
                                Message::OpenArtist(artist_id)
                            } else {
                                Message::ShowWarningToast("未找到对应歌手".to_string())
                            }
                        },
                    ))
                } else {
                    Some(Task::done(Message::ShowWarningToast(
                        "请先登录".to_string(),
                    )))
                }
            }

            Message::NcmPlaylistDetailLoaded(detail) => {
                debug!(
                    "NCM playlist detail loaded: {} with {} songs",
                    detail.name,
                    detail.songs.len()
                );

                let playlist_id = -(detail.id as i64);

                // Calculate total duration
                let total_secs: u64 = detail.songs.iter().map(|s| s.duration / 1000).sum();
                let total_mins = total_secs / 60;
                let total_hours = total_mins / 60;
                let remaining_mins = total_mins % 60;
                let total_duration = if total_hours > 0 {
                    format!("约 {} 小时 {} 分钟", total_hours, remaining_mins)
                } else {
                    format!("{} 分钟", total_mins)
                };

                // Update existing PlaylistView with full details (keep cover_path if already loaded)
                if let Some(playlist) = &mut self.ui.playlist_page.current {
                    if playlist.id == playlist_id {
                        playlist.name = detail.name.clone();
                        playlist.description = if detail.description.is_empty() {
                            None
                        } else {
                            Some(detail.description.clone())
                        };
                        playlist.owner = if detail.creator_nickname.is_empty() {
                            "网易云音乐".to_string()
                        } else {
                            detail.creator_nickname.clone()
                        };
                        playlist.creator_id = detail.creator_id;
                        playlist.song_count = detail.songs.len() as u32;
                        playlist.total_duration = total_duration;
                        playlist.is_subscribed = detail.subscribed;
                    }
                }

                // Store NCM songs for playback
                self.ui.home.current_ncm_playlist_songs = detail.songs.clone();

                // Check if creator avatar already exists in cache
                let avatars_cache_dir = crate::utils::avatars_cache_dir();
                let avatar_stem = format!("playlist_creator_{}", detail.id);
                let existing_avatar =
                    crate::utils::find_cached_image(&avatars_cache_dir, &avatar_stem);

                // Start creator avatar download task only if not cached
                let avatar_task = if existing_avatar.is_some() {
                    // Avatar already cached, send message directly
                    if let Some(path) = existing_avatar {
                        Task::done(Message::NcmPlaylistCreatorAvatarLoaded(
                            playlist_id,
                            path.to_string_lossy().to_string(),
                        ))
                    } else {
                        Task::none()
                    }
                } else if !detail.creator_avatar_url.is_empty() {
                    if let Some(client) = &self.core.ncm_client {
                        let client = client.clone();
                        let avatar_url = detail.creator_avatar_url.clone();
                        let ncm_id = detail.id;
                        let internal_id = playlist_id;
                        Task::perform(
                            async move {
                                crate::utils::download_playlist_creator_avatar(
                                    &client,
                                    ncm_id,
                                    &avatar_url,
                                )
                                .await
                                .map(|p| (internal_id, p.to_string_lossy().to_string()))
                            },
                            |result| {
                                if let Some((id, path)) = result {
                                    Message::NcmPlaylistCreatorAvatarLoaded(id, path)
                                } else {
                                    Message::NoOp
                                }
                            },
                        )
                    } else {
                        Task::none()
                    }
                } else {
                    Task::none()
                };

                let creator_detail_task = if detail.creator_id != 0 {
                    if let Some(client) = &self.core.ncm_client {
                        let client = client.clone();
                        let creator_id = detail.creator_id;
                        let internal_id = playlist_id;
                        Task::perform(
                            async move { client.client.user_detail(creator_id).await.ok() },
                            move |result| {
                                if let Some(user_detail) = result {
                                    Message::PlaylistCreatorDetailLoaded(internal_id, user_detail)
                                } else {
                                    Message::NoOp
                                }
                            },
                        )
                    } else {
                        Task::none()
                    }
                } else {
                    Task::none()
                };

                // Spawn async task to convert songs (cover download already started in OpenNcmPlaylist)
                let songs = detail.songs.clone();
                let cover_cache_dir = crate::utils::covers_cache_dir();
                let avatars_cache_dir = crate::utils::avatars_cache_dir();
                let ncm_playlist_id = detail.id;

                // Start songs conversion task
                let songs_task = Task::perform(
                    async move {
                        // Run all blocking operations in spawn_blocking
                        tokio::task::spawn_blocking(move || {
                            // Check all song covers (file system operations)
                            let cover_paths: Vec<(u64, Option<String>)> = songs
                                .iter()
                                .map(|song| {
                                    let stem = format!("cover_{}", song.id);
                                    let cover_path =
                                        crate::utils::find_cached_image(&cover_cache_dir, &stem)
                                            .map(|p| p.to_string_lossy().to_string());
                                    (song.id, cover_path)
                                })
                                .collect();

                            // Convert songs to views
                            let song_views =
                                crate::app::update::page_loader::convert_ncm_songs_to_views(
                                    &songs,
                                    &cover_paths,
                                );

                            // Check creator avatar
                            let avatar_stem = format!("playlist_creator_{}", ncm_playlist_id);
                            let avatar_path =
                                crate::utils::find_cached_image(&avatars_cache_dir, &avatar_stem)
                                    .map(|p| p.to_string_lossy().to_string());

                            (playlist_id, song_views, avatar_path)
                        })
                        .await
                        .unwrap_or_else(|_| (playlist_id, Vec::new(), None))
                    },
                    |(playlist_id, song_views, avatar_path)| {
                        Message::NcmPlaylistSongsReady(
                            playlist_id,
                            song_views,
                            None,
                            crate::utils::ColorPalette::default(),
                            avatar_path,
                        )
                    },
                );

                return Some(Task::batch([songs_task, avatar_task, creator_detail_task]));
            }

            Message::NcmPlaylistSongsReady(
                playlist_id,
                song_views,
                _cover_path,
                _palette,
                avatar_path,
            ) => {
                let mut song_views = song_views.clone();

                if let Some(playlist) = &self.ui.playlist_page.current {
                    if playlist.id == *playlist_id
                        && playlist.kind == crate::ui::pages::playlist::DetailPageKind::Album
                    {
                        if let Some(shared_cover_path) = playlist.cover_path.clone() {
                            for song in &mut song_views {
                                song.cover_path = Some(shared_cover_path.clone());
                            }
                        }
                    }
                }

                debug!("NCM playlist songs ready: {} songs", song_views.len());

                let initial_cover_requests: Vec<(i64, String)> = song_views
                    .iter()
                    .filter_map(|song| {
                        if song.cover_path.is_some() {
                            return None;
                        }

                        song.pic_url
                            .as_ref()
                            .filter(|url| !url.is_empty())
                            .map(|url| (song.id, url.clone()))
                    })
                    .take(12)
                    .collect();

                // Update existing playlist view with songs
                if let Some(playlist) = &mut self.ui.playlist_page.current {
                    if playlist.id == *playlist_id {
                        playlist.songs = song_views.clone();
                        if let Some(avatar) = avatar_path {
                            playlist.owner_avatar_path = Some(avatar.clone());
                        }
                    }
                }

                // Update load state
                self.ui.playlist_page.load_state =
                    crate::app::update::page_loader::PlaylistLoadState::Ready;

                // Scroll to top
                let mut tasks = vec![iced::widget::operation::snap_to(
                    iced::widget::Id::new("playlist_scroll"),
                    iced::widget::scrollable::RelativeOffset { x: 0.0, y: 0.0 },
                )];
                if !initial_cover_requests.is_empty() {
                    tasks.push(Task::done(Message::RequestSongCoversLazy(
                        initial_cover_requests,
                    )));
                }
                Some(Task::batch(tasks))
            }

            Message::AlbumDetailLoaded(detail) => {
                debug!(
                    "Album detail loaded: {} with {} songs",
                    detail.name,
                    detail.songs.len()
                );

                let page_id = album_page_id(detail.id);
                let total_secs: u64 = detail.songs.iter().map(|song| song.duration / 1000).sum();
                let total_mins = total_secs / 60;
                let total_hours = total_mins / 60;
                let remaining_mins = total_mins % 60;
                let total_duration = if total_hours > 0 {
                    format!("约 {} 小时 {} 分钟", total_hours, remaining_mins)
                } else {
                    format!("{} 分钟", total_mins)
                };

                if let Some(playlist) = &mut self.ui.playlist_page.current {
                    if playlist.id == page_id {
                        playlist.kind = crate::ui::pages::playlist::DetailPageKind::Album;
                        playlist.name = detail.name.clone();
                        playlist.description = (!detail.description.trim().is_empty())
                            .then_some(detail.description.clone());
                        playlist.owner = if detail.artist_name.trim().is_empty() {
                            "网易云音乐".to_string()
                        } else {
                            detail.artist_name.clone()
                        };
                        playlist.owner_artist_id =
                            (detail.artist_id != 0).then_some(detail.artist_id);
                        playlist.song_count = detail.track_count.max(detail.songs.len() as u32);
                        playlist.total_duration = total_duration;
                        playlist.like_count.clear();
                    }
                }

                self.ui.home.current_ncm_playlist_songs = detail.songs.clone();

                let artist_avatar_task = if detail.artist_id != 0 {
                    let cover_cache_dir = crate::utils::covers_cache_dir();
                    let artist_cover_stem = format!("artist_{}", detail.artist_id);
                    let existing_artist_cover =
                        crate::utils::find_cached_image(&cover_cache_dir, &artist_cover_stem)
                            .map(|path| path.to_string_lossy().to_string());

                    if let Some(path) = existing_artist_cover {
                        Task::done(Message::NcmPlaylistCreatorAvatarLoaded(page_id, path))
                    } else if !detail.artist_pic_url.is_empty() {
                        if let Some(client) = &self.core.ncm_client {
                            let client = client.clone();
                            let artist_id = detail.artist_id;
                            let artist_pic_url = detail.artist_pic_url.clone();
                            Task::perform(
                                async move {
                                    crate::utils::download_artist_cover(
                                        &client,
                                        artist_id,
                                        &artist_pic_url,
                                    )
                                    .await
                                    .map(|path| (page_id, path.to_string_lossy().to_string()))
                                },
                                |result| {
                                    if let Some((page_id, path)) = result {
                                        Message::NcmPlaylistCreatorAvatarLoaded(page_id, path)
                                    } else {
                                        Message::NoOp
                                    }
                                },
                            )
                        } else {
                            Task::none()
                        }
                    } else {
                        Task::none()
                    }
                } else {
                    Task::none()
                };

                let cached_cover = find_cached_album_cover(detail.id);
                let cover_task = if let Some(path) = cached_cover.clone() {
                    Task::done(Message::AlbumCoverLoaded(page_id, path))
                } else if !detail.cover_img_url.is_empty() {
                    if let Some(client) = &self.core.ncm_client {
                        let client = client.clone();
                        let album_id = detail.id;
                        let cover_url = detail.cover_img_url.clone();
                        let cover_path = crate::utils::covers_cache_dir()
                            .join(format!("search_album_{}.jpg", album_id));
                        Task::perform(
                            async move {
                                crate::utils::download_img(
                                    &client, &cover_url, cover_path, 300, 300,
                                )
                                .await
                                .map(|path| {
                                    (album_page_id(album_id), path.to_string_lossy().to_string())
                                })
                            },
                            |result| {
                                if let Some((page_id, path)) = result {
                                    Message::AlbumCoverLoaded(page_id, path)
                                } else {
                                    Message::NoOp
                                }
                            },
                        )
                    } else {
                        Task::none()
                    }
                } else {
                    Task::none()
                };

                let songs = detail.songs.clone();
                let shared_cover_path = cached_cover;
                let cover_cache_dir = crate::utils::covers_cache_dir();
                let songs_task = Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let cover_paths: Vec<(u64, Option<String>)> = songs
                                .iter()
                                .map(|song| {
                                    let stem = format!("cover_{}", song.id);
                                    let cover_path =
                                        crate::utils::find_cached_image(&cover_cache_dir, &stem)
                                            .map(|path| path.to_string_lossy().to_string())
                                            .or_else(|| shared_cover_path.clone());
                                    (song.id, cover_path)
                                })
                                .collect();

                            crate::app::update::page_loader::convert_ncm_songs_to_views(
                                &songs,
                                &cover_paths,
                            )
                        })
                        .await
                        .unwrap_or_default()
                    },
                    move |song_views| {
                        Message::NcmPlaylistSongsReady(
                            page_id,
                            song_views,
                            None,
                            crate::utils::ColorPalette::default(),
                            None,
                        )
                    },
                );

                Some(Task::batch([artist_avatar_task, cover_task, songs_task]))
            }

            Message::ArtistDetailLoaded(detail) => {
                debug!(
                    "Artist detail loaded: {} with {} songs",
                    detail.name,
                    detail.hot_songs.len()
                );

                let page_id = artist_page_id(detail.id);
                let total_secs: u64 = detail.hot_songs.iter().map(|s| s.duration / 1000).sum();
                let total_mins = total_secs / 60;
                let total_hours = total_mins / 60;
                let remaining_mins = total_mins % 60;
                let total_duration = if total_hours > 0 {
                    format!("约 {} 小时 {} 分钟", total_hours, remaining_mins)
                } else {
                    format!("{} 分钟", total_mins)
                };

                let description = if detail.description.trim().is_empty() {
                    Some(format!(
                        "{} 首热门单曲 · {} 张专辑",
                        detail.music_size, detail.album_size
                    ))
                } else {
                    Some(detail.description.clone())
                };

                if let Some(playlist) = &mut self.ui.playlist_page.current {
                    if playlist.id == page_id {
                        playlist.kind = crate::ui::pages::playlist::DetailPageKind::Artist;
                        playlist.name = detail.name.clone();
                        playlist.description = description;
                        playlist.profile_stats = Some(format!(
                            "{} 首热门单曲 · {} 张专辑",
                            detail.music_size, detail.album_size
                        ));
                        playlist.owner = "歌手热门作品".to_string();
                        playlist.owner_artist_id = Some(detail.id);
                        playlist.song_count = detail.hot_songs.len() as u32;
                        playlist.total_duration = total_duration;
                        playlist.like_count = format!("{} 张专辑", detail.album_size);
                    }
                }

                self.ui.home.current_ncm_playlist_songs = detail.hot_songs.clone();

                let cover_cache_dir = crate::utils::covers_cache_dir();
                let cover_stem = format!("artist_{}", detail.id);
                let existing_cover = crate::utils::find_cached_image(&cover_cache_dir, &cover_stem)
                    .map(|p| p.to_string_lossy().to_string());

                let cover_task = if let Some(path) = existing_cover {
                    Task::done(Message::ArtistCoverLoaded(page_id, path))
                } else if !detail.pic_url.is_empty() {
                    if let Some(client) = &self.core.ncm_client {
                        let client = client.clone();
                        let artist_id = detail.id;
                        let cover_url = detail.pic_url.clone();
                        Task::perform(
                            async move {
                                crate::utils::download_artist_cover(&client, artist_id, &cover_url)
                                    .await
                                    .map(|p| {
                                        (artist_page_id(artist_id), p.to_string_lossy().to_string())
                                    })
                            },
                            |result| {
                                if let Some((id, path)) = result {
                                    Message::ArtistCoverLoaded(id, path)
                                } else {
                                    Message::NoOp
                                }
                            },
                        )
                    } else {
                        Task::none()
                    }
                } else {
                    Task::none()
                };

                let songs = detail.hot_songs.clone();
                let songs_task = Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let cover_paths: Vec<(u64, Option<String>)> = songs
                                .iter()
                                .map(|song| {
                                    let stem = format!("cover_{}", song.id);
                                    let cover_path =
                                        crate::utils::find_cached_image(&cover_cache_dir, &stem)
                                            .map(|p| p.to_string_lossy().to_string());
                                    (song.id, cover_path)
                                })
                                .collect();

                            crate::app::update::page_loader::convert_ncm_songs_to_views(
                                &songs,
                                &cover_paths,
                            )
                        })
                        .await
                        .unwrap_or_default()
                    },
                    move |song_views| {
                        Message::NcmPlaylistSongsReady(
                            page_id,
                            song_views,
                            None,
                            crate::utils::ColorPalette::default(),
                            None,
                        )
                    },
                );

                let albums_task = if let Some(client) = &self.core.ncm_client {
                    let client = client.clone();
                    let page_id = page_id;
                    let artist_name = detail.name.clone();
                    Task::perform(
                        async move {
                            client
                                .client
                                .search(
                                    &artist_name,
                                    crate::api::ncm_api::SearchType::Albums,
                                    50,
                                    0,
                                )
                                .await
                                .ok()
                                .map(|response| {
                                    let mut albums: Vec<_> = response
                                        .albums
                                        .into_iter()
                                        .filter(|album| album.author.trim() == artist_name.trim())
                                        .collect();
                                    albums.sort_by(|a, b| a.name.cmp(&b.name));
                                    (page_id, albums)
                                })
                        },
                        move |result| {
                            if let Some((page_id, albums)) = result {
                                Message::ArtistAlbumsLoaded(page_id, albums)
                            } else {
                                Message::ArtistAlbumsLoaded(page_id, Vec::new())
                            }
                        },
                    )
                } else {
                    Task::none()
                };

                Some(Task::batch([cover_task, songs_task, albums_task]))
            }

            Message::ArtistAlbumsLoaded(page_id, albums) => {
                if let Some(playlist) = &mut self.ui.playlist_page.current {
                    if playlist.id == *page_id {
                        playlist.artist_albums = albums.clone();
                    }
                }
                let Some(client) = &self.core.ncm_client else {
                    return Some(Task::none());
                };

                let covers_dir = crate::utils::covers_cache_dir();
                let mut tasks = Vec::new();
                for album in albums {
                    if album.cover_img_url.is_empty() {
                        continue;
                    }

                    let cover_stem = format!("search_album_{}", album.id);
                    if let Some(path) = crate::utils::find_cached_image(&covers_dir, &cover_stem) {
                        tasks.push(Task::done(Message::ArtistAlbumCoverLoaded(
                            *page_id,
                            album.id,
                            path.to_string_lossy().to_string(),
                        )));
                        continue;
                    }

                    let client = client.clone();
                    let page_id = *page_id;
                    let album_id = album.id;
                    let cover_url = album.cover_img_url.clone();
                    let cover_path = covers_dir.join(format!("{}.jpg", cover_stem));
                    tasks.push(Task::perform(
                        async move {
                            crate::utils::download_img(&client, &cover_url, cover_path, 300, 300)
                                .await
                                .map(|path| (page_id, album_id, path.to_string_lossy().to_string()))
                        },
                        |result| {
                            if let Some((page_id, album_id, path)) = result {
                                Message::ArtistAlbumCoverLoaded(page_id, album_id, path)
                            } else {
                                Message::NoOp
                            }
                        },
                    ));
                }

                Some(if tasks.is_empty() {
                    Task::none()
                } else {
                    Task::batch(tasks)
                })
            }

            Message::ArtistAlbumCoverLoaded(page_id, album_id, path) => {
                let mut should_cache_handle = false;
                if let Some(playlist) = &mut self.ui.playlist_page.current {
                    if playlist.id == *page_id {
                        if let Some(album) = playlist
                            .artist_albums
                            .iter_mut()
                            .find(|album| album.id == *album_id)
                        {
                            album.cover_img_url = path.clone();
                            should_cache_handle = true;
                        }
                    }
                }

                if should_cache_handle && std::path::Path::new(path).exists() {
                    self.ui.playlist_page.artist_album_covers.insert(
                        *album_id,
                        iced::widget::image::Handle::from_path(path.clone()),
                    );
                }
                Some(Task::none())
            }

            Message::UserPageDetailLoaded(page_id, detail) => {
                let page_id = *page_id;
                let description = if !detail.signature.trim().is_empty() {
                    Some(detail.signature.clone())
                } else if detail.artist_id != 0 {
                    Some("网易云音乐人".to_string())
                } else {
                    Some(format!("网易云用户 {}", detail.nickname))
                };

                if let Some(playlist) = &mut self.ui.playlist_page.current {
                    if playlist.id == page_id {
                        playlist.kind = crate::ui::pages::playlist::DetailPageKind::User;
                        playlist.name = detail.nickname.clone();
                        playlist.description = description;
                        playlist.profile_stats = Some(format!(
                            "关注 {} · 粉丝 {}",
                            format_social_count(detail.follows),
                            format_social_count(detail.followeds)
                        ));
                        playlist.owner = if detail.artist_name.trim().is_empty() {
                            "网易云用户".to_string()
                        } else {
                            detail.artist_name.clone()
                        };
                        playlist.owner_artist_id =
                            (detail.artist_id != 0).then_some(detail.artist_id);
                        playlist.creator_id = detail.user_id;
                        playlist.like_count.clear();
                    }
                }

                let avatar_cache_dir = crate::utils::avatars_cache_dir();
                let avatar_stem = format!("avatar_{}", detail.user_id);
                let existing_avatar =
                    crate::utils::find_cached_image(&avatar_cache_dir, &avatar_stem)
                        .map(|p| p.to_string_lossy().to_string());

                let avatar_task = if let Some(path) = existing_avatar {
                    Task::batch([
                        Task::done(Message::NcmPlaylistCreatorAvatarLoaded(
                            page_id,
                            path.clone(),
                        )),
                        Task::done(Message::ArtistCoverLoaded(page_id, path)),
                    ])
                } else if !detail.avatar_url.is_empty() {
                    if let Some(client) = &self.core.ncm_client {
                        let client = client.clone();
                        let user_id = detail.user_id;
                        let avatar_url = detail.avatar_url.clone();
                        Task::perform(
                            async move {
                                crate::utils::download_avatar(&client, user_id, &avatar_url)
                                    .await
                                    .map(|p| (page_id, p.to_string_lossy().to_string()))
                            },
                            |result| {
                                if let Some((page_id, path)) = result {
                                    Message::NcmPlaylistCreatorAvatarLoaded(page_id, path)
                                } else {
                                    Message::NoOp
                                }
                            },
                        )
                    } else {
                        Task::none()
                    }
                } else {
                    Task::none()
                };

                let artist_task = if detail.artist_id != 0 {
                    if let Some(client) = &self.core.ncm_client {
                        let client = client.clone();
                        let artist_id = detail.artist_id;
                        Task::perform(
                            async move {
                                client
                                    .client
                                    .artist_detail(artist_id)
                                    .await
                                    .ok()
                                    .map(|detail| (page_id, detail))
                            },
                            |result| {
                                if let Some((page_id, detail)) = result {
                                    Message::UserArtistDetailLoaded(page_id, detail)
                                } else {
                                    Message::NoOp
                                }
                            },
                        )
                    } else {
                        Task::none()
                    }
                } else {
                    self.ui.playlist_page.load_state =
                        crate::app::update::page_loader::PlaylistLoadState::Ready;
                    Task::none()
                };

                let playlist_task = if let Some(client) = &self.core.ncm_client {
                    let client = client.clone();
                    let user_id = detail.user_id;
                    Task::perform(
                        async move { client.client.user_song_list(user_id, 0, 30).await.ok() },
                        move |result| {
                            if let Some(playlists) = result {
                                Message::UserPagePlaylistsLoaded(page_id, playlists)
                            } else {
                                Message::UserPagePlaylistsLoaded(page_id, Vec::new())
                            }
                        },
                    )
                } else {
                    Task::none()
                };

                Some(Task::batch([avatar_task, artist_task, playlist_task]))
            }

            Message::UserPagePlaylistsLoaded(page_id, playlists) => {
                if let Some(playlist) = &mut self.ui.playlist_page.current {
                    if playlist.id == *page_id {
                        playlist.user_playlists = playlists.clone();
                    }
                }

                let Some(client) = &self.core.ncm_client else {
                    return Some(Task::none());
                };

                let covers_dir = crate::utils::covers_cache_dir();
                let mut tasks = Vec::new();
                for playlist in playlists {
                    if playlist.cover_img_url.is_empty() {
                        continue;
                    }

                    let cover_stem = format!("playlist_{}", playlist.id);
                    if let Some(path) = crate::utils::find_cached_image(&covers_dir, &cover_stem) {
                        tasks.push(Task::done(Message::UserPlaylistCoverLoaded(
                            *page_id,
                            playlist.id,
                            path.to_string_lossy().to_string(),
                        )));
                        continue;
                    }

                    let client = client.clone();
                    let page_id = *page_id;
                    let playlist_id = playlist.id;
                    let cover_url = playlist.cover_img_url.clone();
                    tasks.push(Task::perform(
                        async move {
                            crate::utils::download_playlist_cover(&client, playlist_id, &cover_url)
                                .await
                                .map(|path| {
                                    (page_id, playlist_id, path.to_string_lossy().to_string())
                                })
                        },
                        |result| {
                            if let Some((page_id, playlist_id, path)) = result {
                                Message::UserPlaylistCoverLoaded(page_id, playlist_id, path)
                            } else {
                                Message::NoOp
                            }
                        },
                    ));
                }

                Some(if tasks.is_empty() {
                    Task::none()
                } else {
                    Task::batch(tasks)
                })
            }

            Message::UserPlaylistCoverLoaded(page_id, playlist_id, path) => {
                if let Some(playlist) = &mut self.ui.playlist_page.current {
                    if playlist.id == *page_id {
                        if let Some(item) = playlist
                            .user_playlists
                            .iter_mut()
                            .find(|item| item.id == *playlist_id)
                        {
                            item.cover_img_url = path.clone();
                        }
                    }
                }
                Some(Task::none())
            }

            Message::UserArtistDetailLoaded(page_id, detail) => {
                let page_id = *page_id;
                let total_secs: u64 = detail.hot_songs.iter().map(|s| s.duration / 1000).sum();
                let total_mins = total_secs / 60;
                let total_hours = total_mins / 60;
                let remaining_mins = total_mins % 60;
                let total_duration = if total_hours > 0 {
                    format!("约 {} 小时 {} 分钟", total_hours, remaining_mins)
                } else {
                    format!("{} 分钟", total_mins)
                };

                if let Some(playlist) = &mut self.ui.playlist_page.current {
                    if playlist.id == page_id {
                        if playlist
                            .description
                            .as_deref()
                            .unwrap_or("")
                            .trim()
                            .is_empty()
                        {
                            playlist.description = Some(format!(
                                "{} 首热门单曲 · {} 张专辑",
                                detail.music_size, detail.album_size
                            ));
                        }
                        if playlist
                            .profile_stats
                            .as_deref()
                            .unwrap_or("")
                            .trim()
                            .is_empty()
                        {
                            playlist.profile_stats = Some(format!(
                                "{} 首热门单曲 · {} 张专辑",
                                detail.music_size, detail.album_size
                            ));
                        }
                        playlist.owner = "热门作品".to_string();
                        playlist.owner_artist_id = Some(detail.id);
                        playlist.song_count = detail.hot_songs.len() as u32;
                        playlist.total_duration = total_duration;
                        playlist.like_count = format!("{} 张专辑", detail.album_size);
                    }
                }

                self.ui.home.current_ncm_playlist_songs = detail.hot_songs.clone();
                self.ui.playlist_page.load_state =
                    crate::app::update::page_loader::PlaylistLoadState::Ready;

                let cover_cache_dir = crate::utils::covers_cache_dir();
                let cover_stem = format!("artist_{}", detail.id);
                let existing_cover = crate::utils::find_cached_image(&cover_cache_dir, &cover_stem)
                    .map(|p| p.to_string_lossy().to_string());

                let cover_task = if let Some(path) = existing_cover {
                    Task::done(Message::ArtistCoverLoaded(page_id, path))
                } else if !detail.pic_url.is_empty() {
                    if let Some(client) = &self.core.ncm_client {
                        let client = client.clone();
                        let artist_id = detail.id;
                        let cover_url = detail.pic_url.clone();
                        Task::perform(
                            async move {
                                crate::utils::download_artist_cover(&client, artist_id, &cover_url)
                                    .await
                                    .map(|p| (page_id, p.to_string_lossy().to_string()))
                            },
                            |result| {
                                if let Some((id, path)) = result {
                                    Message::ArtistCoverLoaded(id, path)
                                } else {
                                    Message::NoOp
                                }
                            },
                        )
                    } else {
                        Task::none()
                    }
                } else {
                    Task::none()
                };

                let songs = detail.hot_songs.clone();
                let songs_task = Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let cover_paths: Vec<(u64, Option<String>)> = songs
                                .iter()
                                .map(|song| {
                                    let stem = format!("cover_{}", song.id);
                                    let cover_path =
                                        crate::utils::find_cached_image(&cover_cache_dir, &stem)
                                            .map(|p| p.to_string_lossy().to_string());
                                    (song.id, cover_path)
                                })
                                .collect();

                            crate::app::update::page_loader::convert_ncm_songs_to_views(
                                &songs,
                                &cover_paths,
                            )
                        })
                        .await
                        .unwrap_or_default()
                    },
                    move |song_views| {
                        Message::NcmPlaylistSongsReady(
                            page_id,
                            song_views,
                            None,
                            crate::utils::ColorPalette::default(),
                            None,
                        )
                    },
                );

                Some(Task::batch([cover_task, songs_task]))
            }

            Message::CurrentSongCoverReady(song_id, path) => {
                tracing::info!(
                    "Current song cover downloaded: song_id={}, path={}",
                    song_id,
                    path
                );

                // Update current_song's cover_path
                if let Some(current) = &mut self.playback.current_song {
                    if current.id == *song_id {
                        current.cover_path = Some(path.clone());
                    }
                }

                // Update in queue and database
                if let Some(idx) = self.playback.current_index {
                    if let Some(queue_song) = self.playback.queue.get_mut(idx) {
                        if queue_song.id == *song_id {
                            queue_song.cover_path = Some(path.clone());

                            // Also update database with local cover path
                            if let Some(db) = &self.core.db {
                                let db = db.clone();
                                let song_clone = queue_song.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = db.upsert_ncm_song(&song_clone).await {
                                        tracing::warn!(
                                            "Failed to update cover path in database: {}",
                                            e
                                        );
                                    }
                                });
                            }
                        }
                    }
                }

                // If lyrics page is open, update the background with new cover
                if self.ui.lyrics.is_open {
                    if let Some(song) = self.playback.current_song.clone() {
                        if song.id == *song_id {
                            return Some(self.update_lyrics_background(&song));
                        }
                    }
                }

                Some(Task::none())
            }

            Message::NcmPlaylistSongCoversBatchLoaded(covers) => {
                // Batch update cover paths in the current playlist view
                if let Some(playlist) = &mut self.ui.playlist_page.current {
                    for (song_id, path) in covers.iter() {
                        // Remove from pending downloads
                        self.ui
                            .playlist_page
                            .pending_cover_downloads
                            .remove(song_id);

                        if let Some(song) = playlist.songs.iter_mut().find(|s| s.id == *song_id) {
                            song.cover_path = Some(path.clone());
                            // 封面已下载，清除远程 URL
                            song.pic_url = None;
                            // Update the cover_handle for immediate display
                            if std::path::Path::new(path).exists() {
                                song.cover_handle =
                                    Some(iced::widget::image::Handle::from_path(path));
                            }
                        }
                    }
                }

                let mut next_requests: Vec<(i64, String)> = Vec::new();
                if let Some(playlist) = self.ui.playlist_page.current.as_ref() {
                    if playlist.id < 0 {
                        let scroll_state = self.ui.playlist_page.scroll_state.borrow();
                        let (start, end) = scroll_state.visible_range();
                        drop(scroll_state);

                        for idx in start..end.min(playlist.songs.len()) {
                            let song = &playlist.songs[idx];
                            if song.cover_handle.is_some() {
                                continue;
                            }
                            let Some(pic_url) = song.pic_url.as_ref() else {
                                continue;
                            };
                            if pic_url.is_empty() {
                                continue;
                            }
                            if self
                                .ui
                                .playlist_page
                                .pending_cover_downloads
                                .contains(&song.id)
                            {
                                continue;
                            }
                            next_requests.push((song.id, pic_url.clone()));
                            if next_requests.len() >= 5 {
                                break;
                            }
                        }
                    }
                }

                if next_requests.is_empty() {
                    Some(Task::none())
                } else {
                    Some(Task::done(Message::RequestSongCoversLazy(next_requests)))
                }
            }

            Message::RequestSongCoversLazy(requests) => {
                let cover_cache_dir = crate::utils::covers_cache_dir();
                let mut cached_results = Vec::new();
                let mut songs_to_download = Vec::new();

                for (song_id, pic_url) in requests.iter().cloned() {
                    if self
                        .ui
                        .playlist_page
                        .pending_cover_downloads
                        .contains(&song_id)
                    {
                        continue;
                    }

                    if pic_url.is_empty() {
                        continue;
                    }

                    let ncm_id = if song_id < 0 {
                        (-song_id) as u64
                    } else {
                        song_id as u64
                    };
                    let stem = format!("cover_{}", ncm_id);

                    if let Some(path) = crate::utils::find_cached_image(&cover_cache_dir, &stem) {
                        cached_results.push((song_id, path.to_string_lossy().to_string()));
                    } else {
                        songs_to_download.push((song_id, pic_url));
                    }
                }

                let cached_task = if cached_results.is_empty() {
                    None
                } else {
                    Some(Task::done(Message::NcmPlaylistSongCoversBatchLoaded(
                        cached_results,
                    )))
                };

                if songs_to_download.is_empty() {
                    return Some(cached_task.unwrap_or_else(Task::none));
                }

                // Mark as pending
                for (song_id, _) in &songs_to_download {
                    self.ui
                        .playlist_page
                        .pending_cover_downloads
                        .insert(*song_id);
                }

                // Start download task
                if let Some(client) = &self.core.ncm_client {
                    let client = client.clone();
                    let download_task = Task::perform(
                        async move {
                            let mut results = Vec::new();
                            for (song_id, pic_url) in songs_to_download {
                                let ncm_id = if song_id < 0 {
                                    (-song_id) as u64
                                } else {
                                    song_id as u64
                                };
                                if let Some(path) =
                                    crate::utils::download_cover(&client, ncm_id, &pic_url).await
                                {
                                    results.push((song_id, path.to_string_lossy().to_string()));
                                }
                            }
                            results
                        },
                        Message::NcmPlaylistSongCoversBatchLoaded,
                    );

                    return Some(if let Some(cached_task) = cached_task {
                        Task::batch([cached_task, download_task])
                    } else {
                        download_task
                    });
                }

                Some(cached_task.unwrap_or_else(Task::none))
            }

            Message::NcmPlaylistCoverLoaded(playlist_id, path) => {
                tracing::info!("Playlist cover loaded: id={}, path={}", playlist_id, path);
                // Update the playlist cover path
                if let Some(playlist) = &mut self.ui.playlist_page.current {
                    tracing::info!("Current playlist id: {}", playlist.id);
                    if playlist.id == *playlist_id {
                        tracing::info!("Updating playlist cover to: {}", path);
                        playlist.cover_path = Some(path.clone());

                        // Also extract color palette from the downloaded cover
                        let palette = crate::utils::ColorPalette::from_image_path(
                            std::path::Path::new(&path),
                        );
                        tracing::info!(
                            "Updated palette from cover: primary=({:.2}, {:.2}, {:.2})",
                            palette.primary.r,
                            palette.primary.g,
                            palette.primary.b
                        );
                        playlist.palette = palette;
                    }
                }
                Some(Task::none())
            }

            Message::NcmPlaylistCreatorAvatarLoaded(playlist_id, path) => {
                tracing::info!(
                    "Playlist creator avatar loaded: id={}, path={}",
                    playlist_id,
                    path
                );
                // Update the playlist owner avatar path
                if let Some(playlist) = &mut self.ui.playlist_page.current {
                    if playlist.id == *playlist_id {
                        playlist.owner_avatar_path = Some(path.clone());
                        if playlist.kind == crate::ui::pages::playlist::DetailPageKind::User
                            && playlist.cover_path.is_none()
                        {
                            playlist.cover_path = Some(path.clone());
                            playlist.palette = crate::utils::ColorPalette::from_image_path(
                                std::path::Path::new(path),
                            );
                        }
                    }
                }
                Some(Task::none())
            }

            Message::ArtistCoverLoaded(artist_id, path) => {
                if let Some(playlist) = &mut self.ui.playlist_page.current {
                    if playlist.id == *artist_id {
                        playlist.cover_path = Some(path.clone());
                        playlist.palette =
                            crate::utils::ColorPalette::from_image_path(std::path::Path::new(path));
                    }
                }
                Some(Task::none())
            }

            Message::AlbumCoverLoaded(page_id, path) => {
                if let Some(playlist) = &mut self.ui.playlist_page.current {
                    if playlist.id == *page_id {
                        playlist.cover_path = Some(path.clone());
                        playlist.palette =
                            crate::utils::ColorPalette::from_image_path(std::path::Path::new(path));
                        if playlist.kind == crate::ui::pages::playlist::DetailPageKind::Album {
                            for song in &mut playlist.songs {
                                song.cover_path = Some(path.clone());
                            }
                        }
                    }
                }
                Some(Task::none())
            }

            Message::PlaylistCreatorDetailLoaded(playlist_id, detail) => {
                if let Some(playlist) = &mut self.ui.playlist_page.current {
                    if playlist.id == *playlist_id && detail.artist_id != 0 {
                        playlist.owner_artist_id = Some(detail.artist_id);
                    }
                }
                Some(Task::none())
            }

            Message::TogglePlaylistSubscribe(playlist_id) => {
                if !self.core.is_logged_in {
                    return Some(Task::done(Message::ShowWarningToast(
                        "请先登录".to_string(),
                    )));
                }

                // Get current subscription status
                let is_subscribed = self
                    .ui
                    .playlist_page
                    .current
                    .as_ref()
                    .map(|p| p.is_subscribed)
                    .unwrap_or(false);

                if let Some(client) = &self.core.ncm_client {
                    let client = client.clone();
                    let playlist_id = *playlist_id;
                    let new_status = !is_subscribed;

                    Some(Task::perform(
                        async move {
                            // NCM playlist IDs are stored as negative in our system
                            let ncm_id = (-playlist_id) as u64;
                            match client.client.playlist_subscribe(new_status, ncm_id).await {
                                Ok(_) => Some((playlist_id, new_status)),
                                Err(e) => {
                                    error!("Failed to toggle playlist subscription: {}", e);
                                    None
                                }
                            }
                        },
                        |result| {
                            if let Some((id, subscribed)) = result {
                                Message::PlaylistSubscribeChanged(id, subscribed)
                            } else {
                                Message::ShowErrorToast("操作失败".to_string())
                            }
                        },
                    ))
                } else {
                    Some(Task::none())
                }
            }

            Message::PlaylistSubscribeChanged(playlist_id, subscribed) => {
                // Update the subscription status in the current playlist view
                if let Some(playlist) = &mut self.ui.playlist_page.current {
                    if playlist.id == *playlist_id {
                        playlist.is_subscribed = *subscribed;
                    }
                }
                let msg = if *subscribed {
                    "已收藏歌单"
                } else {
                    "已取消收藏"
                };
                Some(Task::done(Message::ShowSuccessToast(msg.to_string())))
            }

            _ => None,
        }
    }

    /// Load homepage data (banners, top picks, trending songs)
    fn load_homepage_data(&self) -> Task<Message> {
        let client = self.core.ncm_client.clone();

        Task::batch([
            Task::perform(
                {
                    let client = client.clone();
                    async move {
                        if let Some(client) = client {
                            match client.client.banners().await {
                                Ok(banners) => banners,
                                Err(e) => {
                                    error!("Failed to load banners: {:?}", e);
                                    Vec::new()
                                }
                            }
                        } else {
                            Vec::new()
                        }
                    }
                },
                Message::BannersLoaded,
            ),
            Task::perform(
                {
                    let client = client.clone();
                    async move {
                        if let Some(client) = client {
                            const TRENDING_CHART_ID: u64 = 19723756;
                            match client.client.song_list_detail(TRENDING_CHART_ID).await {
                                Ok(detail) => detail.songs,
                                Err(e) => {
                                    error!("Failed to load trending songs: {:?}", e);
                                    Vec::new()
                                }
                            }
                        } else {
                            Vec::new()
                        }
                    }
                },
                Message::TrendingSongsLoaded,
            ),
            Task::perform(
                {
                    let client = client.clone();
                    async move {
                        if let Some(client) = client {
                            match client.client.top_song_list("全部", "hot", 0, 8).await {
                                Ok(playlists) => playlists,
                                Err(e) => {
                                    error!("Failed to load top picks: {:?}", e);
                                    Vec::new()
                                }
                            }
                        } else {
                            Vec::new()
                        }
                    }
                },
                Message::TopPicksLoaded,
            ),
        ])
    }

    /// Load user playlists (liked songs + collected playlists)
    fn load_user_playlists(&self) -> Task<Message> {
        let client = self.core.ncm_client.clone();
        let uid = self.core.user_info.as_ref().map(|u| u.user_id).unwrap_or(0);
        let nickname = self
            .core
            .user_info
            .as_ref()
            .map(|u| u.nickname.clone())
            .unwrap_or_default();

        if uid == 0 {
            return Task::none();
        }

        Task::perform(
            async move {
                if let Some(client) = client {
                    match client.client.user_song_list(uid, 0, 100).await {
                        Ok(mut playlists) => {
                            // First playlist is "liked songs", rename it
                            if let Some(first) = playlists.first_mut() {
                                first.name = format!("{} 喜欢的音乐", nickname);
                            }
                            playlists
                        }
                        Err(e) => {
                            error!("Failed to load user playlists: {:?}", e);
                            Vec::new()
                        }
                    }
                } else {
                    Vec::new()
                }
            },
            Message::UserPlaylistsLoaded,
        )
    }

    /// Load discover page data (recommended playlists for logged-in users, hot playlists for all)
    pub fn load_discover_data(&mut self) -> Task<Message> {
        self.ui.discover.data_loaded = true;
        self.ui.discover.recommended_loading = true;
        self.ui.discover.hot_loading = true;

        let client = self.core.ncm_client.clone();
        let is_logged_in = self.core.is_logged_in;

        let mut tasks = Vec::new();

        // Load recommended playlists (only for logged-in users)
        if is_logged_in {
            tasks.push(Task::perform(
                {
                    let client = client.clone();
                    async move {
                        if let Some(client) = client {
                            match client.client.recommend_resource().await {
                                Ok(playlists) => playlists,
                                Err(e) => {
                                    error!("Failed to load recommended playlists: {:?}", e);
                                    Vec::new()
                                }
                            }
                        } else {
                            Vec::new()
                        }
                    }
                },
                Message::RecommendedPlaylistsLoaded,
            ));
        }

        // Load hot playlists (for all users)
        tasks.push(Task::perform(
            {
                let client = client.clone();
                async move {
                    if let Some(client) = client {
                        match client.client.top_song_list("全部", "hot", 0, 30).await {
                            Ok(playlists) => {
                                let has_more = playlists.len() >= 30;
                                (playlists, has_more)
                            }
                            Err(e) => {
                                error!("Failed to load hot playlists: {:?}", e);
                                (Vec::new(), false)
                            }
                        }
                    } else {
                        (Vec::new(), false)
                    }
                }
            },
            |(playlists, has_more)| Message::HotPlaylistsLoaded(playlists, has_more),
        ));

        Task::batch(tasks)
    }
}
