use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Lyrics {
    pub lyric: Vec<String>,
    pub tlyric: Vec<String>,
    /// NetEase word-level lyrics (YRC) returned by `/api/song/lyric/v1`.
    pub yrc: Vec<String>,
    /// Word-level translated lyrics, when available.
    pub ytlrc: Vec<String>,
    /// Romanized line lyrics.
    pub romalrc: Vec<String>,
    /// Word-level romanized lyrics.
    pub yromalrc: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct ArtistSummary {
    pub id: u64,
    pub name: String,
    pub image_url: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct UserSummary {
    pub id: u64,
    pub nickname: String,
    pub avatar_url: String,
    #[serde(default)]
    pub vip: VipInfo,
}

/// Normalized NCM membership rights shared by account, profile and UI layers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct VipInfo {
    pub vip_type: i32,
    pub red_vip_level: u32,
    pub annual_count: u32,
    pub icon_url: String,
}

impl VipInfo {
    pub fn is_vip(&self) -> bool {
        self.vip_type > 0 || self.red_vip_level > 0
    }

    pub fn is_annual(&self) -> bool {
        self.annual_count > 0
    }

    pub fn display_label(&self) -> String {
        if !self.is_vip() {
            return "普通用户".to_string();
        }
        if self.red_vip_level > 0 {
            let annual = if self.is_annual() { " 年费" } else { "" };
            return format!("黑胶 VIP Lv.{}{}", self.red_vip_level, annual);
        }
        "黑胶 VIP".to_string()
    }
}

/// NCM's complete quality taxonomy. This is the only place that translates
/// API level strings into product-facing metadata.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Deserialize, Serialize, PartialOrd, Ord,
)]
#[serde(rename_all = "lowercase")]
pub enum NcmQualityLevel {
    #[default]
    Standard,
    Higher,
    ExHigh,
    Lossless,
    HiRes,
    JvEffect,
    Sky,
    Dolby,
    JyMaster,
}

impl NcmQualityLevel {
    #[allow(dead_code)]
    pub const ALL: [Self; 9] = [
        Self::Standard,
        Self::Higher,
        Self::ExHigh,
        Self::Lossless,
        Self::HiRes,
        Self::JvEffect,
        Self::Sky,
        Self::Dolby,
        Self::JyMaster,
    ];

    pub fn api_level(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Higher => "higher",
            Self::ExHigh => "exhigh",
            Self::Lossless => "lossless",
            Self::HiRes => "hires",
            Self::JvEffect => "jyeffect",
            Self::Sky => "sky",
            Self::Dolby => "dolby",
            Self::JyMaster => "jymaster",
        }
    }

    pub fn from_api_level(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "standard" => Some(Self::Standard),
            "higher" => Some(Self::Higher),
            "exhigh" | "high" => Some(Self::ExHigh),
            "lossless" => Some(Self::Lossless),
            "hires" | "hi-res" => Some(Self::HiRes),
            "jyeffect" | "jvEffect" => Some(Self::JvEffect),
            "sky" => Some(Self::Sky),
            "dolby" => Some(Self::Dolby),
            "jymaster" | "master" => Some(Self::JyMaster),
            _ => None,
        }
    }

    pub fn from_legacy_rate(value: u32) -> Self {
        match value {
            0 => Self::Standard,
            1 => Self::Higher,
            2 => Self::ExHigh,
            3 => Self::Lossless,
            4 => Self::HiRes,
            5 => Self::JvEffect,
            6 => Self::Sky,
            7 => Self::Dolby,
            8 => Self::JyMaster,
            _ => Self::ExHigh,
        }
    }

    #[allow(dead_code)]
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Standard => "标准音质",
            Self::Higher => "较高音质",
            Self::ExHigh => "极高音质",
            Self::Lossless => "无损音质",
            Self::HiRes => "Hi-Res",
            Self::JvEffect => "高清臻音",
            Self::Sky => "沉浸环绕声",
            Self::Dolby => "杜比全景声",
            Self::JyMaster => "超清母带",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            Self::Standard => "128K",
            Self::Higher => "192K",
            Self::ExHigh => "320K",
            Self::Lossless => "SQ",
            Self::HiRes => "Hi-Res",
            Self::JvEffect => "臻音",
            Self::Sky => "环绕声",
            Self::Dolby => "Dolby",
            Self::JyMaster => "母带",
        }
    }

    /// Higher values represent a more premium server quality tier.
    pub fn priority(self) -> u8 {
        match self {
            Self::Standard => 0,
            Self::Higher => 1,
            Self::ExHigh => 2,
            Self::Lossless => 3,
            Self::HiRes => 4,
            Self::JvEffect => 5,
            Self::Sky => 6,
            Self::Dolby => 7,
            Self::JyMaster => 8,
        }
    }

    #[allow(dead_code)]
    pub fn is_spatial_or_enhanced(self) -> bool {
        matches!(
            self,
            Self::JvEffect | Self::Sky | Self::Dolby | Self::JyMaster
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct SongQualityOption {
    pub level: NcmQualityLevel,
    pub bitrate: Option<u32>,
    pub size: Option<u64>,
    pub format: Option<String>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
    pub channels: Option<u32>,
}

impl SongQualityOption {
    #[allow(dead_code)]
    pub fn description(&self) -> String {
        let mut parts = Vec::new();
        if let Some(bitrate) = self.bitrate.filter(|value| *value > 0) {
            parts.push(format!("{} kbps", bitrate / 1000));
        }
        if let Some(size) = self.size.filter(|value| *value > 0) {
            parts.push(format!("{:.1} MB", size as f64 / 1024.0 / 1024.0));
        }
        parts.join(" · ")
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct SongQualityDetail {
    pub song_id: u64,
    pub options: Vec<SongQualityOption>,
    pub highest_available: Option<NcmQualityLevel>,
}

impl SongQualityDetail {
    pub fn best_for(&self, requested: NcmQualityLevel) -> Option<&SongQualityOption> {
        self.options
            .iter()
            .find(|option| option.level == requested)
            .or_else(|| self.options.iter().max_by_key(|option| option.level.priority()))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct AlbumSummary {
    pub id: u64,
    pub name: String,
    pub image_url: String,
    pub artists: Vec<ArtistSummary>,
    pub publish_time: u64,
    pub tags: String,
}

impl AlbumSummary {
    pub fn primary_artist(&self) -> Option<&ArtistSummary> {
        self.artists.first()
    }

    pub fn artist_names(&self) -> String {
        joined_artist_names(&self.artists)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct PlaylistSummary {
    pub id: u64,
    pub name: String,
    pub cover_url: String,
    pub creator: UserSummary,
    pub subscribed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct VideoSummary {
    pub id: u64,
    pub name: String,
    pub cover_url: String,
    pub artist_name: String,
    pub duration_ms: u64,
    pub play_count: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct RadioSummary {
    pub id: u64,
    pub name: String,
    pub cover_url: String,
    pub creator: UserSummary,
    pub category: String,
    pub program_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum TrackAvailability {
    Free,
    VipOnly,
    Payment,
    VipOnlyHighRate,
    Unavailable,
    Unknown,
}

impl TrackAvailability {
    pub fn from_fee(fee: i32) -> Self {
        match fee {
            0 => Self::Free,
            1 => Self::VipOnly,
            4 => Self::Payment,
            8 => Self::VipOnlyHighRate,
            _ => Self::Unknown,
        }
    }

    pub fn from_privilege(st: i32, fee: i32) -> Self {
        if st < 0 {
            Self::Unavailable
        } else {
            Self::from_fee(fee)
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Free => "免费",
            Self::VipOnly => "VIP",
            Self::Payment => "EP/购买",
            Self::VipOnlyHighRate => "高音质 VIP",
            Self::Unavailable => "不可用",
            Self::Unknown => "",
        }
    }

    pub fn is_restricted(&self) -> bool {
        matches!(
            self,
            Self::VipOnly | Self::Payment | Self::VipOnlyHighRate | Self::Unavailable
        )
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Track {
    pub id: u64,
    pub title: String,
    pub artists: Vec<ArtistSummary>,
    pub album: AlbumSummary,
    pub duration_ms: u64,
    pub availability: TrackAvailability,
    pub track_number: Option<u32>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    #[serde(default)]
    pub quality_options: Vec<SongQualityOption>,
}

impl Track {
    pub fn primary_artist(&self) -> Option<&ArtistSummary> {
        self.artists.first()
    }

    pub fn artist_names(&self) -> String {
        joined_artist_names(&self.artists)
    }

    pub fn cover_url(&self) -> &str {
        self.album.image_url.as_str()
    }
}

impl PartialEq for Track {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Default for Track {
    fn default() -> Self {
        Self {
            id: 0,
            title: String::new(),
            artists: Vec::new(),
            album: AlbumSummary::default(),
            duration_ms: 0,
            availability: TrackAvailability::Unknown,
            track_number: None,
            year: None,
            genre: None,
            quality_options: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlaylistDetail {
    pub id: u64,
    pub name: String,
    pub cover_url: String,
    pub description: String,
    pub create_time: u64,
    pub track_update_time: u64,
    pub creator: UserSummary,
    pub track_count: u64,
    pub subscribed: bool,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArtistDetail {
    pub id: u64,
    pub name: String,
    pub image_url: String,
    pub description: String,
    pub track_count: u32,
    pub album_count: u32,
    pub mv_count: u32,
    pub followed: bool,
    pub top_tracks: Vec<Track>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AlbumDetail {
    pub id: u64,
    pub name: String,
    pub image_url: String,
    pub description: String,
    pub artists: Vec<ArtistSummary>,
    pub track_count: u32,
    pub publish_time: u64,
    pub tracks: Vec<Track>,
    pub tags: String,
}

impl AlbumDetail {
    pub fn primary_artist(&self) -> Option<&ArtistSummary> {
        self.artists.first()
    }

    pub fn artist_names(&self) -> String {
        joined_artist_names(&self.artists)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserDetail {
    pub user_id: u64,
    pub artist_id: u64,
    pub nickname: String,
    pub artist_name: String,
    pub signature: String,
    pub follows: u64,
    pub followeds: u64,
    pub avatar_url: String,
    pub background_url: String,
    #[serde(default)]
    pub vip: VipInfo,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TrackUrl {
    pub id: u64,
    pub url: String,
    pub requested_level: NcmQualityLevel,
    pub level: NcmQualityLevel,
    pub rate: u32,
    pub size: Option<u64>,
    pub format: Option<String>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
    pub channels: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Msg {
    pub code: i32,
    pub msg: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct LoginInfo {
    pub code: i32,
    pub user_id: u64,
    pub nickname: String,
    pub avatar_url: String,
    pub vip_type: i32,
    #[serde(default)]
    pub vip: VipInfo,
    pub msg: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Banner {
    pub image_url: String,
    pub target_id: u64,
    pub target: BannerTarget,
    pub title: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum BannerTarget {
    Song,
    Album,
    Unknown,
}

impl From<i32> for BannerTarget {
    fn from(t: i32) -> Self {
        match t {
            1 => Self::Song,
            10 => Self::Album,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchType {
    Songs,
    Albums,
    Artists,
    Playlists,
    Videos,
    Radios,
}

impl SearchType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SearchType::Songs => "1",
            SearchType::Albums => "10",
            SearchType::Artists => "100",
            SearchType::Playlists => "1000",
            SearchType::Videos => "1004",
            SearchType::Radios => "1009",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SearchResponse {
    pub tracks: Vec<Track>,
    pub albums: Vec<AlbumSummary>,
    pub artists: Vec<ArtistSummary>,
    pub playlists: Vec<PlaylistSummary>,
    pub videos: Vec<VideoSummary>,
    pub radios: Vec<RadioSummary>,
    pub track_count: u32,
    pub album_count: u32,
    pub artist_count: u32,
    pub playlist_count: u32,
    pub video_count: u32,
    pub radio_count: u32,
}

fn joined_artist_names(artists: &[ArtistSummary]) -> String {
    let names = artists
        .iter()
        .filter_map(|artist| {
            let name = artist.name.trim();
            (!name.is_empty()).then_some(name)
        })
        .collect::<Vec<_>>();

    if names.is_empty() {
        "unknown".to_string()
    } else {
        names.join(" / ")
    }
}
