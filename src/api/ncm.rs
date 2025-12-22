//! NCM Client implementation
//!
//! Wraps the local ncm_api module with cookie persistence and QR code login support.

use anyhow::Result;
use cookie_store::CookieStore;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{fs, io, path::PathBuf};
use tracing::{debug, error};

use super::ncm_api::{
    CookieBuilder, CookieJar, MusicApi,
    model::{SongInfo, SongUrl},
};

const COOKIE_FILE: &str = "cookies.json";
const MAX_CONS: usize = 32;

/// Default quality: 320kbps (index 2)
const DEFAULT_QUALITY: u32 = 2;

pub const BASE_URL_LIST: [&str; 12] = [
    "https://music.163.com/",
    "https://music.163.com/eapi/clientlog",
    "https://music.163.com/eapi/feedback",
    "https://music.163.com/api/clientlog",
    "https://music.163.com/api/feedback",
    "https://music.163.com/neapi/clientlog",
    "https://music.163.com/neapi/feedback",
    "https://music.163.com/weapi/clientlog",
    "https://music.163.com/weapi/feedback",
    "https://music.163.com/wapi/clientlog",
    "https://music.163.com/wapi/feedback",
    "https://music.163.com/openapi/clientlog",
];

/// NCM API client with built-in quality settings
///
/// The client internally manages the music quality setting, so callers
/// don't need to pass quality parameters. Use `set_quality()` to change
/// the quality setting.
#[derive(Clone)]
pub struct NcmClient {
    pub client: MusicApi,
    /// Music quality setting (0=128k, 1=192k, 2=320k, 3=SQ, 4=Hi-Res)
    /// Using Arc<AtomicU32> for thread-safe sharing across async tasks
    quality: Arc<AtomicU32>,
}

impl std::fmt::Debug for NcmClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NcmClient")
            .field("client", &"<MusicApi>")
            .finish()
    }
}

impl NcmClient {
    pub fn new() -> Self {
        Self {
            client: MusicApi::new(MAX_CONS),
            quality: Arc::new(AtomicU32::new(DEFAULT_QUALITY)),
        }
    }

    /// Create a new client with proxy settings
    pub fn with_proxy(proxy_url: Option<String>) -> Self {
        let mut client = Self::new();
        if let Some(url) = proxy_url {
            if let Err(e) = client.set_proxy(url) {
                tracing::warn!("Failed to set proxy: {}", e);
            }
        }
        client
    }

    pub fn from_cookie_jar(cookie_jar: CookieJar) -> Self {
        Self {
            client: MusicApi::from_cookie_jar(cookie_jar, MAX_CONS),
            quality: Arc::new(AtomicU32::new(DEFAULT_QUALITY)),
        }
    }

    /// Create a client from cookie jar with proxy settings
    pub fn from_cookie_jar_with_proxy(cookie_jar: CookieJar, proxy_url: Option<String>) -> Self {
        let mut client = Self::from_cookie_jar(cookie_jar);
        if let Some(url) = proxy_url {
            if let Err(e) = client.set_proxy(url) {
                tracing::warn!("Failed to set proxy: {}", e);
            }
        }
        client
    }

    pub fn set_proxy(&mut self, proxy: String) -> Result<()> {
        self.client.set_proxy(&proxy)
    }

    /// Set the music quality for downloads
    ///
    /// Quality values:
    /// - 0: 128kbps (Standard)
    /// - 1: 192kbps (Higher)
    /// - 2: 320kbps (High) - default
    /// - 3: SQ/FLAC (Lossless)
    /// - 4: Hi-Res
    pub fn set_quality(&self, quality: u32) {
        self.quality.store(quality, Ordering::Relaxed);
        tracing::info!(
            "Music quality set to: {} ({})",
            quality,
            Self::quality_to_bitrate(quality)
        );
    }

    /// Get current quality setting
    pub fn quality(&self) -> u32 {
        self.quality.load(Ordering::Relaxed)
    }

    /// Convert quality index to API bitrate value
    fn quality_to_bitrate(quality: u32) -> u32 {
        match quality {
            0 => 128000,
            1 => 192000,
            2 => 320000,
            3 => 999000,
            4 => 1900000,
            _ => 320000,
        }
    }

    /// Get API bitrate string for current quality setting
    fn current_bitrate(&self) -> String {
        Self::quality_to_bitrate(self.quality()).to_string()
    }

    fn data_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rustle")
    }

    fn cache_dir() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rustle")
    }

    pub fn cookie_file_path() -> PathBuf {
        let data_dir = Self::data_dir();
        fs::create_dir_all(&data_dir).ok();
        data_dir.join(COOKIE_FILE)
    }

    pub fn load_cookie_jar_from_file() -> Option<CookieJar> {
        match fs::File::open(Self::cookie_file_path()) {
            Err(err) => match err.kind() {
                io::ErrorKind::NotFound => (),
                other => error!("{:?}", other),
            },
            Ok(file) => match cookie_store::serde::json::load(io::BufReader::new(file)) {
                Err(err) => error!("{:?}", err),
                Ok(cookie_store) => {
                    let cookie_jar = CookieJar::default();
                    for base_url in BASE_URL_LIST {
                        let url = base_url.parse().unwrap();
                        for c in cookie_store.matches(&url) {
                            let cookie = CookieBuilder::new(c.name(), c.value())
                                .domain("music.163.com")
                                .path(c.path().unwrap_or("/"))
                                .build()
                                .unwrap();
                            cookie_jar.set(cookie, &base_url.parse().unwrap()).unwrap();
                        }
                    }
                    return Some(cookie_jar);
                }
            },
        };
        None
    }

    pub fn save_cookie_jar_to_file(&self) {
        if let Some(cookie_jar) = self.client.cookie_jar() {
            match fs::File::create(Self::cookie_file_path()) {
                Err(err) => error!("{:?}", err),
                Ok(mut file) => {
                    let mut cookie_store = CookieStore::default();
                    for base_url in BASE_URL_LIST {
                        let uri = &base_url.parse().unwrap();
                        let url = &base_url.parse().unwrap();
                        for c in cookie_jar.get_for_uri(url) {
                            let cookie = cookie_store::Cookie::parse(
                                format!(
                                    "{}={}; Path={}; Domain=music.163.com; Max-Age=31536000",
                                    c.name(),
                                    c.value(),
                                    url.path()
                                ),
                                uri,
                            )
                            .unwrap();
                            cookie_store.insert(cookie, uri).unwrap();
                        }
                    }
                    cookie_store::serde::json::save(&cookie_store, &mut file).unwrap();
                }
            }
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

    pub async fn create_qrcode(&self) -> Result<(PathBuf, String)> {
        let (qr_url, unikey) = self.client.login_qr_create().await?;
        let cache_dir = Self::cache_dir();
        fs::create_dir_all(&cache_dir)?;

        // Clean up old QR code files
        if let Ok(entries) = fs::read_dir(&cache_dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                if name.starts_with("qrimage_") && name.ends_with(".png") {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }

        // Use timestamp in filename to avoid iced image cache
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let path = cache_dir.join(format!("qrimage_{}.png", timestamp));
        qrcode_generator::to_png_to_file(qr_url, qrcode_generator::QrCodeEcc::Low, 200, &path)?;
        Ok((path, unikey))
    }

    /// Get song URLs using the client's quality setting
    ///
    /// The quality is automatically determined by the client's internal setting.
    /// Use `set_quality()` to change the quality before calling this method.
    pub async fn songs_url(&self, ids: &[u64]) -> Result<Vec<SongUrl>> {
        self.client.songs_url(ids, &self.current_bitrate()).await
    }

    pub async fn song_detail(&self, ids: &[u64]) -> Result<Vec<SongInfo>> {
        self.client.song_detail(ids).await
    }

    pub async fn get_lyrics(&self, si: &SongInfo) -> Result<Vec<(u64, String)>> {
        let cache_dir = Self::cache_dir();
        fs::create_dir_all(&cache_dir)?;

        let lyric_path = cache_dir.join(format!(
            "{}-{}-{}.lrc",
            si.name.replace('/', "／"),
            si.singer,
            si.album
        ));

        let tlyric_path = cache_dir.join(format!("{}.tlrc", si.id));
        let re = regex::Regex::new(r"\[\d+:\d+.\d+\]").unwrap();
        let re_abnormal_ts = regex::Regex::new(r"^\[(\d+):(\d+):(\d+)\]").unwrap();

        if !lyric_path.exists() {
            if let Ok(lyr) = self.client.song_lyric(si.id).await {
                debug!("歌词: {:?}", lyr);
                let mut lt = Vec::new();
                for l in lyr.lyric.iter() {
                    let mut time = 0;
                    if l.len() >= 10 && re.is_match(l) {
                        time = (l[1..3].parse::<u64>().unwrap_or(0) * 60
                            + l[4..6].parse::<u64>().unwrap_or(0))
                            * 1000
                            + l[7..9].parse::<u64>().unwrap_or(0) * 10;
                        let mut nl = re.replace_all(l, "").to_string();
                        nl.push('\n');
                        lt.push((time, nl));
                    }
                    for t in lyr.tlyric.iter() {
                        if t.len() >= 10 && l.len() >= 10 && t.starts_with(&l[0..10]) {
                            let mut nt = re.replace_all(t, "").to_string();
                            nt.push('\n');
                            lt.push((time, nt));
                        }
                    }
                }
                let lyric = lyr
                    .lyric
                    .into_iter()
                    .map(|x| re_abnormal_ts.replace_all(&x, "[$1:$2.$3]").to_string())
                    .collect::<Vec<String>>()
                    .join("\n");
                fs::write(&lyric_path, lyric)?;
                if !lyr.tlyric.is_empty() {
                    let tlyric = lyr
                        .tlyric
                        .into_iter()
                        .map(|x| re_abnormal_ts.replace_all(&x, "[$1:$2.$3]").to_string())
                        .collect::<Vec<String>>()
                        .join("\n");
                    fs::write(&tlyric_path, tlyric)?;
                }
                Ok(lt)
            } else {
                anyhow::bail!("No lyrics found!")
            }
        } else {
            let lyric = fs::read_to_string(&lyric_path)?;
            let lyrics: Vec<String> = lyric.split('\n').map(|s| s.to_string()).collect();
            let mut tlyrics = vec![];
            if tlyric_path.exists() {
                let tlyric = fs::read_to_string(&tlyric_path)?;
                tlyrics = tlyric.split('\n').map(|s| s.to_string()).collect();
            }
            let mut lt = Vec::new();
            for l in lyrics.iter() {
                let mut time = 0;
                if l.len() >= 10 && re.is_match(l) {
                    time = (l[1..3].parse::<u64>().unwrap_or(0) * 60
                        + l[4..6].parse::<u64>().unwrap_or(0))
                        * 1000
                        + l[7..9].parse::<u64>().unwrap_or(0) * 10;
                    let mut nl = re.replace_all(l, "").to_string();
                    nl.push('\n');
                    lt.push((time, nl));
                }
                for t in tlyrics.iter() {
                    if t.len() >= 10 && l.len() >= 10 && t.starts_with(&l[0..10]) {
                        let mut nt = re.replace_all(t, "").to_string();
                        nt.push('\n');
                        lt.push((time, nt));
                    }
                }
            }
            Ok(lt)
        }
    }
}

impl Default for NcmClient {
    fn default() -> Self {
        Self::new()
    }
}
