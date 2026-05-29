//! Unified image type taxonomy — pure data, zero UI dependencies.
//!
//! Every image category the app manages flows through a single resolution
//! pipeline defined here.  UI widgets (cover_image, avatar_image) and the
//! update handler (app::update::images) both depend on this module.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Image category
// ---------------------------------------------------------------------------

/// Every distinct image domain the app manages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageKind {
    /// NCM song / track cover art (identified by song's NCM id)
    SongCover,
    /// Local-library song cover art (identified by local database song id)
    LocalSongCover,
    /// NCM playlist cover (identified by playlist id)
    PlaylistCover,
    /// Local-library playlist cover (identified by local database playlist id)
    LocalPlaylistCover,
    /// Artist portrait / avatar (identified by artist id)
    ArtistCover,
    /// Album cover (identified by album id)
    AlbumCover,
    /// User avatar (identified by user id)
    UserAvatar,
    /// Homepage carousel banner (identified by banner index / target id)
    Banner,
}

impl ImageKind {
    /// Which cache directory stores images of this kind.
    pub fn cache_dir(&self) -> PathBuf {
        match self {
            Self::SongCover
            | Self::LocalSongCover
            | Self::PlaylistCover
            | Self::LocalPlaylistCover
            | Self::ArtistCover
            | Self::AlbumCover => crate::utils::covers_cache_dir(),
            Self::UserAvatar => crate::utils::avatars_cache_dir(),
            Self::Banner => crate::utils::banners_cache_dir(),
        }
    }

    /// File stem used in the cache directory (without extension).
    pub fn file_stem(&self, id: u64) -> String {
        match self {
            Self::SongCover => format!("cover_{}", id),
            Self::LocalSongCover => format!("local_song_{}", id),
            Self::PlaylistCover => format!("playlist_{}", id),
            Self::LocalPlaylistCover => format!("local_playlist_{}", id),
            Self::ArtistCover => format!("artist_{}", id),
            Self::AlbumCover => format!("album_{}", id),
            Self::UserAvatar => format!("avatar_{}", id),
            Self::Banner => format!("banner_{}", id),
        }
    }
}

pub fn song_cover_key(song_id: i64) -> Option<(ImageKind, u64)> {
    if song_id < 0 {
        song_id
            .checked_neg()
            .and_then(|id| u64::try_from(id).ok())
            .map(|id| (ImageKind::SongCover, id))
    } else {
        u64::try_from(song_id)
            .ok()
            .map(|id| (ImageKind::LocalSongCover, id))
    }
}

/// The result of a successful async download.
#[derive(Debug, Clone)]
pub struct ImageResult {
    pub kind: ImageKind,
    pub id: u64,
    pub path: PathBuf,
}

// ---------------------------------------------------------------------------
// Cover size enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub enum CoverSize {
    Tiny,
    Picker,
    Medium,
    Large,
    ExtraLarge,
}

impl CoverSize {
    pub fn px(&self) -> f32 {
        match self {
            CoverSize::Tiny => 40.0,
            CoverSize::Picker => 40.0,
            CoverSize::Medium => 56.0,
            CoverSize::Large => 200.0,
            CoverSize::ExtraLarge => 400.0,
        }
    }

    pub fn radius(&self) -> f32 {
        match self {
            CoverSize::Tiny => 4.0,
            CoverSize::Picker => 8.0,
            CoverSize::Medium => 8.0,
            CoverSize::Large => 12.0,
            CoverSize::ExtraLarge => 16.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Pure cache-resolver (no side effects)
// ---------------------------------------------------------------------------

/// Synchronous cache probe — returns `Some(path)` if a file for this
/// `(kind, id)` already exists on disk, `None` otherwise.
///
/// Pure function: no queue push, no state mutation, no download trigger.
pub fn resolve_cached(kind: ImageKind, id: u64) -> Option<PathBuf> {
    crate::utils::find_cached_image(&kind.cache_dir(), &kind.file_stem(id))
}

/// True when a string points at a remote HTTP(S) image source.
pub fn is_remote_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// True when a string field (`cover_path` / `cover_img_url`) holds a valid
/// local path rather than an http(s) URL.
pub fn is_valid_local_path(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    if is_remote_url(s) {
        return false;
    }
    std::path::Path::new(s).exists()
}
