//! Netease Cloud Music API module
//!
//! Provides NCM client with cookie management, QR login, and API wrappers.

mod ncm;

pub use ncm::{
    AlbumDetail, AlbumSummary, ArtistDetail, ArtistSummary, Banner, BannerTarget, LoginInfo,
    NcmClient, PlaylistDetail, PlaylistSummary, SearchType, Track, UserDetail, UserSummary,
};
#[allow(unused_imports)]
pub use ncm::TrackAvailability;
