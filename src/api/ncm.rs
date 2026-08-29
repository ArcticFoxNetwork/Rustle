//! NCM service boundary.
//!
//! Rustle keeps stable domain models here while delegating all NetEase request
//! implementation to `ncm-api-rs`.

mod client;
mod mapper;
mod models;

pub use client::NcmClient;
pub use models::{
    AlbumDetail, AlbumSummary, ArtistDetail, ArtistSummary, Banner, BannerTarget, LoginInfo,
    NcmQualityLevel, PlaylistDetail, PlaylistSummary, RadioSummary, SearchType, SongQualityDetail,
    SongQualityOption, Track, TrackAvailability, TrackUrl, UserDetail, UserSummary, VideoSummary,
    VipInfo, VipTier,
};
