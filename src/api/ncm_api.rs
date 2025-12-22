//! Netease Cloud Music API - Local Implementation
//!
//! Core API client for NCM with encryption and model types.

mod encrypt;
pub mod model;

use anyhow::{Result, anyhow};
use encrypt::Crypto;
pub use isahc::cookies::{CookieBuilder, CookieJar};
use isahc::{prelude::*, *};
use lazy_static::lazy_static;
pub use model::*;
use parking_lot::RwLock;
use regex::Regex;
use std::fmt;
use std::sync::Arc;
use std::{collections::HashMap, path::PathBuf, time::Duration};

lazy_static! {
    static ref _CSRF: Regex = Regex::new(r"_csrf=(?P<csrf>[^(;|$)]+)").unwrap();
}

static BASE_URL: &str = "https://music.163.com";

const TIMEOUT: u64 = 100;

const LINUX_USER_AGNET: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/60.0.3112.90 Safari/537.36";

const USER_AGENT_LIST: [&str; 14] = [
    "Mozilla/5.0 (iPhone; CPU iPhone OS 9_1 like Mac OS X) AppleWebKit/601.1.46 (KHTML, like Gecko) Version/9.0 Mobile/13B143 Safari/601.1",
    "Mozilla/5.0 (iPhone; CPU iPhone OS 9_1 like Mac OS X) AppleWebKit/601.1.46 (KHTML, like Gecko) Version/9.0 Mobile/13B143 Safari/601.1",
    "Mozilla/5.0 (Linux; Android 5.0; SM-G900P Build/LRX21T) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/59.0.3071.115 Mobile Safari/537.36",
    "Mozilla/5.0 (Linux; Android 6.0; Nexus 5 Build/MRA58N) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/59.0.3071.115 Mobile Safari/537.36",
    "Mozilla/5.0 (Linux; Android 5.1.1; Nexus 6 Build/LYZ28E) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/59.0.3071.115 Mobile Safari/537.36",
    "Mozilla/5.0 (iPhone; CPU iPhone OS 10_3_2 like Mac OS X) AppleWebKit/603.2.4 (KHTML, like Gecko) Mobile/14F89;GameHelper",
    "Mozilla/5.0 (iPhone; CPU iPhone OS 10_0 like Mac OS X) AppleWebKit/602.1.38 (KHTML, like Gecko) Version/10.0 Mobile/14A300 Safari/602.1",
    "Mozilla/5.0 (iPad; CPU OS 10_0 like Mac OS X) AppleWebKit/602.1.38 (KHTML, like Gecko) Version/10.0 Mobile/14A300 Safari/602.1",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.12; rv:46.0) Gecko/20100101 Firefox/46.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_12_5) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/59.0.3071.115 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_12_5) AppleWebKit/603.2.4 (KHTML, like Gecko) Version/10.1.1 Safari/603.2.4",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:46.0) Gecko/20100101 Firefox/46.0",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/51.0.2704.103 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/42.0.2311.135 Safari/537.36 Edge/13.1058",
];

#[derive(Clone)]
pub struct MusicApi {
    client: HttpClient,
    csrf: Arc<RwLock<String>>,
}

impl fmt::Debug for MusicApi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MusicApi")
            .field("client", &"<HttpClient>")
            .field("csrf", &"<RwLock<String>>")
            .finish()
    }
}

enum CryptoApi {
    Weapi,
    LinuxApi,
    Eapi,
}

impl Default for MusicApi {
    fn default() -> Self {
        Self::new(0)
    }
}

impl MusicApi {
    pub fn new(max_cons: usize) -> Self {
        let client = HttpClient::builder()
            .timeout(Duration::from_secs(TIMEOUT))
            .max_connections(max_cons)
            .cookies()
            .build()
            .expect("初始化网络请求失败!");
        Self {
            client,
            csrf: Arc::new(RwLock::new(String::new())),
        }
    }

    pub fn from_cookie_jar(cookie_jar: CookieJar, max_cons: usize) -> Self {
        let client = HttpClient::builder()
            .timeout(Duration::from_secs(TIMEOUT))
            .max_connections(max_cons)
            .cookies()
            .cookie_jar(cookie_jar.to_owned())
            .build()
            .expect("初始化网络请求失败!");
        Self {
            client,
            csrf: Arc::new(RwLock::new(String::new())),
        }
    }

    pub fn cookie_jar(&self) -> Option<&CookieJar> {
        self.client.cookie_jar()
    }

    pub fn set_proxy(&mut self, proxy: &str) -> Result<()> {
        if let Some(cookie_jar) = self.client.cookie_jar() {
            let client = HttpClient::builder()
                .timeout(Duration::from_secs(TIMEOUT))
                .proxy(Some(proxy.parse()?))
                .cookies()
                .cookie_jar(cookie_jar.to_owned())
                .build()
                .expect("初始化网络请求失败!");
            self.client = client;
        } else {
            let client = HttpClient::builder()
                .timeout(Duration::from_secs(TIMEOUT))
                .proxy(Some(proxy.parse()?))
                .cookies()
                .build()
                .expect("初始化网络请求失败!");
            self.client = client;
        }
        Ok(())
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        params: HashMap<&str, &str>,
        cryptoapi: CryptoApi,
        ua: &str,
        append_csrf: bool,
    ) -> Result<String> {
        let mut csrf = self.csrf.read().clone();
        if csrf.is_empty() {
            if let Some(cookies) = self.cookie_jar() {
                let uri = BASE_URL.parse().unwrap();
                if let Some(cookie) = cookies.get_by_name(&uri, "__csrf") {
                    let __csrf = cookie.value().to_string();
                    *self.csrf.write() = __csrf.clone();
                    csrf = __csrf;
                }
            }
        }
        let mut url = format!("{}{}?csrf_token={}", BASE_URL, path, csrf);
        if !append_csrf {
            url = format!("{}{}", BASE_URL, path);
        }
        match method {
            Method::Post => {
                let user_agent = match cryptoapi {
                    CryptoApi::LinuxApi => LINUX_USER_AGNET.to_string(),
                    CryptoApi::Weapi => choose_user_agent(ua).to_string(),
                    CryptoApi::Eapi => choose_user_agent(ua).to_string(),
                };
                let body = match cryptoapi {
                    CryptoApi::LinuxApi => {
                        let data = format!(
                            r#"{{"method":"linuxapi","url":"{}","params":{}}}"#,
                            url.replace("weapi", "api"),
                            serde_json::to_string(&params)?
                        );
                        Crypto::linuxapi(&data)
                    }
                    CryptoApi::Weapi => {
                        let mut params = params;
                        params.insert("csrf_token", &csrf);
                        Crypto::weapi(&serde_json::to_string(&params)?)
                    }
                    CryptoApi::Eapi => {
                        let mut params = params;
                        params.insert("csrf_token", &csrf);
                        url = path.to_string();
                        Crypto::eapi(
                            "/api/song/enhance/player/url",
                            &serde_json::to_string(&params)?,
                        )
                    }
                };

                let request = Request::post(&url)
                    .header("Cookie", "os=pc; appver=2.7.1.198277")
                    .header("Accept", "*/*")
                    .header("Accept-Language", "en-US,en;q=0.5")
                    .header("Connection", "keep-alive")
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .header("Host", "music.163.com")
                    .header("Referer", "https://music.163.com")
                    .header("User-Agent", user_agent)
                    .body(body)
                    .unwrap();
                let mut response = self
                    .client
                    .send_async(request)
                    .await
                    .map_err(|_| anyhow!("none"))?;
                response.text().await.map_err(|_| anyhow!("none"))
            }
            Method::Get => self
                .client
                .get_async(&url)
                .await
                .map_err(|_| anyhow!("none"))?
                .text()
                .await
                .map_err(|_| anyhow!("none")),
        }
    }

    pub async fn login_qr_create(&self) -> Result<(String, String)> {
        let path = "/weapi/login/qrcode/unikey";
        let mut params = HashMap::new();
        params.insert("type", "1");
        let result = self
            .request(Method::Post, path, params, CryptoApi::Weapi, "", true)
            .await?;
        let unikey = to_unikey(result)?;
        Ok((
            format!("https://music.163.com/login?codekey={}", &unikey),
            unikey,
        ))
    }

    pub async fn login_qr_check(&self, key: String) -> Result<Msg> {
        let path = "/weapi/login/qrcode/client/login";
        let mut params = HashMap::new();
        params.insert("type", "1");
        params.insert("key", &key);
        let result = self
            .request(Method::Post, path, params, CryptoApi::Weapi, "", true)
            .await?;
        to_message(result)
    }

    pub async fn login_status(&self) -> Result<LoginInfo> {
        let path = "/api/nuser/account/get";
        let result = self
            .request(
                Method::Post,
                path,
                HashMap::new(),
                CryptoApi::Weapi,
                "",
                true,
            )
            .await?;
        to_login_info(result)
    }

    pub async fn logout(&self) {
        let path = "https://music.163.com/weapi/logout";
        let _ = self
            .request(
                Method::Post,
                path,
                HashMap::new(),
                CryptoApi::Weapi,
                "pc",
                true,
            )
            .await;
    }

    pub async fn user_song_id_list(&self, uid: u64) -> Result<Vec<u64>> {
        let path = "/weapi/song/like/get";
        let mut params = HashMap::new();
        let uid = uid.to_string();
        params.insert("uid", uid.as_str());
        let result = self
            .request(Method::Post, path, params, CryptoApi::Weapi, "", true)
            .await?;
        to_song_id_list(result)
    }

    pub async fn user_song_list(&self, uid: u64, offset: u16, limit: u16) -> Result<Vec<SongList>> {
        let path = "/weapi/user/playlist";
        let mut params = HashMap::new();
        let uid = uid.to_string();
        let offset = offset.to_string();
        let limit = limit.to_string();
        params.insert("uid", uid.as_str());
        params.insert("offset", offset.as_str());
        params.insert("limit", limit.as_str());
        let result = self
            .request(Method::Post, path, params, CryptoApi::Weapi, "", true)
            .await?;
        to_song_list(result, Parse::Usl)
    }

    pub async fn user_cloud_disk(&self) -> Result<Vec<SongInfo>> {
        let path = "/weapi/v1/cloud/get";
        let mut params = HashMap::new();
        params.insert("offset", "0");
        params.insert("limit", "10000");
        let result = self
            .request(Method::Post, path, params, CryptoApi::Weapi, "", true)
            .await?;
        to_song_info(result, Parse::Ucd)
    }

    pub async fn song_list_detail(&self, songlist_id: u64) -> Result<PlayListDetail> {
        let csrf_token = self.csrf.read().clone();
        let path = "/weapi/v6/playlist/detail";
        let mut params = HashMap::new();
        let songlist_id_str = songlist_id.to_string();
        params.insert("id", songlist_id_str.as_str());
        params.insert("offset", "0");
        params.insert("total", "true");
        params.insert("limit", "1000");
        params.insert("n", "1000");
        params.insert("csrf_token", &csrf_token);
        let result = self
            .request(Method::Post, path, params, CryptoApi::Weapi, "", true)
            .await?;
        let mut detail = to_mix_detail(&serde_json::from_str(&result)?)?;

        // If there are more songs than we got, fetch the rest using song_detail
        if detail.track_count > detail.songs.len() as u64 {
            // Get all track IDs from the playlist
            let track_ids = self.playlist_track_ids(songlist_id).await?;

            // We already have the first batch, get the remaining song IDs
            let existing_ids: std::collections::HashSet<u64> =
                detail.songs.iter().map(|s| s.id).collect();
            let remaining_ids: Vec<u64> = track_ids
                .into_iter()
                .filter(|id| !existing_ids.contains(id))
                .collect();

            // Fetch remaining songs in batches of 500
            for chunk in remaining_ids.chunks(500) {
                if let Ok(songs) = self.song_detail(chunk).await {
                    detail.songs.extend(songs);
                }
            }
        }

        Ok(detail)
    }

    /// Get all track IDs from a playlist
    async fn playlist_track_ids(&self, playlist_id: u64) -> Result<Vec<u64>> {
        let csrf_token = self.csrf.read().clone();
        let path = "/weapi/v6/playlist/detail";
        let mut params = HashMap::new();
        let playlist_id_str = playlist_id.to_string();
        params.insert("id", playlist_id_str.as_str());
        params.insert("n", "0"); // Don't fetch song details, just metadata
        params.insert("s", "0"); // Don't fetch subscribers
        params.insert("csrf_token", &csrf_token);
        let result = self
            .request(Method::Post, path, params, CryptoApi::Weapi, "", true)
            .await?;

        let value: serde_json::Value = serde_json::from_str(&result)?;
        let code: i64 = value.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
        if code != 200 {
            return Err(anyhow!("Failed to get playlist track IDs"));
        }

        // Extract trackIds from playlist
        let track_ids: Vec<u64> = value
            .get("playlist")
            .and_then(|p| p.get("trackIds"))
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.get("id").and_then(|id| id.as_u64()))
                    .collect()
            })
            .unwrap_or_default();

        Ok(track_ids)
    }

    pub async fn songs_url(&self, ids: &[u64], br: &str) -> Result<Vec<SongUrl>> {
        let path = "https://interface3.music.163.com/eapi/song/enhance/player/url";
        let mut params = HashMap::new();
        let ids = serde_json::to_string(ids)?;
        params.insert("ids", ids.as_str());
        params.insert("br", br);
        let result = self
            .request(Method::Post, path, params, CryptoApi::Eapi, "", true)
            .await?;
        to_song_url(result)
    }

    pub async fn recommend_resource(&self) -> Result<Vec<SongList>> {
        let path = "/weapi/v1/discovery/recommend/resource";
        let result = self
            .request(
                Method::Post,
                path,
                HashMap::new(),
                CryptoApi::Weapi,
                "",
                true,
            )
            .await?;
        to_song_list(result, Parse::Rmd)
    }

    pub async fn recommend_songs(&self) -> Result<Vec<SongInfo>> {
        let path = "/weapi/v2/discovery/recommend/songs";
        let mut params = HashMap::new();
        params.insert("total", "ture");
        let result = self
            .request(Method::Post, path, params, CryptoApi::Weapi, "", true)
            .await?;
        to_song_info(result, Parse::Rmds)
    }

    pub async fn top_song_list(
        &self,
        cat: &str,
        order: &str,
        offset: u16,
        limit: u16,
    ) -> Result<Vec<SongList>> {
        let path = "/weapi/playlist/list";
        let mut params = HashMap::new();
        let offset = offset.to_string();
        let limit = limit.to_string();
        params.insert("cat", cat);
        params.insert("order", order);
        params.insert("total", "true");
        params.insert("offset", &offset[..]);
        params.insert("limit", &limit[..]);
        let result = self
            .request(Method::Post, path, params, CryptoApi::Weapi, "", true)
            .await?;
        to_song_list(result, Parse::Top)
    }

    pub async fn toplist(&self) -> Result<Vec<TopList>> {
        let path = "/api/toplist";
        let params = HashMap::new();
        let res = self
            .request(Method::Post, path, params, CryptoApi::Weapi, "", true)
            .await?;
        to_toplist(res)
    }

    pub async fn song_detail(&self, ids: &[u64]) -> Result<Vec<SongInfo>> {
        let path = "/weapi/v3/song/detail";
        let mut params = HashMap::new();
        let c = serde_json::to_string(
            &ids.iter()
                .map(|id| {
                    let mut map = HashMap::new();
                    map.insert("id", id);
                    map
                })
                .collect::<Vec<_>>(),
        )?;
        params.insert("c", &c[..]);
        let result = self
            .request(Method::Post, path, params, CryptoApi::Weapi, "", true)
            .await?;
        to_song_info(result, Parse::Usl)
    }

    pub async fn song_lyric(&self, music_id: u64) -> Result<Lyrics> {
        let csrf_token = self.csrf.read().clone();
        let path = "/weapi/song/lyric";
        let mut params = HashMap::new();
        let id = music_id.to_string();
        params.insert("id", &id[..]);
        params.insert("lv", "-1");
        params.insert("tv", "-1");
        params.insert("csrf_token", &csrf_token);
        let result = self
            .request(Method::Post, path, params, CryptoApi::Weapi, "", true)
            .await?;
        to_lyric(result)
    }

    /// Like or unlike a song
    pub async fn like_song(&self, track_id: u64, like: bool) -> Result<()> {
        let csrf_token = self.csrf.read().clone();
        let path = "/weapi/radio/like";
        let mut params = HashMap::new();
        params.insert("alg", "itembased");
        let track_id_str = track_id.to_string();
        let like_str = like.to_string();
        params.insert("trackId", &track_id_str);
        params.insert("like", &like_str);
        params.insert("time", "3");
        params.insert("csrf_token", &csrf_token);
        let _result = self
            .request(Method::Post, path, params, CryptoApi::Weapi, "", true)
            .await?;
        Ok(())
    }

    pub async fn banners(&self) -> Result<Vec<BannersInfo>> {
        let path = "/weapi/v2/banner/get";
        let mut params = HashMap::new();
        params.insert("clientType", "pc");
        let result = self
            .request(Method::Post, path, params, CryptoApi::Weapi, "", true)
            .await?;
        to_banners_info(result)
    }

    pub async fn download_img<I>(
        &self,
        url: I,
        path: PathBuf,
        _width: u16,
        _high: u16,
    ) -> Result<()>
    where
        I: Into<String>,
    {
        if !path.exists() {
            let url = url.into();
            let image_url = format!("{}?param={}y{}", url, _width, _high);

            let mut response = self.client.get_async(image_url).await?;
            if response.status().is_success() {
                let mut buf = vec![];
                response.copy_to(&mut buf).await?;
                std::fs::write(&path, buf)?;
            }
        }
        Ok(())
    }

    pub async fn download_file<I>(&self, url: I, path: PathBuf) -> Result<()>
    where
        I: Into<String>,
    {
        if !path.exists() {
            let url = url.into();
            let mut response = self.client.get_async(url).await?;
            if response.status().is_success() {
                let mut buf = vec![];
                response.copy_to(&mut buf).await?;
                std::fs::write(&path, buf)?;
            }
        }
        Ok(())
    }

    /// Subscribe or unsubscribe from a playlist
    /// subscribe: true to subscribe, false to unsubscribe
    /// playlist_id: playlist ID
    pub async fn playlist_subscribe(&self, subscribe: bool, playlist_id: u64) -> Result<()> {
        let path = if subscribe {
            "/weapi/playlist/subscribe"
        } else {
            "/weapi/playlist/unsubscribe"
        };
        let mut params = HashMap::new();
        let id = playlist_id.to_string();
        params.insert("id", id.as_str());
        let result = self
            .request(Method::Post, path, params, CryptoApi::Weapi, "", true)
            .await?;
        let msg = to_msg(result)?;
        if msg.code == 200 {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to {} playlist: {}",
                if subscribe {
                    "subscribe"
                } else {
                    "unsubscribe"
                },
                msg.msg
            ))
        }
    }
}

fn choose_user_agent(ua: &str) -> &str {
    let index = if ua == "mobile" {
        rand::random::<u16>() % 7
    } else if ua == "pc" {
        rand::random::<u16>() % 5 + 8
    } else if !ua.is_empty() {
        return ua;
    } else {
        rand::random::<u16>() % USER_AGENT_LIST.len() as u16
    };
    USER_AGENT_LIST[index as usize]
}
