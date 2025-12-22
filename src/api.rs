//! Netease Cloud Music API module
//!
//! Provides NCM client with cookie management, QR login, and API wrappers.

mod ncm;
pub mod ncm_api;

pub use ncm::NcmClient;
pub use ncm_api::model::{
    BannersInfo, LoginInfo, PlayListDetail, SongCopyright, SongInfo, SongList, TargetType, TopList,
};
