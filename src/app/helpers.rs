//! Async helper functions for database operations

use std::path::PathBuf;
use std::sync::Arc;

use crate::database::{Database, DbPlaybackState, DbPlaylist, DbSong, NewPlaylist};
use crate::features::import::{CoverCache, default_cache_dir};
#[cfg(target_os = "linux")]
use crate::features::{MprisCommand, MprisHandle, mpris};
#[cfg(target_os = "linux")]
use crate::features::{TrayCommand, TrayHandle, TrayState, tray};
use crate::features::PlayMode;
use crate::ui::pages;
use crate::utils::format_relative_time;

/// Initialize database connection
pub async fn init_database() -> anyhow::Result<Database> {
    let data_dir = directories::ProjectDirs::from("com", "rustle", "Rustle")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    std::fs::create_dir_all(&data_dir)?;
    let db_path = data_dir.join("rustle.db");

    tracing::info!("Initializing database at: {}", db_path.display());
    Database::new(&db_path).await
}

/// Load all songs from database
pub async fn load_songs(db: Arc<Database>) -> Vec<DbSong> {
    db.get_all_songs().await.unwrap_or_default()
}

/// Load all playlists from database
pub async fn load_playlists(db: Arc<Database>) -> Vec<DbPlaylist> {
    db.get_all_playlists().await.unwrap_or_default()
}

/// Load playback state from database
pub async fn load_playback_state(db: Arc<Database>) -> Option<DbPlaybackState> {
    db.get_playback_state().await.ok()
}

/// Load queue from database
pub async fn load_queue(db: Arc<Database>) -> Vec<DbSong> {
    db.get_queue().await.unwrap_or_default()
}

/// Validate all songs in database and remove entries for missing files
/// Returns the number of invalid songs removed
/// Note: NCM songs (file_path starts with "ncm://") are skipped as they are cloud songs
pub async fn validate_songs(db: Arc<Database>) -> u32 {
    let songs = match db.get_all_songs().await {
        Ok(songs) => songs,
        Err(e) => {
            tracing::error!("Failed to load songs for validation: {}", e);
            return 0;
        }
    };

    let mut removed_count = 0u32;

    for song in songs {
        // Skip NCM cloud songs - they don't have local files
        if song.file_path.starts_with("ncm://") {
            continue;
        }

        let path = std::path::Path::new(&song.file_path);
        if !path.exists() {
            tracing::info!("Removing invalid song (file not found): {}", song.file_path);
            if let Err(e) = db.delete_song(song.id).await {
                tracing::error!("Failed to delete invalid song {}: {}", song.id, e);
            } else {
                removed_count += 1;
            }
        }
    }

    if removed_count > 0 {
        tracing::info!("Removed {} invalid songs from database", removed_count);
    }

    removed_count
}

/// Initialize cover cache
pub async fn init_cover_cache() -> anyhow::Result<CoverCache> {
    let cache_dir = default_cache_dir();
    CoverCache::new(cache_dir)
}

/// Initialize font system for lyrics text shaping
/// We do it in a background thread
pub async fn init_font_system() -> crate::features::lyrics::engine::SharedFontSystem {
    tokio::task::spawn_blocking(|| {
        tracing::info!("Initializing FontSystem for lyrics...");
        let start = std::time::Instant::now();
        let font_system = cosmic_text::FontSystem::new();
        tracing::info!("FontSystem initialized in {:?}", start.elapsed());

        // Warm up font cache with common character sets
        // This pre-loads font fallback information for CJK and Latin characters
        let font_system = std::sync::Arc::new(parking_lot::Mutex::new(font_system));
        warm_up_font_cache(&font_system);

        font_system
    })
    .await
    .expect("FontSystem initialization should not panic")
}

/// Warm up font cache with common character sets
/// This pre-loads font fallback information to avoid lag when switching between languages
fn warm_up_font_cache(font_system: &crate::features::lyrics::engine::SharedFontSystem) {
    use cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping};

    let start = std::time::Instant::now();

    // Sample text covering common character sets
    let warmup_texts = [
        // Latin (English)
        "The quick brown fox jumps over the lazy dog",
        // CJK (Chinese)
        "你好世界，这是一段中文歌词测试",
        // CJK (Japanese)
        "こんにちは世界、日本語のテスト",
        // CJK (Korean)
        "안녕하세요 세계, 한국어 테스트",
        // Numbers and punctuation
        "0123456789 !@#$%^&*()[]{}",
    ];

    let mut fs = font_system.lock();
    let metrics = Metrics::new(48.0, 48.0 * 1.4);
    let mut buffer = Buffer::new(&mut fs, metrics);
    buffer.set_size(&mut fs, Some(800.0), None);

    let attrs = Attrs::new().family(Family::SansSerif);

    for text in warmup_texts {
        buffer.set_text(&mut fs, text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut fs, false);
    }

    tracing::info!("Font cache warmed up in {:?}", start.elapsed());
}

/// Initialize system tray (Linux only)
/// Returns the command receiver wrapped in Arc<Mutex>; the handle is stored globally for updates
#[cfg(target_os = "linux")]
pub async fn init_tray() -> anyhow::Result<
    std::sync::Arc<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<TrayCommand>>>,
> {
    let (handle, rx) = tray::start_tray()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start tray: {}", e))?;

    // Store handle globally for later updates
    TRAY_HANDLE.set(handle).ok();

    tracing::info!("System tray started");
    Ok(std::sync::Arc::new(tokio::sync::Mutex::new(rx)))
}

/// Global tray handle for updates (Linux only)
#[cfg(target_os = "linux")]
static TRAY_HANDLE: once_cell::sync::OnceCell<TrayHandle> = once_cell::sync::OnceCell::new();

/// Get the global tray handle (Linux only)
#[cfg(target_os = "linux")]
pub fn get_tray_handle() -> Option<&'static TrayHandle> {
    TRAY_HANDLE.get()
}

/// Initialize MPRIS (Linux only)
/// Returns the command receiver wrapped in Arc<Mutex>
#[cfg(target_os = "linux")]
pub async fn init_mpris() -> anyhow::Result<(
    MprisHandle,
    std::sync::Arc<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<MprisCommand>>>,
)> {
    let (handle, rx) = mpris::start_mpris();

    tracing::info!("MPRIS service started");
    Ok((handle, std::sync::Arc::new(tokio::sync::Mutex::new(rx))))
}

/// Global MPRIS handle for updates (Linux only)
#[cfg(target_os = "linux")]
static MPRIS_HANDLE: once_cell::sync::OnceCell<MprisHandle> = once_cell::sync::OnceCell::new();

/// Set the global MPRIS handle (Linux only)
#[cfg(target_os = "linux")]
pub fn set_mpris_handle(handle: MprisHandle) {
    MPRIS_HANDLE.set(handle).ok();
}

/// Update tray state with full info including play mode (Linux only)
#[cfg(target_os = "linux")]
pub fn update_tray_state_full(
    is_playing: bool,
    title: Option<String>,
    artist: Option<String>,
    play_mode: PlayMode,
) {
    update_tray_state_with_favorite(is_playing, title, artist, play_mode, None, false);
}

/// Update tray state with full info including play mode (no-op on non-Linux)
#[cfg(not(target_os = "linux"))]
pub fn update_tray_state_full(
    _is_playing: bool,
    _title: Option<String>,
    _artist: Option<String>,
    _play_mode: PlayMode,
) {
}

/// Update tray state with full info including favorite status (Linux only)
#[cfg(target_os = "linux")]
pub fn update_tray_state_with_favorite(
    is_playing: bool,
    title: Option<String>,
    artist: Option<String>,
    play_mode: PlayMode,
    ncm_song_id: Option<u64>,
    is_favorited: bool,
) {
    if let Some(handle) = get_tray_handle() {
        let state = TrayState {
            is_playing,
            title,
            artist,
            play_mode,
            ncm_song_id,
            is_favorited,
        };
        let handle = handle.clone();
        tokio::spawn(async move {
            handle.update(state).await;
        });
    }
}

/// Update tray state with full info including favorite status (no-op on non-Linux)
#[cfg(not(target_os = "linux"))]
pub fn update_tray_state_with_favorite(
    _is_playing: bool,
    _title: Option<String>,
    _artist: Option<String>,
    _play_mode: PlayMode,
    _ncm_song_id: Option<u64>,
    _is_favorited: bool,
) {
}

/// Open folder dialog
pub async fn open_folder_dialog() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("选择音乐文件夹")
        .pick_folder()
        .await
        .map(|handle| handle.path().to_path_buf())
}

/// Create playlist from import results
pub async fn create_playlist_from_import(
    db: Arc<Database>,
    name: String,
    cover_path: Option<String>,
    scanned_paths: Vec<std::path::PathBuf>,
) -> anyhow::Result<i64> {
    let playlist = NewPlaylist {
        name,
        description: None,
        cover_path,
        is_smart: false,
    };

    let playlist_id = db.create_playlist(playlist).await?;

    // Add songs from scanned paths to playlist
    for path in scanned_paths {
        let path_str = path.to_string_lossy().to_string();
        // Find song by path in database
        if let Ok(Some(song)) = db.get_song_by_path(&path_str).await {
            if let Err(e) = db.add_song_to_playlist(playlist_id, song.id).await {
                tracing::warn!("Failed to add song {} to playlist: {}", song.id, e);
            }
        } else {
            tracing::warn!("Song not found in database: {}", path_str);
        }
    }

    Ok(playlist_id)
}

/// Load playlist view data from database
pub async fn load_playlist_view(
    db: Arc<Database>,
    playlist_id: i64,
) -> Option<pages::PlaylistView> {
    // Get playlist info
    let playlist = db.get_playlist(playlist_id).await.ok()??;

    // Get songs in playlist with added_at date
    let songs = db
        .get_playlist_songs_with_date(playlist_id)
        .await
        .unwrap_or_default();

    // Convert to view models
    let song_views: Vec<pages::PlaylistSongView> = songs
        .iter()
        .enumerate()
        .map(|(i, song)| {
            let duration_secs = song.duration_secs as u64;
            let mins = duration_secs / 60;
            let secs = duration_secs % 60;

            // Format added_at as relative time
            let added_date = format_relative_time(song.added_at);

            pages::PlaylistSongView::new(
                song.id,
                i + 1,
                song.title.clone(),
                if song.artist.is_empty() {
                    "未知艺术家".to_string()
                } else {
                    song.artist.clone()
                },
                if song.album.is_empty() {
                    "未知专辑".to_string()
                } else {
                    song.album.clone()
                },
                format!("{}:{:02}", mins, secs),
                added_date,
                song.cover_path.clone(),
                false,
            )
        })
        .collect();

    // Calculate total duration
    let total_secs: u64 = songs.iter().map(|s| s.duration_secs as u64).sum();
    let total_mins = total_secs / 60;
    let total_hours = total_mins / 60;
    let remaining_mins = total_mins % 60;
    let total_duration = if total_hours > 0 {
        format!("约 {} 小时 {} 分钟", total_hours, remaining_mins)
    } else {
        format!("{} 分钟", total_mins)
    };

    // Extract color palette from cover image
    let palette = playlist
        .cover_path
        .as_ref()
        .map(|p| crate::utils::ColorPalette::from_image_path(std::path::Path::new(p)))
        .unwrap_or_default();

    Some(pages::PlaylistView {
        id: playlist.id,
        name: playlist.name,
        description: playlist.description,
        cover_path: playlist.cover_path,
        owner: "本地".to_string(),
        owner_avatar_path: None,
        creator_id: 0,
        song_count: songs.len() as u32,
        total_duration,
        like_count: String::new(),
        songs: song_views,
        palette,
        is_local: true,
        is_subscribed: false,
    })
}
