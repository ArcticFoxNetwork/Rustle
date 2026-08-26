//! Netease Cloud Music API module
//!
//! Provides NCM client with cookie management, QR login, and API wrappers.

mod ncm;

#[allow(unused_imports)]
pub use ncm::{
    AlbumDetail, AlbumSummary, ArtistDetail, ArtistSummary, Banner, BannerTarget, LoginInfo,
    NcmClient, NcmQualityLevel, PlaylistDetail, PlaylistSummary, RadioSummary, SearchType,
    SongQualityDetail, SongQualityOption, Track, TrackAvailability, TrackUrl, UserDetail,
    UserSummary, VideoSummary, VipInfo,
};
