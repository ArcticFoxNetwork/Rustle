//! NCM (Netease Cloud Music) related message handlers

use iced::Task;
use std::time::Duration;
use tracing::{debug, error, info};

use crate::api::{LoginInfo, NcmClient};
use crate::app::message::QrLoginStatus;
use crate::app::state::UserInfo;
use crate::app::{App, Message};

impl App {
    /// Set the NCM client and sync quality settings
    fn set_ncm_client(&mut self, client: NcmClient) {
        // Sync quality setting from app settings to client
        client.set_quality(self.core.settings.playback.music_quality.to_api_rate());
        self.core.ncm_client = Some(client);
    }

    /// Handle NCM-related messages
    pub fn handle_ncm(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::NcmClientReady(client) => {
                self.set_ncm_client(client.clone());
                Some(Task::done(Message::TryAutoLogin(0)))
            }

            Message::TryAutoLogin(retry_count) => {
                let retry = *retry_count;
                let proxy_url = self.core.settings.network.proxy_url();
                if let Some((cookie_jar, cookie_store, csrf_token)) =
                    NcmClient::load_cookie_jar_from_file()
                {
                    let client = NcmClient::from_cookie_jar_with_proxy(
                        cookie_jar,
                        cookie_store,
                        csrf_token,
                        proxy_url,
                    );
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
                    Task::done(Message::ShowToast("登录成功！".to_string())),
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

                Some(Task::done(Message::ShowToast("已退出登录".to_string())))
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
                                            Message::ShowToast("无法获取歌曲信息".to_string())
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

            Message::ToplistLoaded(toplists) => {
                self.ui.home.toplists = toplists.clone();
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

            Message::OpenTrendingSongs => {
                self.ui.active_nav = crate::ui::components::NavItem::Discover;
                Some(Task::none())
            }

            Message::SongCoverLoaded(song_id, path) => {
                // Create image handle for instant rendering (no disk IO in render loop)
                let handle = iced::widget::image::Handle::from_path(path);
                self.ui.home.song_covers.insert(*song_id, handle);
                Some(Task::none())
            }

            Message::ToggleFavorite(song_id) => {
                if !self.core.is_logged_in {
                    return Some(Task::done(Message::ShowToast("请先登录".to_string())));
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
                                Message::ShowToast("操作失败".to_string())
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
                if let Some(current) = &self.library.current_song {
                    if current.id < 0 && (-current.id) as u64 == *song_id {
                        let is_playing = self
                            .core
                            .audio
                            .as_ref()
                            .map(|p| p.is_playing())
                            .unwrap_or(false);
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

                if let Some(client) = &self.core.ncm_client {
                    let client = client.clone();
                    let song_info_clone = song_info.clone();

                    Some(Task::perform(
                        async move {
                            let song_cache_dir = crate::utils::songs_cache_dir();
                            let cover_cache_dir = crate::utils::covers_cache_dir();

                            if let Err(e) = std::fs::create_dir_all(&song_cache_dir) {
                                error!("Failed to create song cache dir: {}", e);
                                return None;
                            }
                            std::fs::create_dir_all(&cover_cache_dir).ok();

                            // Use stem for cache lookup - actual extension determined by format
                            let song_stem = song_info_clone.id.to_string();

                            // Handle Cover Image - use download_cover which handles format detection
                            let cover_path_str = crate::utils::download_cover(
                                &client,
                                song_info_clone.id,
                                &song_info_clone.pic_url,
                            )
                            .await
                            .map(|p| p.to_string_lossy().to_string());

                            // Handle Song File - check cache with any audio extension
                            if let Some(cached_path) = crate::utils::find_cached_audio(&song_cache_dir, &song_stem) {
                                debug!("Song found in cache: {:?}", cached_path);
                                return Some((
                                    song_info_clone,
                                    cached_path.to_string_lossy().to_string(),
                                    cover_path_str,
                                ));
                            }

                            // Download to temp file, then rename with correct extension
                            let temp_path = song_cache_dir.join(format!("{}.tmp", song_stem));

                            match client.songs_url(&[song_info_clone.id]).await {
                                Ok(urls) => {
                                    if let Some(song_url) = urls.first() {
                                        debug!("Got song URL: {}", song_url.url);
                                        if client
                                            .client
                                            .download_file(&song_url.url, temp_path.clone())
                                            .await
                                            .is_ok()
                                        {
                                            // Detect format and rename
                                            let ext = if let Ok(bytes) = std::fs::read(&temp_path) {
                                                crate::utils::detect_audio_format(&bytes)
                                            } else {
                                                "mp3"
                                            };
                                            let final_path = song_cache_dir.join(format!("{}.{}", song_stem, ext));
                                            if std::fs::rename(&temp_path, &final_path).is_ok() {
                                                Some((
                                                    song_info_clone,
                                                    final_path.to_string_lossy().to_string(),
                                                    cover_path_str,
                                                ))
                                            } else {
                                                let _ = std::fs::remove_file(&temp_path);
                                                error!("Failed to rename downloaded song");
                                                None
                                            }
                                        } else {
                                            error!("Failed to download song");
                                            None
                                        }
                                    } else {
                                        error!(
                                            "No song URL returned for song {}",
                                            song_info_clone.id
                                        );
                                        None
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to fetch song URL: {}", e);
                                    None
                                }
                            }
                        },
                        |result| {
                            if let Some((song_info, path, cover_path)) = result {
                                Message::PlayNcmUrl(song_info, path, cover_path)
                            } else {
                                Message::ShowToast("无法播放歌曲".to_string())
                            }
                        },
                    ))
                } else {
                    Some(Task::done(Message::ShowToast("请先登录".to_string())))
                }
            }

            Message::PlayNcmUrl(song_info, url, cover_override) => {
                debug!("Preparing NCM song: {} - {}", song_info.name, url);

                // Get cover path: use override, or check if cached cover exists
                let cover_path = cover_override.clone().or_else(|| {
                    let covers_dir = crate::utils::covers_cache_dir();
                    let cached_path = covers_dir.join(format!("{}.jpg", song_info.id));
                    if cached_path.exists() {
                        Some(cached_path.to_string_lossy().to_string())
                    } else {
                        None
                    }
                });

                let temp_song = crate::database::DbSong {
                    id: -(song_info.id as i64),
                    file_path: url.clone(),
                    title: song_info.name.clone(),
                    artist: song_info.singer.clone(),
                    album: song_info.album.clone(),
                    duration_secs: (song_info.duration / 1000) as i64,
                    track_number: None,
                    year: None,
                    genre: None,
                    cover_path: cover_path.clone(),
                    file_hash: None,
                    file_size: 0,
                    format: Some("mp3".to_string()),
                    play_count: 0,
                    last_played: Some(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs() as i64,
                    ),
                    last_modified: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64,
                    created_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64,
                };

                if let Some(db) = &self.core.db {
                    let db = db.clone();
                    let song_clone = temp_song.clone();
                    Some(Task::perform(
                        async move {
                            match db.upsert_ncm_song(&song_clone).await {
                                Ok(id) => Some(id),
                                Err(e) => {
                                    error!("Failed to upsert NCM song: {}", e);
                                    None
                                }
                            }
                        },
                        move |id_opt| {
                            if let Some(id) = id_opt {
                                let mut final_song = temp_song.clone();
                                final_song.id = id;
                                Message::PlayResolvedNcmSong(final_song)
                            } else {
                                Message::ShowToast("数据库错误".to_string())
                            }
                        },
                    ))
                } else {
                    Some(Task::done(Message::PlayResolvedNcmSong(temp_song)))
                }
            }

            Message::PlayResolvedNcmSong(song) => {
                if let Some(player) = &mut self.core.audio {
                    let path = std::path::PathBuf::from(&song.file_path);
                    match player.play(path) {
                        Ok(_) => {
                            info!("Started playing resolved NCM song: {}", song.title);

                            self.library.current_song = Some(song.clone());
                            self.library.queue.clear();
                            self.library.queue.push(song.clone());
                            self.library.queue_index = Some(0);

                            if let Some(db) = &self.core.db {
                                let db = db.clone();
                                let song_id = song.id;
                                tokio::spawn(async move {
                                    let _ = db.record_play(song_id, 0, false).await;
                                });
                            }

                            self.update_mpris_state();
                        }
                        Err(e) => {
                            error!("Failed to play NCM song: {}", e);
                            return Some(Task::done(Message::ShowToast("播放失败".to_string())));
                        }
                    }
                    return Some(Task::none());
                }

                Some(Task::none())
            }

            Message::AddNcmPlaylist(songs, play_now) => {
                debug!(
                    "Adding {} NCM songs to playlist, play_now: {}",
                    songs.len(),
                    play_now
                );

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
                        play_count: 0,
                        last_played: None,
                        last_modified: 0,
                        created_at: 0,
                    })
                    .collect();

                if *play_now {
                    self.library.queue = db_songs.clone();
                    self.library.queue_index = Some(0);

                    // Save queue to database
                    if let Some(db) = &self.core.db {
                        let db = db.clone();
                        let queue_clone = db_songs;
                        tokio::spawn(async move {
                            let _ = db.save_queue_with_songs(&queue_clone, None).await;
                        });
                    }

                    return Some(self.update(Message::PlayQueueIndex(0)));
                } else {
                    self.library.queue.extend(db_songs);

                    // Save updated queue to database
                    if let Some(db) = &self.core.db {
                        let db = db.clone();
                        let queue_clone = self.library.queue.clone();
                        tokio::spawn(async move {
                            let _ = db.save_queue_with_songs(&queue_clone, None).await;
                        });
                    }
                }

                Some(Task::none())
            }

            // PlayNcmQueueSong and NcmSongResolved are now handled by PlayQueueIndex and SongResolved in queue.rs
            Message::CloudPlaylistLoaded(songs) => {
                self.ui.home.cloud_songs = songs.clone();
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
                let playlist_id = *playlist_id;

                // Check if already viewing this playlist - skip redundant load
                if self.is_viewing_ncm_playlist(playlist_id) {
                    debug!(
                        "Already viewing NCM playlist {}, skipping load",
                        playlist_id
                    );
                    return Some(Task::none());
                }

                // Check if already loading
                if matches!(
                    self.ui.playlist_page.load_state,
                    crate::app::update::page_loader::PlaylistLoadState::Loading
                ) {
                    debug!("Playlist already loading, skipping");
                    return Some(Task::none());
                }

                debug!("Opening NCM playlist: {}", playlist_id);

                // Reset playlist search state
                self.ui.playlist_page.search_expanded = false;
                self.ui.playlist_page.search_query.clear();
                self.ui.playlist_page.viewing_recently_played = false;

                // Clear old animation states
                self.ui.clear_playlist_animations();

                // Try to get basic info from user_playlists for immediate skeleton display
                let basic_info = self
                    .ui
                    .home
                    .user_playlists
                    .iter()
                    .find(|p| p.id == playlist_id)
                    .map(|p| (p.name.clone(), p.author.clone(), p.cover_img_url.clone()));

                // Create skeleton PlaylistView immediately for instant UI response
                let internal_id = -(playlist_id as i64);
                let (name, owner, cover_url) = basic_info
                    .unwrap_or_else(|| ("加载中...".to_string(), String::new(), String::new()));

                let skeleton_view = crate::ui::pages::PlaylistView {
                    id: internal_id,
                    name,
                    description: None,
                    cover_path: None,
                    owner,
                    owner_avatar_path: None,
                    creator_id: 0,
                    song_count: 0,
                    total_duration: String::new(),
                    like_count: String::new(),
                    songs: Vec::new(),
                    palette: crate::utils::ColorPalette::default(),
                    is_local: false,
                    is_subscribed: false,
                };

                // Update state immediately - page will show skeleton
                self.ui.playlist_page.current = Some(skeleton_view);
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
                    .push(crate::app::state::NavigationEntry::NcmPlaylist(playlist_id));

                // Start cover download immediately if we have the URL
                let cover_task = if !cover_url.is_empty() {
                    if let Some(client) = &self.core.ncm_client {
                        let client = client.clone();
                        Task::perform(
                            async move {
                                crate::utils::download_playlist_cover(
                                    &client,
                                    playlist_id,
                                    &cover_url,
                                )
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

                // Start API request for full details
                let api_task = if let Some(client) = &self.core.ncm_client {
                    let client = client.clone();
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
                } else {
                    Task::none()
                };

                return Some(Task::batch([cover_task, api_task]));
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

                return Some(Task::batch([songs_task, avatar_task]));
            }

            Message::NcmPlaylistSongsReady(
                playlist_id,
                song_views,
                _cover_path,
                _palette,
                avatar_path,
            ) => {
                debug!("NCM playlist songs ready: {} songs", song_views.len());

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
                Some(iced::widget::operation::snap_to(
                    iced::widget::Id::new("playlist_scroll"),
                    iced::widget::scrollable::RelativeOffset { x: 0.0, y: 0.0 },
                ))
            }

            Message::CurrentSongCoverReady(song_id, path) => {
                tracing::info!(
                    "Current song cover downloaded: song_id={}, path={}",
                    song_id,
                    path
                );

                // Update current_song's cover_path
                if let Some(current) = &mut self.library.current_song {
                    if current.id == *song_id {
                        current.cover_path = Some(path.clone());
                    }
                }

                // Update in queue and database
                if let Some(idx) = self.library.queue_index {
                    if let Some(queue_song) = self.library.queue.get_mut(idx) {
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
                    if let Some(song) = self.library.current_song.clone() {
                        if song.id == *song_id {
                            return Some(self.update_lyrics_background_only(&song));
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
                Some(Task::none())
            }

            Message::RequestSongCoversLazy(requests) => {
                // Filter out songs that are already being downloaded or have covers
                let songs_to_download: Vec<_> = requests
                    .iter()
                    .filter(|(song_id, pic_url)| {
                        // Skip if already pending
                        if self
                            .ui
                            .playlist_page
                            .pending_cover_downloads
                            .contains(song_id)
                        {
                            return false;
                        }
                        // Skip if pic_url is empty
                        if pic_url.is_empty() {
                            return false;
                        }
                        // Skip if cover already exists locally
                        let cover_cache_dir = crate::utils::covers_cache_dir();
                        let ncm_id = if *song_id < 0 {
                            (-*song_id) as u64
                        } else {
                            *song_id as u64
                        };
                        let stem = format!("cover_{}", ncm_id);
                        crate::utils::find_cached_image(&cover_cache_dir, &stem).is_none()
                    })
                    .cloned()
                    .collect();

                if songs_to_download.is_empty() {
                    return Some(Task::none());
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
                    return Some(Task::perform(
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
                    ));
                }
                Some(Task::none())
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
                    }
                }
                Some(Task::none())
            }

            Message::TogglePlaylistSubscribe(playlist_id) => {
                if !self.core.is_logged_in {
                    return Some(Task::done(Message::ShowToast("请先登录".to_string())));
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
                                Message::ShowToast("操作失败".to_string())
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
                Some(Task::done(Message::ShowToast(msg.to_string())))
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
