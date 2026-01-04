//! Application messages

use std::path::PathBuf;
use std::sync::Arc;

use iced::keyboard::{Key, Modifiers};

use crate::api::{BannersInfo, LoginInfo, NcmClient, PlayListDetail, SongInfo, SongList, TopList};
use crate::app::state::UserInfo;
use crate::database::{Database, DbPlaybackState, DbPlaylist, DbSong};
use crate::features::Action;
use crate::features::import::{CoverCache, ScanProgress, WatchEvent};
use crate::ui::components::{LibraryItem, NavItem};
use crate::ui::pages;

/// Settings sections for navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Account,
    Playback,
    Display,
    System,
    Network,
    Storage,
    Shortcuts,
    About,
}

/// Application messages
#[derive(Clone)]
pub enum Message {
    /// No-op message for event interception (modal backdrop clicks)
    Noop,

    // ============ Navigation ============
    /// Navigation menu item selected
    Navigate(NavItem),
    /// Navigate back in history
    NavigateBack,
    /// Navigate forward in history
    NavigateForward,
    /// Library item selected
    LibrarySelect(LibraryItem),
    /// Search query changed
    SearchChanged(String),
    /// Play hero banner playlist
    PlayHero,
    /// Import local playlist
    ImportLocalPlaylist,
    /// Folder selected from dialog
    FolderSelected(Option<PathBuf>),

    // ============ Window ============
    /// Minimize window
    WindowMinimize,
    /// Maximize window
    WindowMaximize,
    /// Mouse pressed (for window drag detection)
    MousePressed,
    /// Mouse released (for sidebar resize end)
    MouseReleased,
    /// Mouse moved (track cursor position for drag area)
    MouseMoved(iced::Point),
    /// Open settings
    OpenSettings,
    /// Open settings and close lyrics page
    OpenSettingsWithCloseLyrics,
    /// Open audio engine page
    OpenAudioEngine,

    // ============ Settings ============
    /// Update close behavior
    UpdateCloseBehavior(crate::features::CloseBehavior),
    /// Save settings
    SaveSettings,
    /// Update playback settings
    UpdateFadeInOut(bool),
    UpdateVolumeNormalization(bool),
    UpdateMusicQuality(crate::features::MusicQuality),
    UpdateEqualizerEnabled(bool),
    UpdateEqualizerPreset(crate::features::EqualizerPreset),
    UpdateEqualizerValues([f32; 10]),
    UpdateEqualizerPreamp(f32),
    /// Update spectrum analyzer settings
    UpdateSpectrumDecay(f32),
    UpdateSpectrumBarsMode(bool),
    /// Update display settings
    UpdateDarkMode(bool),
    UpdateAppLanguage(String),
    /// Update power saving mode
    UpdatePowerSavingMode(bool),
    /// Update storage settings
    UpdateMaxCacheMb(u64),
    ClearCache,
    /// Cache cleared result (files_deleted, bytes_freed)
    CacheCleared(usize, u64),
    /// Refresh cache statistics
    RefreshCacheStats,
    /// Enforce cache size limit
    EnforceCacheLimit,
    /// Update system settings
    UpdateAudioOutputDevice(Option<String>),
    UpdateAudioBufferSize(u32),
    /// Update network settings
    UpdateProxyType(crate::features::ProxyType),
    UpdateProxyHost(String),
    UpdateProxyPort(String),
    UpdateProxyUsername(String),
    UpdateProxyPassword(String),
    /// Apply proxy settings to the NCM client
    ApplyProxySettings,
    /// Settings navigation
    ScrollToSection(SettingsSection),
    /// Settings page scrolled (y offset in pixels)
    SettingsScrolled(f32),
    /// Start editing a keybinding for an action
    StartEditingKeybinding(Action),
    /// Cancel keybinding edit
    CancelEditingKeybinding,
    /// Key pressed while editing keybinding
    KeybindingKeyPressed(Key, Modifiers),

    // ============ Database ============
    /// Database initialized
    DatabaseReady(Arc<Database>),
    /// Database error
    DatabaseError(String),
    /// Songs loaded from database
    SongsLoaded(Vec<DbSong>),
    /// Playlists loaded from database
    PlaylistsLoaded(Vec<DbPlaylist>),
    /// Playback state loaded
    PlaybackStateLoaded(DbPlaybackState),
    /// Queue restored from database on startup (does not auto-play)
    QueueRestored(Vec<DbSong>),
    /// NCM song resolved during app startup restore
    /// (queue_index, resolved_result, saved_position_secs)
    SongResolvedForRestore(
        usize,
        Option<crate::app::update::song_resolver::ResolvedSong>,
        f64,
    ),
    /// Songs validated - invalid entries removed
    SongsValidated(u32),
    /// Queue loaded from playlist (starts playing)
    QueueLoaded(Vec<DbSong>),
    /// Recently played loaded
    RecentlyPlayedLoaded(Vec<DbSong>),

    // ============ Import ============
    /// Cover cache ready
    CoverCacheReady(Arc<CoverCache>),
    /// Start scanning a folder
    StartScan(PathBuf),
    /// Scan progress update
    ScanProgressUpdate(ScanProgress),
    /// Add folder to watch list
    AddWatchedFolder(PathBuf),
    /// Remove folder from watch list
    RemoveWatchedFolder(PathBuf),
    /// File watcher event
    WatcherEvent(WatchEvent),
    /// Show toast notification
    ShowToast(String),
    /// Show error toast notification
    ShowErrorToast(String),
    /// Hide toast notification
    HideToast,
    /// Clear importing playlist from sidebar
    ClearImportingPlaylist,
    /// Playlist created in database
    PlaylistCreated(i64),

    // ============ Playlist page ============
    /// Navigate to playlist detail page
    OpenPlaylist(i64),
    /// Request to delete a local playlist (shows confirmation dialog)
    RequestDeletePlaylist(i64),
    /// Confirm playlist deletion
    ConfirmDeletePlaylist,
    /// Cancel playlist deletion
    CancelDeletePlaylist,
    /// Playlist deleted confirmation
    PlaylistDeleted(i64),
    /// Playlist view loaded from database
    PlaylistViewLoaded(pages::PlaylistView),
    /// NCM playlist songs converted (async cover check complete)
    /// (playlist_id, song_views, cover_path, palette, avatar_path)
    NcmPlaylistSongsReady(
        i64,
        Vec<crate::ui::pages::PlaylistSongView>,
        Option<String>,
        crate::utils::ColorPalette,
        Option<String>,
    ),
    /// Play a specific song
    PlaySong(i64),
    /// Hover over a song in playlist
    HoverSong(Option<i64>),
    /// Hover over an icon button
    HoverIcon(Option<IconId>),
    /// Hover over a sidebar item
    HoverSidebar(Option<SidebarId>),
    /// Animation tick
    AnimationTick,
    /// Toggle playlist search input expansion
    TogglePlaylistSearch,
    /// Playlist search query changed
    PlaylistSearchChanged(String),
    /// Submit playlist search (Enter key)
    PlaylistSearchSubmit,
    /// Playlist search input lost focus
    PlaylistSearchBlur,

    // ============ Edit dialog ============
    /// Edit playlist (open edit dialog)
    EditPlaylist(i64),
    /// Close edit dialog
    CloseEditDialog,
    /// Edit form: name changed
    EditPlaylistNameChanged(String),
    /// Edit form: description changed
    EditPlaylistDescriptionChanged(String),
    /// Pick cover image
    PickCoverImage,
    /// Cover image picked
    CoverImagePicked(Option<String>),
    /// Save playlist edits
    SavePlaylistEdits,
    /// Playlist updated in database (with playlist id to reload)
    PlaylistUpdated(i64),

    // ============ Lyrics page ============
    /// Open lyrics page
    OpenLyricsPage,
    /// Close lyrics page
    CloseLyricsPage,
    /// Scroll lyrics manually (delta in pixels)
    LyricsScroll(f32),
    /// Window resized (for lyrics viewport calculation)
    WindowResized(iced::Size),
    /// Font system initialized asynchronously (for lyrics text shaping)
    LyricsFontSystemReady(crate::features::lyrics::engine::SharedFontSystem),
    /// Lyrics loaded from online (song_id, lyrics_lines)
    LyricsLoaded(i64, Vec<crate::ui::pages::LyricLine>),
    /// Lyrics loading failed
    LyricsLoadFailed(i64, String),
    /// Preload lyrics for a song (song_id, ncm_id, song_name, singer, album)
    PreloadLyrics(i64, u64, String, String, String),
    /// Local/cached lyrics loaded asynchronously (song_id, lyrics_lines)
    LocalLyricsReady(i64, Vec<crate::ui::pages::LyricLine>),
    /// Engine lines pre-computed asynchronously (song_id, engine_lines)
    LyricsEngineLinesReady(
        i64,
        std::sync::Arc<Vec<crate::features::lyrics::engine::LyricLineData>>,
    ),
    /// 异步预计算的 shaped lines (song_id, shaped_lines, pre_generated_sdf_bitmaps)
    /// 文本布局的唯一数据源，在后台线程计算
    /// 包含预生成的 SDF 位图，避免首次渲染时阻塞主线程
    LyricsShapedLinesReady(
        i64,
        std::sync::Arc<Vec<crate::features::lyrics::engine::CachedShapedLine>>,
        std::collections::HashMap<
            cosmic_text::CacheKey,
            crate::features::lyrics::engine::sdf_generator::SdfBitmap,
        >,
    ),
    /// Background colors extracted asynchronously (song_id, primary, secondary, tertiary)
    LyricsBackgroundReady(i64, [f32; 4], [f32; 4], [f32; 4]),
    /// Album cover image loaded asynchronously for lyrics background (song_id, image_data, width, height)
    LyricsCoverImageReady(i64, Vec<u8>, u32, u32),

    // ============ Playback controls ============
    /// Toggle play/pause
    TogglePlayback,
    /// Play next song
    NextSong,
    /// Play previous song
    PrevSong,
    /// Update seek preview position while dragging (0.0 to 1.0)
    SeekPreview(f32),
    /// Finish seeking and apply the preview position
    SeekRelease,
    /// Set volume (0.0 to 1.0)
    SetVolume(f32),
    /// Playback tick (for progress updates)
    PlaybackTick,
    /// Toggle queue panel visibility
    ToggleQueue,
    /// Cycle to next play mode
    CyclePlayMode,
    /// Audio preload ready (new architecture) - (queue_index, file_path, is_next)
    PreloadAudioReady(usize, String, bool),
    /// Audio preload failed (new architecture) - (queue_index, is_next)
    PreloadAudioFailed(usize, bool),

    // ============ Queue management ============
    /// Play entire playlist (replace queue with playlist songs)
    PlayPlaylist(i64),
    /// Play a song from queue by index
    PlayQueueIndex(usize),
    /// Song resolved successfully (index, file_path, cover_path)
    SongResolved(usize, String, Option<String>),
    /// Song resolution failed
    SongResolveFailed,
    /// Remove song from queue by index
    RemoveFromQueue(usize),
    /// Clear the entire queue
    ClearQueue,

    // ============ Keyboard events ============
    /// Keyboard key pressed
    KeyPressed(Key, Modifiers),
    /// Execute a keybinding action
    ExecuteAction(Action),

    // ============ Exit dialog ============
    /// Request to close the window (triggers exit dialog if needed)
    RequestClose,
    /// Confirm exit and close the application
    ConfirmExit,
    /// Minimize to system tray
    MinimizeToTray,
    /// Cancel exit dialog
    CancelExit,
    /// Toggle "remember my choice" checkbox
    ExitDialogRememberChanged(bool),

    // ============ System Tray ============
    /// Tray service started
    TrayStarted(
        std::sync::Arc<
            tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<crate::features::TrayCommand>>,
        >,
    ),
    /// Tray command received
    TrayCommand(crate::features::TrayCommand),

    // ============ Media Controls ============
    /// Media controls service started
    MprisStartedWithHandle(
        crate::platform::media_controls::MediaHandle,
        std::sync::Arc<
            tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<crate::platform::media_controls::MediaCommand>>,
        >,
    ),
    /// Media controls command received
    MprisCommand(crate::platform::media_controls::MediaCommand),
    /// Show window from tray
    ShowWindow,
    /// Toggle window visibility
    ToggleWindow,
    /// Window operation completed (for debouncing)
    WindowOperationComplete,

    // ============ NCM Login ============
    /// Initialize NCM client
    NcmClientReady(NcmClient),
    /// Try to auto-login with saved cookies
    TryAutoLogin(u8),
    /// Auto login result
    AutoLoginResult(Option<LoginInfo>, u8),
    /// Request QR code for login
    RequestQrCode,
    /// QR code generated
    QrCodeReady(PathBuf, String),
    /// Check QR code scan status
    CheckQrStatus(String),
    /// QR code login result
    QrLoginResult(QrLoginStatus),
    /// Login successful
    LoginSuccess(LoginInfo),
    /// Logout
    Logout,
    /// User info loaded
    UserInfoLoaded(UserInfo),
    /// User avatar loaded
    UserAvatarLoaded(std::path::PathBuf),
    /// Toggle login popup visibility
    ToggleLoginPopup,
    /// No operation (placeholder)
    NoOp,

    // ============ NCM Homepage Data ============
    /// Banners loaded
    BannersLoaded(Vec<BannersInfo>),
    /// Banner image loaded with dimensions (index, path, width, height)
    BannerImageLoaded(usize, PathBuf, u32, u32),
    /// Banner play button clicked
    BannerPlay(usize),
    /// Carousel navigate
    CarouselNavigate(i32),
    /// Carousel auto-advance tick
    CarouselTick,
    /// Top picks (trending playlists) loaded
    TopPicksLoaded(Vec<SongList>),
    /// Toplist (charts) loaded
    ToplistLoaded(Vec<TopList>),
    /// Trending songs (飙升榜) loaded
    TrendingSongsLoaded(Vec<SongInfo>),
    /// Navigate to trending songs page
    OpenTrendingSongs,
    /// Song cover loaded
    SongCoverLoaded(u64, PathBuf),
    /// Toggle favorite status for a song
    ToggleFavorite(u64),
    /// Favorite status changed
    FavoriteStatusChanged(u64, bool),
    /// Play NCM song
    PlayNcmSong(SongInfo),
    /// Play NCM song by URL with optional cover path
    PlayNcmUrl(SongInfo, String, Option<String>),
    /// Add NCM songs to queue
    AddNcmPlaylist(Vec<SongInfo>, bool),
    /// Open NCM playlist detail page
    OpenNcmPlaylist(u64),
    /// Play resolved NCM song (with real DB ID)
    PlayResolvedNcmSong(DbSong),
    /// Queue song upserted to DB (index, updated_song)
    QueueSongUpserted(usize, DbSong),
    /// Toggle favorite status for banner item
    ToggleBannerFavorite(usize),

    // ============ Cloud Playlist ============
    /// Cloud playlist songs loaded
    CloudPlaylistLoaded(Vec<SongInfo>),
    /// User playlists loaded
    UserPlaylistsLoaded(Vec<SongList>),
    /// NCM playlist detail loaded
    NcmPlaylistDetailLoaded(PlayListDetail),
    /// Current playing song cover downloaded (song_id, local_path)
    CurrentSongCoverReady(i64, String),
    /// NCM playlist song covers batch loaded (vec of (song_id, local_path))
    NcmPlaylistSongCoversBatchLoaded(Vec<(i64, String)>),
    /// Request lazy loading of song covers for visible items (song_id, pic_url)
    RequestSongCoversLazy(Vec<(i64, String)>),
    /// NCM playlist cover loaded (playlist_id, local_path)
    NcmPlaylistCoverLoaded(i64, String),
    /// Toggle playlist subscription (subscribe/unsubscribe)
    TogglePlaylistSubscribe(i64),
    /// Playlist subscription status changed
    PlaylistSubscribeChanged(i64, bool),

    /// Hover over a trending song
    HoverTrendingSong(Option<u64>),

    // ============ Discover Page ============
    /// Recommended playlists loaded (for logged-in users)
    RecommendedPlaylistsLoaded(Vec<SongList>),
    /// Hot playlists loaded (playlists, has_more)
    HotPlaylistsLoaded(Vec<SongList>, bool),
    /// Discover playlist cover loaded (playlist_id, local_path)
    DiscoverPlaylistCoverLoaded(u64, PathBuf),
    /// Discover playlist cover GPU allocation completed
    DiscoverCoverAllocated(
        u64,
        Result<iced::widget::image::Allocation, iced::widget::image::Error>,
    ),
    /// Hover over a discover playlist card
    HoverDiscoverPlaylist(Option<u64>),
    /// Play a playlist from discover page
    PlayDiscoverPlaylist(u64),
    /// Load more hot playlists (pagination)
    LoadMoreHotPlaylists,
    /// See all recommended playlists
    SeeAllRecommended,
    /// See all hot playlists
    SeeAllHot,

    // ============ Sidebar Resize ============
    /// Start dragging sidebar resize handle
    SidebarResizeStart,
    /// Stop dragging sidebar resize handle
    SidebarResizeEnd,
}

/// Icon identifiers for hover tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconId {
    PlayButton,
    Edit,
    Delete,
    Search,
    Sort,
    Like,
    Download,
}

/// Sidebar item identifiers for hover tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SidebarId {
    Nav(usize),        // Navigation items (Home, Discover, Radio)
    Library(usize),    // Library items
    Playlist(i64),     // Playlist by ID
    UserPlaylist(u64), // NCM Playlist by ID
    UserCard,          // User profile card at bottom
}

/// QR login status
#[derive(Debug, Clone)]
pub enum QrLoginStatus {
    /// Waiting for scan (801)
    WaitingForScan,
    /// Scanned, waiting for confirmation (802)
    WaitingForConfirm,
    /// Expired (800)
    Expired,
    /// Success (803)
    Success,
    /// Error
    Error(String),
}

// Manual Debug implementation to avoid slow formatting of large data structures
// This prevents the "Slow Debug implementation" warning from iced_debug
impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Use a macro to reduce boilerplate for simple variants
        macro_rules! simple {
            ($name:literal) => { write!(f, $name) };
            ($name:literal, $($arg:tt)*) => { write!(f, concat!($name, "({})"), format_args!($($arg)*)) };
        }

        match self {
            // High-frequency messages - keep minimal (no data)
            Self::AnimationTick => simple!("AnimationTick"),
            Self::PlaybackTick => simple!("PlaybackTick"),
            Self::CarouselTick => simple!("CarouselTick"),
            Self::Noop => simple!("Noop"),
            Self::NoOp => simple!("NoOp"),

            // Large Vec data - only show count
            Self::SongsLoaded(v) => simple!("SongsLoaded", "{} songs", v.len()),
            Self::PlaylistsLoaded(v) => simple!("PlaylistsLoaded", "{} playlists", v.len()),
            Self::QueueRestored(v) => simple!("QueueRestored", "{} songs", v.len()),
            Self::SongResolvedForRestore(idx, result, pos) => {
                simple!(
                    "SongResolvedForRestore",
                    "idx={}, resolved={}, pos={:.1}s",
                    idx,
                    result.is_some(),
                    pos
                )
            }
            Self::QueueLoaded(v) => simple!("QueueLoaded", "{} songs", v.len()),
            Self::RecentlyPlayedLoaded(v) => simple!("RecentlyPlayedLoaded", "{} songs", v.len()),
            Self::BannersLoaded(v) => simple!("BannersLoaded", "{} banners", v.len()),
            Self::TopPicksLoaded(v) => simple!("TopPicksLoaded", "{} picks", v.len()),
            Self::ToplistLoaded(v) => simple!("ToplistLoaded", "{} lists", v.len()),
            Self::TrendingSongsLoaded(v) => simple!("TrendingSongsLoaded", "{} songs", v.len()),
            Self::CloudPlaylistLoaded(v) => simple!("CloudPlaylistLoaded", "{} songs", v.len()),
            Self::UserPlaylistsLoaded(v) => simple!("UserPlaylistsLoaded", "{} playlists", v.len()),
            Self::NcmPlaylistSongCoversBatchLoaded(v) => {
                simple!("NcmPlaylistSongCoversBatchLoaded", "{} covers", v.len())
            }
            Self::RequestSongCoversLazy(v) => simple!("RequestSongCoversLazy", "{} songs", v.len()),
            Self::AddNcmPlaylist(v, play) => {
                simple!("AddNcmPlaylist", "{} songs, play={}", v.len(), play)
            }

            // Arc-wrapped types - just show variant name
            Self::DatabaseReady(_) => simple!("DatabaseReady"),
            Self::CoverCacheReady(_) => simple!("CoverCacheReady"),
            Self::TrayStarted(_) => simple!("TrayStarted"),

            // Complex types - show key identifier only
            Self::NcmPlaylistDetailLoaded(d) => simple!("NcmPlaylistDetailLoaded", "id={}", d.id),
            Self::PlaylistViewLoaded(v) => simple!("PlaylistViewLoaded", "id={}", v.id),
            Self::NcmPlaylistSongsReady(id, songs, _, _, _) => {
                simple!("NcmPlaylistSongsReady", "id={}, {} songs", id, songs.len())
            }
            Self::PlaybackStateLoaded(_) => simple!("PlaybackStateLoaded"),
            Self::ScanProgressUpdate(_) => simple!("ScanProgressUpdate"),
            Self::LoginSuccess(_) => simple!("LoginSuccess"),
            Self::UserInfoLoaded(_) => simple!("UserInfoLoaded"),
            Self::AutoLoginResult(r, retry) => simple!(
                "AutoLoginResult",
                "success={}, retry={}",
                r.is_some(),
                retry
            ),
            Self::PlayNcmSong(s) => simple!("PlayNcmSong", "id={}", s.id),
            Self::PlayNcmUrl(s, _, _) => simple!("PlayNcmUrl", "id={}", s.id),
            Self::PlayResolvedNcmSong(s) => simple!("PlayResolvedNcmSong", "id={}", s.id),
            Self::QueueSongUpserted(i, s) => simple!("QueueSongUpserted", "idx={}, id={}", i, s.id),

            // Navigation
            Self::Navigate(nav) => simple!("Navigate", "{:?}", nav),
            Self::NavigateBack => simple!("NavigateBack"),
            Self::NavigateForward => simple!("NavigateForward"),
            Self::LibrarySelect(item) => simple!("LibrarySelect", "{:?}", item),
            Self::SearchChanged(_) => simple!("SearchChanged"),
            Self::PlayHero => simple!("PlayHero"),
            Self::ImportLocalPlaylist => simple!("ImportLocalPlaylist"),
            Self::FolderSelected(p) => simple!("FolderSelected", "{:?}", p.as_ref().map(|_| "...")),

            // Window
            Self::WindowMinimize => simple!("WindowMinimize"),
            Self::WindowMaximize => simple!("WindowMaximize"),
            Self::MousePressed => simple!("MousePressed"),
            Self::MouseReleased => simple!("MouseReleased"),
            Self::MouseMoved(_) => simple!("MouseMoved"),
            Self::OpenSettings => simple!("OpenSettings"),
            Self::OpenSettingsWithCloseLyrics => simple!("OpenSettingsWithCloseLyrics"),
            Self::OpenAudioEngine => simple!("OpenAudioEngine"),

            // Settings - most are simple
            Self::UpdateCloseBehavior(b) => simple!("UpdateCloseBehavior", "{:?}", b),
            Self::SaveSettings => simple!("SaveSettings"),
            Self::UpdateFadeInOut(b) => simple!("UpdateFadeInOut", "{}", b),
            Self::UpdateVolumeNormalization(b) => simple!("UpdateVolumeNormalization", "{}", b),
            Self::UpdateMusicQuality(q) => simple!("UpdateMusicQuality", "{:?}", q),
            Self::UpdateEqualizerEnabled(b) => simple!("UpdateEqualizerEnabled", "{}", b),
            Self::UpdateEqualizerPreset(p) => simple!("UpdateEqualizerPreset", "{:?}", p),
            Self::UpdateEqualizerValues(_) => simple!("UpdateEqualizerValues"),
            Self::UpdateEqualizerPreamp(v) => simple!("UpdateEqualizerPreamp", "{:.1}", v),
            Self::UpdateSpectrumDecay(v) => simple!("UpdateSpectrumDecay", "{:.2}", v),
            Self::UpdateSpectrumBarsMode(b) => simple!("UpdateSpectrumBarsMode", "{}", b),
            Self::UpdateDarkMode(b) => simple!("UpdateDarkMode", "{}", b),
            Self::UpdateAppLanguage(l) => simple!("UpdateAppLanguage", "{}", l),
            Self::UpdatePowerSavingMode(b) => simple!("UpdatePowerSavingMode", "{}", b),
            Self::UpdateMaxCacheMb(m) => simple!("UpdateMaxCacheMb", "{}", m),
            Self::ClearCache => simple!("ClearCache"),
            Self::CacheCleared(n, b) => simple!("CacheCleared", "{} files, {} bytes", n, b),
            Self::RefreshCacheStats => simple!("RefreshCacheStats"),
            Self::EnforceCacheLimit => simple!("EnforceCacheLimit"),
            Self::UpdateAudioOutputDevice(_) => simple!("UpdateAudioOutputDevice"),
            Self::UpdateAudioBufferSize(s) => simple!("UpdateAudioBufferSize", "{}", s),
            Self::UpdateProxyType(t) => simple!("UpdateProxyType", "{:?}", t),
            Self::UpdateProxyHost(_) => simple!("UpdateProxyHost"),
            Self::UpdateProxyPort(_) => simple!("UpdateProxyPort"),
            Self::UpdateProxyUsername(_) => simple!("UpdateProxyUsername"),
            Self::UpdateProxyPassword(_) => simple!("UpdateProxyPassword"),
            Self::ApplyProxySettings => simple!("ApplyProxySettings"),
            Self::ScrollToSection(s) => simple!("ScrollToSection", "{:?}", s),
            Self::SettingsScrolled(y) => simple!("SettingsScrolled", "{:.0}", y),
            Self::StartEditingKeybinding(a) => simple!("StartEditingKeybinding", "{:?}", a),
            Self::CancelEditingKeybinding => simple!("CancelEditingKeybinding"),
            Self::KeybindingKeyPressed(_, _) => simple!("KeybindingKeyPressed"),

            // Database
            Self::DatabaseError(e) => simple!("DatabaseError", "{}", e),
            Self::SongsValidated(n) => simple!("SongsValidated", "{}", n),

            // Import
            Self::StartScan(_) => simple!("StartScan"),
            Self::AddWatchedFolder(_) => simple!("AddWatchedFolder"),
            Self::RemoveWatchedFolder(_) => simple!("RemoveWatchedFolder"),
            Self::WatcherEvent(_) => simple!("WatcherEvent"),
            Self::ShowToast(_) => simple!("ShowToast"),
            Self::ShowErrorToast(_) => simple!("ShowErrorToast"),
            Self::HideToast => simple!("HideToast"),
            Self::ClearImportingPlaylist => simple!("ClearImportingPlaylist"),
            Self::PlaylistCreated(id) => simple!("PlaylistCreated", "{}", id),

            // Playlist page
            Self::OpenPlaylist(id) => simple!("OpenPlaylist", "{}", id),
            Self::RequestDeletePlaylist(id) => simple!("RequestDeletePlaylist", "{}", id),
            Self::ConfirmDeletePlaylist => simple!("ConfirmDeletePlaylist"),
            Self::CancelDeletePlaylist => simple!("CancelDeletePlaylist"),
            Self::PlaylistDeleted(id) => simple!("PlaylistDeleted", "{}", id),
            Self::PlaySong(id) => simple!("PlaySong", "{}", id),
            Self::HoverSong(id) => simple!("HoverSong", "{:?}", id),
            Self::HoverIcon(id) => simple!("HoverIcon", "{:?}", id),
            Self::HoverSidebar(id) => simple!("HoverSidebar", "{:?}", id),
            Self::TogglePlaylistSearch => simple!("TogglePlaylistSearch"),
            Self::PlaylistSearchChanged(_) => simple!("PlaylistSearchChanged"),
            Self::PlaylistSearchSubmit => simple!("PlaylistSearchSubmit"),
            Self::PlaylistSearchBlur => simple!("PlaylistSearchBlur"),

            // Edit dialog
            Self::EditPlaylist(id) => simple!("EditPlaylist", "{}", id),
            Self::CloseEditDialog => simple!("CloseEditDialog"),
            Self::EditPlaylistNameChanged(_) => simple!("EditPlaylistNameChanged"),
            Self::EditPlaylistDescriptionChanged(_) => simple!("EditPlaylistDescriptionChanged"),
            Self::PickCoverImage => simple!("PickCoverImage"),
            Self::CoverImagePicked(_) => simple!("CoverImagePicked"),
            Self::SavePlaylistEdits => simple!("SavePlaylistEdits"),
            Self::PlaylistUpdated(id) => simple!("PlaylistUpdated", "{}", id),

            // Lyrics
            Self::OpenLyricsPage => simple!("OpenLyricsPage"),
            Self::CloseLyricsPage => simple!("CloseLyricsPage"),
            Self::LyricsScroll(d) => simple!("LyricsScroll", "{:.1}", d),
            Self::WindowResized(size) => simple!("WindowResized", "{}x{}", size.width, size.height),
            Self::LyricsFontSystemReady(_) => simple!("LyricsFontSystemReady"),
            Self::LyricsLoaded(id, lines) => {
                simple!("LyricsLoaded", "id={}, {} lines", id, lines.len())
            }
            Self::LyricsLoadFailed(id, _) => simple!("LyricsLoadFailed", "id={}", id),
            Self::PreloadLyrics(id, _, _, _, _) => simple!("PreloadLyrics", "id={}", id),
            Self::LocalLyricsReady(id, lines) => {
                simple!("LocalLyricsReady", "id={}, {} lines", id, lines.len())
            }
            Self::LyricsEngineLinesReady(id, lines) => {
                simple!("LyricsEngineLinesReady", "id={}, {} lines", id, lines.len())
            }
            Self::LyricsShapedLinesReady(id, lines, bitmaps) => simple!(
                "LyricsShapedLinesReady",
                "id={}, {} lines, {} bitmaps",
                id,
                lines.len(),
                bitmaps.len()
            ),
            Self::LyricsBackgroundReady(id, _, _, _) => {
                simple!("LyricsBackgroundReady", "id={}", id)
            }
            Self::LyricsCoverImageReady(id, _, w, h) => {
                simple!("LyricsCoverImageReady", "id={}, {}x{}", id, w, h)
            }

            // Playback controls
            Self::TogglePlayback => simple!("TogglePlayback"),
            Self::NextSong => simple!("NextSong"),
            Self::PrevSong => simple!("PrevSong"),
            Self::SeekPreview(p) => simple!("SeekPreview", "{:.2}", p),
            Self::SeekRelease => simple!("SeekRelease"),
            Self::SetVolume(v) => simple!("SetVolume", "{:.2}", v),
            Self::ToggleQueue => simple!("ToggleQueue"),
            Self::CyclePlayMode => simple!("CyclePlayMode"),
            Self::PreloadAudioReady(idx, _, is_next) => {
                simple!("PreloadAudioReady", "idx={}, next={}", idx, is_next)
            }
            Self::PreloadAudioFailed(idx, is_next) => {
                simple!("PreloadAudioFailed", "idx={}, next={}", idx, is_next)
            }

            // Queue management
            Self::PlayPlaylist(id) => simple!("PlayPlaylist", "{}", id),
            Self::PlayQueueIndex(i) => simple!("PlayQueueIndex", "{}", i),
            Self::SongResolved(i, _, _) => simple!("SongResolved", "idx={}", i),
            Self::SongResolveFailed => simple!("SongResolveFailed"),
            Self::RemoveFromQueue(i) => simple!("RemoveFromQueue", "{}", i),
            Self::ClearQueue => simple!("ClearQueue"),

            // Keyboard
            Self::KeyPressed(_, _) => simple!("KeyPressed"),
            Self::ExecuteAction(a) => simple!("ExecuteAction", "{:?}", a),

            // Exit dialog
            Self::RequestClose => simple!("RequestClose"),
            Self::ConfirmExit => simple!("ConfirmExit"),
            Self::MinimizeToTray => simple!("MinimizeToTray"),
            Self::CancelExit => simple!("CancelExit"),
            Self::ExitDialogRememberChanged(b) => simple!("ExitDialogRememberChanged", "{}", b),

            // Tray
            Self::TrayCommand(c) => simple!("TrayCommand", "{:?}", c),

            // Media Controls
            Self::MprisCommand(c) => simple!("MprisCommand", "{:?}", c),
            Self::MprisStartedWithHandle(_, _) => simple!("MprisStartedWithHandle"),
            Self::ShowWindow => simple!("ShowWindow"),
            Self::ToggleWindow => simple!("ToggleWindow"),
            Self::WindowOperationComplete => simple!("WindowOperationComplete"),

            // NCM Login
            Self::NcmClientReady(_) => simple!("NcmClientReady"),
            Self::TryAutoLogin(retry) => simple!("TryAutoLogin", "retry={}", retry),
            Self::RequestQrCode => simple!("RequestQrCode"),
            Self::QrCodeReady(_, _) => simple!("QrCodeReady"),
            Self::CheckQrStatus(_) => simple!("CheckQrStatus"),
            Self::QrLoginResult(s) => simple!("QrLoginResult", "{:?}", s),
            Self::Logout => simple!("Logout"),
            Self::UserAvatarLoaded(_) => simple!("UserAvatarLoaded"),
            Self::ToggleLoginPopup => simple!("ToggleLoginPopup"),

            // NCM Homepage
            Self::BannerImageLoaded(i, _, _, _) => simple!("BannerImageLoaded", "idx={}", i),
            Self::BannerPlay(i) => simple!("BannerPlay", "{}", i),
            Self::CarouselNavigate(d) => simple!("CarouselNavigate", "{}", d),
            Self::OpenTrendingSongs => simple!("OpenTrendingSongs"),
            Self::SongCoverLoaded(id, _) => simple!("SongCoverLoaded", "{}", id),
            Self::ToggleFavorite(id) => simple!("ToggleFavorite", "{}", id),
            Self::FavoriteStatusChanged(id, s) => simple!("FavoriteStatusChanged", "{}, {}", id, s),
            Self::OpenNcmPlaylist(id) => simple!("OpenNcmPlaylist", "{}", id),
            Self::ToggleBannerFavorite(i) => simple!("ToggleBannerFavorite", "{}", i),

            // Cloud Playlist
            Self::CurrentSongCoverReady(id, _) => simple!("CurrentSongCoverReady", "{}", id),
            Self::NcmPlaylistCoverLoaded(id, _) => simple!("NcmPlaylistCoverLoaded", "{}", id),
            Self::TogglePlaylistSubscribe(id) => simple!("TogglePlaylistSubscribe", "{}", id),
            Self::PlaylistSubscribeChanged(id, s) => {
                simple!("PlaylistSubscribeChanged", "{}, {}", id, s)
            }
            Self::HoverTrendingSong(id) => simple!("HoverTrendingSong", "{:?}", id),

            // Discover Page
            Self::RecommendedPlaylistsLoaded(v) => {
                simple!("RecommendedPlaylistsLoaded", "{} playlists", v.len())
            }
            Self::HotPlaylistsLoaded(v, more) => {
                simple!("HotPlaylistsLoaded", "{} playlists, more={}", v.len(), more)
            }
            Self::DiscoverPlaylistCoverLoaded(id, _) => {
                simple!("DiscoverPlaylistCoverLoaded", "{}", id)
            }
            Self::DiscoverCoverAllocated(id, _) => simple!("DiscoverCoverAllocated", "{}", id),
            Self::HoverDiscoverPlaylist(id) => simple!("HoverDiscoverPlaylist", "{:?}", id),
            Self::PlayDiscoverPlaylist(id) => simple!("PlayDiscoverPlaylist", "{}", id),
            Self::LoadMoreHotPlaylists => simple!("LoadMoreHotPlaylists"),
            Self::SeeAllRecommended => simple!("SeeAllRecommended"),
            Self::SeeAllHot => simple!("SeeAllHot"),

            // Sidebar resize
            Self::SidebarResizeStart => simple!("SidebarResizeStart"),
            Self::SidebarResizeEnd => simple!("SidebarResizeEnd"),
        }
    }
}
