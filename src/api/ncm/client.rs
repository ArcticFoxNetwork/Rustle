use anyhow::{Result, anyhow};
use ncm_api_rs::{ApiClient, Query, create_client};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{fs, io, path::PathBuf};
use tracing::error;

use super::mapper::{self, AlbumSource, PlaylistSource};
use super::models::*;

const COOKIE_FILE: &str = "cookies.json";
const DEFAULT_QUALITY: u32 = 2;
const PLAYLIST_DETAIL_CHUNK_SIZE: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PersistedSession {
    cookie: String,
}

#[derive(Clone)]
pub struct NcmClient {
    client: ApiClient,
    cookie: Arc<parking_lot::RwLock<String>>,
    proxy: Option<String>,
    quality: Arc<AtomicU32>,
}

impl std::fmt::Debug for NcmClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NcmClient")
            .field("client", &"<ncm_api_rs::ApiClient>")
            .field("has_cookie", &!self.cookie.read().is_empty())
            .field("proxy", &self.proxy)
            .finish()
    }
}

impl Default for NcmClient {
    fn default() -> Self {
        Self::new()
    }
}

impl NcmClient {
    pub fn new() -> Self {
        Self::from_cookie_and_proxy(None, None)
    }

    pub fn with_proxy(proxy_url: Option<String>) -> Self {
        Self::from_cookie_and_proxy(None, proxy_url)
    }

    pub fn from_cookie(cookie: String) -> Self {
        Self::from_cookie_and_proxy(Some(cookie), None)
    }

    pub fn from_cookie_with_proxy(cookie: String, proxy_url: Option<String>) -> Self {
        Self::from_cookie_and_proxy(Some(cookie), proxy_url)
    }

    fn from_cookie_and_proxy(cookie: Option<String>, proxy: Option<String>) -> Self {
        let client = create_client(cookie.clone());
        Self {
            client,
            cookie: Arc::new(parking_lot::RwLock::new(cookie.unwrap_or_default())),
            proxy,
            quality: Arc::new(AtomicU32::new(DEFAULT_QUALITY)),
        }
    }

    fn data_dir() -> PathBuf {
        directories::ProjectDirs::from("life", "fxs", "rustle")
            .map(|dirs| dirs.data_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    pub fn cookie_file_path() -> PathBuf {
        let data_dir = Self::data_dir();
        fs::create_dir_all(&data_dir).ok();
        data_dir.join(COOKIE_FILE)
    }

    pub fn load_cookie_from_file() -> Option<String> {
        match fs::File::open(Self::cookie_file_path()) {
            Err(err) => match err.kind() {
                io::ErrorKind::NotFound => None,
                other => {
                    error!("{:?}", other);
                    None
                }
            },
            Ok(file) => match serde_json::from_reader::<_, PersistedSession>(file) {
                Ok(session) if !session.cookie.trim().is_empty() => Some(session.cookie),
                Ok(_) => None,
                Err(err) => {
                    error!("{:?}", err);
                    None
                }
            },
        }
    }

    pub fn save_cookie_to_file(&self) {
        let cookie = self.cookie.read().clone();
        if cookie.trim().is_empty() {
            return;
        }

        match fs::File::create(Self::cookie_file_path()) {
            Ok(file) => {
                if let Err(err) = serde_json::to_writer(file, &PersistedSession { cookie }) {
                    error!("Failed to save cookies: {:?}", err);
                }
            }
            Err(err) => error!("{:?}", err),
        }
    }

    pub fn clean_cookie_file() {
        if let Err(err) = fs::remove_file(Self::cookie_file_path()) {
            match err.kind() {
                io::ErrorKind::NotFound => (),
                other => error!("{:?}", other),
            }
        }
    }

    pub fn set_proxy(&mut self, proxy: String) -> Result<()> {
        self.proxy = Some(proxy);
        Ok(())
    }

    pub fn set_quality(&self, quality: u32) {
        self.quality.store(quality, Ordering::Relaxed);
        tracing::info!(
            "Music quality set to: {} ({})",
            quality,
            Self::quality_to_level(quality)
        );
    }

    pub fn quality(&self) -> u32 {
        self.quality.load(Ordering::Relaxed)
    }

    fn quality_to_level(quality: u32) -> &'static str {
        match quality {
            0 => "standard",
            1 => "higher",
            2 => "exhigh",
            3 => "lossless",
            4 => "hires",
            _ => "exhigh",
        }
    }

    fn current_level(&self) -> &'static str {
        Self::quality_to_level(self.quality())
    }

    fn query(&self) -> Query {
        let mut query = Query::new();
        let cookie = self.cookie.read().clone();
        if !cookie.trim().is_empty() {
            query = query.cookie(&cookie);
        }
        if let Some(proxy) = &self.proxy {
            query.proxy = Some(proxy.clone());
        }
        query
    }

    fn remember_cookies(&self, cookies: Vec<String>) {
        if cookies.is_empty() {
            return;
        }

        let mut stored = self.cookie.write();
        let mut pairs: Vec<(String, String)> = stored.split(';').filter_map(cookie_pair).collect();

        for raw in cookies {
            if let Some((name, value)) = cookie_pair(&raw) {
                if let Some((_, existing)) = pairs.iter_mut().find(|(key, _)| key == &name) {
                    *existing = value;
                } else {
                    pairs.push((name, value));
                }
            }
        }

        *stored = pairs
            .into_iter()
            .map(|(name, value)| format!("{}={}", name, value))
            .collect::<Vec<_>>()
            .join("; ");
    }

    pub async fn create_qrcode(&self) -> Result<(PathBuf, String)> {
        let response = self.client.login_qr_key(&self.query()).await?;
        let unikey = response
            .body
            .get("data")
            .and_then(|data| data.get("unikey"))
            .or_else(|| response.body.get("unikey"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("QR key missing"))?
            .to_string();
        let qr_url = format!("https://music.163.com/login?codekey={}", unikey);

        let cache_dir = crate::utils::cache_dir();
        fs::create_dir_all(&cache_dir)?;

        if let Ok(entries) = fs::read_dir(&cache_dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                if name.starts_with("qrimage_") && name.ends_with(".png") {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let path = cache_dir.join(format!("qrimage_{}.png", timestamp));
        qrcode_generator::to_png_to_file(qr_url, qrcode_generator::QrCodeEcc::Low, 200, &path)?;
        Ok((path, unikey))
    }

    pub async fn login_qr_check(&self, key: String) -> Result<Msg> {
        let query = self.query().param("key", &key);
        let response = self.client.login_qr_check(&query).await?;
        self.remember_cookies(response.cookie);
        Ok(mapper::msg(&response.body))
    }

    pub async fn login_status(&self) -> Result<LoginInfo> {
        let response = self.client.login_status(&self.query()).await?;
        self.remember_cookies(response.cookie);
        mapper::login_info(&response.body)
    }

    pub async fn logout(&self) {
        let _ = self.client.logout(&self.query()).await;
        *self.cookie.write() = String::new();
    }

    pub async fn user_song_id_list(&self, user_id: u64) -> Result<Vec<u64>> {
        let query = self.query().param("uid", &user_id.to_string());
        let response = self.client.likelist(&query).await?;
        mapper::liked_song_ids(&response.body)
    }

    pub async fn user_playlists(
        &self,
        user_id: u64,
        offset: u16,
        limit: u16,
    ) -> Result<Vec<PlaylistSummary>> {
        let query = self
            .query()
            .param("uid", &user_id.to_string())
            .param("offset", &offset.to_string())
            .param("limit", &limit.to_string());
        let response = self.client.user_playlist(&query).await?;
        mapper::playlist_summaries(&response.body, PlaylistSource::User)
    }

    pub async fn playlist_detail(&self, playlist_id: u64) -> Result<PlaylistDetail> {
        let query = self.query().param("id", &playlist_id.to_string());
        let response = self.client.playlist_detail(&query).await?;
        let body = response.body;
        let mut detail = mapper::playlist_detail(&body)?;

        if detail.track_count > detail.tracks.len() as u64 {
            let track_ids = mapper::playlist_track_ids(&body);
            if track_ids.is_empty() {
                return Err(anyhow!("playlist track ids missing"));
            }

            let fetch_limit = detail.track_count.min(track_ids.len() as u64) as usize;
            let mut tracks = Vec::with_capacity(fetch_limit);
            for chunk in track_ids[..fetch_limit].chunks(PLAYLIST_DETAIL_CHUNK_SIZE) {
                tracks.extend(self.track_detail(chunk).await?);
            }

            if tracks.len() < fetch_limit {
                return Err(anyhow!(
                    "playlist tracks incomplete: expected {}, got {}",
                    fetch_limit,
                    tracks.len()
                ));
            }

            detail.tracks = tracks;
        }

        Ok(detail)
    }

    pub async fn artist_detail(&self, artist_id: u64) -> Result<ArtistDetail> {
        let query = self.query().param("id", &artist_id.to_string());
        let response = self.client.artists(&query).await?;
        mapper::artist_detail(&response.body)
    }

    pub async fn album_detail(&self, album_id: u64) -> Result<AlbumDetail> {
        let query = self.query().param("id", &album_id.to_string());
        let response = self.client.album(&query).await?;
        mapper::album_detail(&response.body)
    }

    pub async fn user_detail(&self, user_id: u64) -> Result<UserDetail> {
        let query = self.query().param("uid", &user_id.to_string());
        let response = self.client.user_detail(&query).await?;
        mapper::user_detail(&response.body)
    }

    pub async fn track_urls(&self, ids: &[u64]) -> Result<Vec<TrackUrl>> {
        let id_list = ids.iter().map(u64::to_string).collect::<Vec<_>>().join(",");
        let query = self
            .query()
            .param("id", &id_list)
            .param("level", self.current_level());
        let response = self.client.song_url_v1(&query).await?;
        mapper::track_urls(&response.body)
    }

    pub async fn track_detail(&self, ids: &[u64]) -> Result<Vec<Track>> {
        let id_list = ids.iter().map(u64::to_string).collect::<Vec<_>>().join(",");
        let query = self.query().param("ids", &id_list);
        let response = self.client.song_detail(&query).await?;
        mapper::track_detail(&response.body)
    }

    pub async fn song_lyric(&self, music_id: u64) -> Result<Lyrics> {
        let query = self.query().param("id", &music_id.to_string());
        // Use the v1 endpoint, which is the only standard NCM endpoint that
        // returns the word-level YRC payload. The legacy endpoint only
        // reliably returns line-level LRC data.
        let response = self.client.lyric_new(&query).await?;
        mapper::lyrics(&response.body)
    }

    pub async fn scrobble_song(
        &self,
        song_id: u64,
        source_id: Option<u64>,
        time_secs: u64,
    ) -> Result<()> {
        let mut query = self
            .query()
            .param("id", &song_id.to_string())
            .param("time", &time_secs.to_string());
        let source_id = source_id.map(|id| id.to_string()).unwrap_or_default();
        if !source_id.is_empty() {
            query = query.param("sourceid", &source_id);
        }
        self.client.scrobble(&query).await?;
        Ok(())
    }

    pub async fn recommend_playlists(&self) -> Result<Vec<PlaylistSummary>> {
        let response = self.client.recommend_resource(&self.query()).await?;
        mapper::playlist_summaries(&response.body, PlaylistSource::Recommend)
    }

    pub async fn recommend_tracks(&self) -> Result<Vec<Track>> {
        let response = self.client.recommend_songs(&self.query()).await?;
        Ok(mapper::tracks_from_array(
            response
                .body
                .get("data")
                .and_then(|data| data.get("dailySongs"))
                .and_then(serde_json::Value::as_array),
            None,
        ))
    }

    pub async fn top_playlists(
        &self,
        cat: &str,
        order: &str,
        offset: u16,
        limit: u16,
    ) -> Result<Vec<PlaylistSummary>> {
        let query = self
            .query()
            .param("cat", cat)
            .param("order", order)
            .param("offset", &offset.to_string())
            .param("limit", &limit.to_string());
        let response = self.client.top_playlist(&query).await?;
        mapper::playlist_summaries(&response.body, PlaylistSource::Top)
    }

    pub async fn like_song(&self, track_id: u64, like: bool) -> Result<()> {
        let query = self
            .query()
            .param("id", &track_id.to_string())
            .param("like", if like { "true" } else { "false" });
        self.client.like(&query).await?;
        Ok(())
    }

    pub async fn banners(&self) -> Result<Vec<Banner>> {
        let response = self.client.banner(&self.query()).await?;
        mapper::banners(&response.body)
    }

    pub async fn download_img<I>(
        &self,
        url: I,
        path: PathBuf,
        width: u16,
        height: u16,
    ) -> Result<()>
    where
        I: Into<String>,
    {
        if !path.exists() {
            let url = url.into();
            let image_url = format!("{}?param={}y{}", url, width, height);
            let response = reqwest::Client::new().get(&image_url).send().await?;
            if response.status().is_success() {
                let bytes = response.bytes().await?;
                std::fs::write(&path, bytes)?;
            }
        }
        Ok(())
    }

    pub async fn playlist_subscribe(&self, subscribe: bool, playlist_id: u64) -> Result<()> {
        let query = self
            .query()
            .param("id", &playlist_id.to_string())
            .param("t", if subscribe { "1" } else { "0" });
        self.client.playlist_subscribe(&query).await?;
        Ok(())
    }

    pub async fn personal_fm_tracks(&self) -> Result<Vec<Track>> {
        let response = self.client.personal_fm(&self.query()).await?;
        Ok(mapper::tracks_from_array(
            response
                .body
                .get("data")
                .and_then(serde_json::Value::as_array),
            None,
        ))
    }

    pub async fn search(
        &self,
        keywords: &str,
        search_type: SearchType,
        limit: u32,
        offset: u32,
    ) -> Result<SearchResponse> {
        let query = self
            .query()
            .param("keywords", keywords)
            .param("type", search_type.as_str())
            .param("limit", &limit.to_string())
            .param("offset", &offset.to_string());
        let response = self.client.cloudsearch(&query).await?;
        mapper::search(&response.body, search_type)
    }

    pub async fn artist_albums(&self, artist_id: u64, limit: u32) -> Result<Vec<AlbumSummary>> {
        let query = self
            .query()
            .param("id", &artist_id.to_string())
            .param("limit", &limit.to_string())
            .param("offset", "0");
        let response = self.client.artist_album(&query).await?;
        mapper::album_summaries(&response.body, AlbumSource::Artist)
    }

    pub async fn playlist_add_tracks(&self, pid: u64, track_ids: &str, op: &str) -> Result<()> {
        let query = self
            .query()
            .param("pid", &pid.to_string())
            .param("tracks", track_ids)
            .param("op", op);
        self.client.playlist_tracks(&query).await?;
        Ok(())
    }
}

fn cookie_pair(raw: &str) -> Option<(String, String)> {
    let pair = raw.split(';').next()?.trim();
    let (name, value) = pair.split_once('=')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    Some((name.to_string(), value.trim().to_string()))
}
