use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Lyrics {
    pub lyric: Vec<String>,
    pub tlyric: Vec<String>,
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
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TrackUrl {
    pub id: u64,
    pub url: String,
    pub rate: u32,
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
}

impl SearchType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SearchType::Songs => "1",
            SearchType::Albums => "10",
            SearchType::Artists => "100",
            SearchType::Playlists => "1000",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SearchResponse {
    pub tracks: Vec<Track>,
    pub albums: Vec<AlbumSummary>,
    pub artists: Vec<ArtistSummary>,
    pub playlists: Vec<PlaylistSummary>,
    pub track_count: u32,
    pub album_count: u32,
    pub artist_count: u32,
    pub playlist_count: u32,
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
