//! Application messages

use std::path::PathBuf;
use std::sync::Arc;

use iced::keyboard::{Key, Modifiers};

use crate::api::{
    AlbumDetail, AlbumSummary, ArtistDetail, ArtistSummary, Banner, LoginInfo, PlaylistDetail,
    PlaylistSummary, Track, UserDetail,
};
use crate::app::state::UserInfo;
use crate::app::update::audio_preload_manager::PreloadDirection;
use crate::database::{Database, DbPlaybackState, DbPlaylist, DbSong, DbWatchedFolder};
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

impl SettingsSection {
    /// Widget ID for snap_to scrolling in the settings page
    pub fn widget_id(self) -> iced::widget::Id {
        iced::widget::Id::new(match self {
            SettingsSection::Account => "settings_section_account",
            SettingsSection::Playback => "settings_section_playback",
            SettingsSection::Display => "settings_section_display",
            SettingsSection::System => "settings_section_system",
            SettingsSection::Network => "settings_section_network",
            SettingsSection::Storage => "settings_section_storage",
            SettingsSection::Shortcuts => "settings_section_shortcuts",
            SettingsSection::About => "settings_section_about",
        })
    }
}

/// Target state bucket for measured responsive content widths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentWidthTarget {
    Search,
    Discover,
    PlaylistDetail,
}

/// Search results payload for async loading
#[derive(Debug, Clone)]
pub struct SearchResultsPayload {
    pub tab: crate::app::state::SearchTab,
    pub tracks: Vec<Track>,
    pub albums: Vec<AlbumSummary>,
    pub artists: Vec<ArtistSummary>,
    pub playlists: Vec<PlaylistSummary>,
    pub total_count: u32,
}

/// Local playlist view plus image paths discovered while loading it.
#[derive(Debug, Clone)]
pub struct PlaylistViewPayload {
    pub view: pages::PlaylistView,
    pub images: Vec<(crate::image::ImageKind, u64, PathBuf)>,
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
    /// Mouse wheel scrolled (for volume control etc.)
    MouseWheelScrolled { delta_y: f32 },
    /// Mouse entered/exited volume slider area
    VolumeSliderHovered(bool),
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
    /// Update lyrics panel settings
    UpdateLyricsFontFamily(Option<String>),
    /// Update storage settings
    UpdateMaxCacheMb(u64),
    UpdateDownloadDir(Option<String>),
    /// Open folder dialog for download directory
    UpdateDownloadDirDialog,
    UpdateDownloadQuality(crate::features::MusicQuality),
    ClearCache,
    /// Cache cleared result (files_deleted, bytes_freed)
    CacheCleared(usize, u64),
    /// Refresh cache statistics
    RefreshCacheStats,
    /// Enforce cache size limit
    EnforceCacheLimit,
    /// Update system settings
    UpdateAudioOutputDevice(Option<String>),
    /// Toggle Discord Rich Presence
    UpdateDiscordEnabled(bool),
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
    /// Trigger measurement of section widget positions
    MeasureSectionPositions,
    /// Section positions measured from the widget tree
    SectionPositionsMeasured(Vec<(SettingsSection, f32)>),
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
    /// Download history loaded from database
    DownloadsLoaded(Vec<crate::database::DownloadRow>),
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
    /// Watched local library folders restored from database
    WatchedFoldersLoaded(Vec<DbWatchedFolder>),

    // ============ Import ============
    /// Cover cache ready
    CoverCacheReady(Arc<CoverCache>),
    /// Start scanning a folder
    StartScan(PathBuf),
    /// Cancel the active folder scan
    CancelScan,
    /// Scan progress update
    ScanProgressUpdate(ScanProgress),
    /// Import completed and local library playlist was created
    ImportedPlaylistCreated(Result<i64, String>),
    /// File watcher event
    WatcherEvent(WatchEvent),
    /// Background watcher mutation completed for a local playlist
    WatchedFolderSyncCompleted(Option<i64>),
    /// Show info toast notification
    ShowInfoToast(String),
    /// Show success toast notification
    ShowSuccessToast(String),
    /// Show warning toast notification
    ShowWarningToast(String),
    /// Show error toast notification
    ShowErrorToast(String),
    /// Hide toast notification
    HideToast,
    /// Clear importing playlist from sidebar
    ClearImportingPlaylist,

    // ============ Playlist page ============
    /// Navigate to playlist detail page
    OpenPlaylist(i64),
    /// Request to delete a local playlist (shows confirmation dialog)
    RequestDeletePlaylist(i64),
    /// Confirm playlist deletion
    ConfirmDeletePlaylist,
    /// Playlist deleted confirmation
    PlaylistDeleted(i64),
    /// Playlist view loaded from database
    PlaylistViewLoaded(PlaylistViewPayload),
    /// NCM playlist songs converted for display.
    NcmPlaylistSongsReady(i64, Vec<crate::ui::pages::PlaylistSongView>),
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
    /// Toggle playlist description expand/collapse
    ToggleDescriptionExpand,

    // ============ Edit dialog ============
    /// Edit playlist (open edit dialog)
    EditPlaylist(i64),
    /// Edit form: name changed
    EditPlaylistNameChanged(String),
    /// Edit form: description changed
    EditPlaylistDescriptionChanged(String),
    /// Edit form: watched library toggle changed
    EditPlaylistWatchEnabledChanged(bool),
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
    /// Real rendered lyrics viewport resized
    LyricsViewportResized(iced::Size),
    /// Window resized (for lyrics viewport calculation)
    WindowResized(iced::Size),
    /// Font system initialized asynchronously (for lyrics text shaping)
    LyricsFontSystemReady(crate::features::lyrics::engine::SharedFontSystem),
    /// Lyrics loaded from online (song_id, lyrics_lines)
    LyricsLoaded(i64, Vec<crate::ui::pages::LyricLine>),
    /// Lyrics loading failed
    LyricsLoadFailed(i64, String),
    /// Start online lyrics fetch for display loading (song_id, ncm_id)
    FetchLyricsOnline(i64, u64),
    /// Warm lyrics cache for a song in the background (song_id, ncm_id)
    WarmLyricsCache(i64, u64),
    /// Background lyrics cache warmup completed
    LyricsWarmupFinished(i64, Result<(), String>),
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
        u64,
        std::sync::Arc<Vec<crate::features::lyrics::engine::CachedShapedLine>>,
        std::collections::HashMap<
            cosmic_text::CacheKey,
            crate::features::lyrics::engine::sdf_generator::SdfBitmap,
        >,
        f32,
        f32,
        Option<String>,
    ),
    /// Background colors extracted asynchronously (song_id, cover_path, primary, secondary, tertiary)
    LyricsBackgroundReady(i64, String, [f32; 4], [f32; 4], [f32; 4]),
    /// Album cover image loaded asynchronously for lyrics background (song_id, cover_path, image_data, width, height)
    LyricsCoverImageReady(i64, String, Vec<u8>, u32, u32),

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
    /// Audio preload ready (local file cached) - (queue_index, file_path, direction)
    PreloadReady(usize, String, PreloadDirection),
    /// Audio preload ready with SharedBuffer for streaming playback
    /// (queue_index, file_path, direction, shared_buffer, duration_secs)
    PreloadBufferReady(
        usize,
        String,
        PreloadDirection,
        crate::audio::SharedBuffer,
        u64,
    ),
    /// Audio preload failed - (queue_index, direction)
    PreloadAudioFailed(usize, PreloadDirection),

    // ============ Queue management ============
    /// Play entire playlist (replace queue with playlist songs)
    PlayPlaylist(i64),
    /// Play a song from queue by index
    PlayQueueIndex(usize),
    /// Song resolved with streaming support (index, file_path, cover_path, shared_buffer, duration_secs)
    SongResolvedStreaming(
        usize,
        String,
        Option<String>,
        Option<crate::audio::SharedBuffer>,
        Option<u64>,
    ),
    /// Song resolution failed
    SongResolveFailed,
    /// Remove song from queue by index
    RemoveFromQueue(usize),
    /// Clear the entire queue
    ClearQueue,

    // ============ Download ============
    /// Download a single song by song_id
    DownloadSong(i64),
    /// Download URL resolved (song_id, ncm_id, url, metadata)
    DownloadUrlResolved(i64, u64, String, crate::metadata::SongMetadata),
    /// Download all songs in a playlist
    DownloadPlaylist(i64),
    /// Request download playlist with confirmation (playlist_id, name, song_count)
    RequestDownloadPlaylist(i64, String, u32),
    /// Confirm batch download from confirmation dialog
    ConfirmDownloadPlaylist,
    /// Playlist download URLs resolved (vec of (song_id, ncm_id, url, metadata))
    DownloadBatchEnqueue(Vec<(i64, u64, String, crate::metadata::SongMetadata)>),
    /// Cancel a download
    DownloadCancel(i64),
    /// Download progress update (song_id, downloaded_bytes, total_bytes)
    DownloadProgress(i64, u64, u64),
    /// Download completed (song_id, file_path)
    DownloadCompleted(i64, String),
    /// Download failed (song_id, error_message)
    DownloadError(i64, String),
    /// Delete a download history entry by song_id
    DeleteDownloadHistory(i64),
    /// Switch download panel tab
    SwitchDownloadTab(crate::app::DownloadTab),

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
    /// Toggle "remember my choice" checkbox in exit dialog
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
            tokio::sync::Mutex<
                tokio::sync::mpsc::UnboundedReceiver<crate::platform::media_controls::MediaCommand>,
            >,
        >,
    ),
    /// Media controls command received
    MprisCommand(crate::platform::media_controls::MediaCommand),
    /// Show window from tray
    ShowWindow,
    /// Show hidden window or focus the existing visible window
    ShowOrFocusWindow,
    /// Toggle window visibility
    ToggleWindow,
    /// Window finished showing and is ready to render
    WindowShown,
    /// Window gained focus
    WindowFocused,
    /// Window lost focus
    WindowUnfocused,
    /// Window operation completed (for debouncing)
    WindowOperationComplete,

    // ============ NCM Login ============
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
    /// Toggle login popup visibility
    ToggleLoginPopup,
    /// No operation (placeholder)
    NoOp,

    // ============ NCM Homepage Data ============
    /// Banners loaded
    BannersLoaded(Vec<Banner>),
    /// Banner play button clicked
    BannerPlay(usize),
    /// Carousel navigate
    CarouselNavigate(i32),
    /// Carousel auto-advance tick
    CarouselTick,
    /// Top picks (trending playlists) loaded
    TopPicksLoaded(Vec<PlaylistSummary>),
    /// Trending songs (飙升榜) loaded
    TrendingSongsLoaded(Vec<Track>),
    /// Navigate to trending songs page
    OpenTrendingSongs,
    /// Toggle favorite status for a song
    ToggleFavorite(u64),
    /// Favorite status changed
    FavoriteStatusChanged(u64, bool),
    /// Play NCM song
    PlayNcmSong(Track),
    /// Add NCM songs to queue
    AddNcmPlaylist(Vec<Track>, bool),
    AddNcmPlaylistWithSource(Vec<Track>, bool, Option<u64>),
    /// Open NCM playlist detail page
    OpenNcmPlaylist(u64),
    /// Open user detail page
    OpenUser(u64),
    /// Open artist detail page
    OpenArtist(u64),
    /// Open album detail page
    OpenAlbum(u64),
    /// Resolve artist by name then open detail page
    OpenArtistByName(String),
    /// Toggle favorite status for banner item
    ToggleBannerFavorite(usize),

    // ============ Cloud Playlist ============
    /// User playlists loaded
    UserPlaylistsLoaded(Vec<PlaylistSummary>),
    /// NCM playlist detail loaded
    NcmPlaylistDetailLoaded(PlaylistDetail),
    /// Playlist-like detail page failed to load (internal page id, user-facing message)
    PlaylistPageLoadFailed(i64, String),
    /// Artist detail loaded
    ArtistDetailLoaded(ArtistDetail),
    /// Album detail loaded
    AlbumDetailLoaded(AlbumDetail),
    /// Artist albums loaded for artist page
    ArtistAlbumsLoaded(i64, Vec<AlbumSummary>),
    /// User page detail loaded
    UserPageDetailLoaded(i64, UserDetail),
    /// User playlists loaded for user page
    UserPagePlaylistsLoaded(i64, Vec<PlaylistSummary>),
    /// Artist detail loaded for a user page
    UserArtistDetailLoaded(i64, ArtistDetail),
    /// Playlist creator user detail loaded
    PlaylistCreatorDetailLoaded(i64, UserDetail),

    // ============ Unified Image Pipeline ============
    /// Image download completed and cached locally (kind, id, local_path)
    ImageDownloadReady(crate::image::ImageKind, u64, PathBuf),
    /// Image download failed and should be eligible for retry (kind, id)
    ImageDownloadFailed(crate::image::ImageKind, u64),

    /// Toggle playlist subscription (subscribe/unsubscribe)
    TogglePlaylistSubscribe(i64),
    /// Playlist subscription status changed
    PlaylistSubscribeChanged(i64, bool),

    /// Hover over a trending song
    HoverTrendingSong(Option<u64>),

    // ============ Discover Page ============
    /// Recommended playlists loaded (for logged-in users)
    RecommendedPlaylistsLoaded(Vec<PlaylistSummary>),
    /// Hot playlists loaded (playlists, has_more)
    HotPlaylistsLoaded(Vec<PlaylistSummary>, bool),
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

    // ============ Search Page ============
    /// Submit search query (Enter pressed in search bar)
    SearchSubmit,
    /// Change search tab
    SearchTabChanged(crate::app::state::SearchTab),
    /// Search results loaded
    SearchResultsLoaded(SearchResultsPayload),
    /// Search failed
    SearchFailed(String),
    /// Change search page (pagination)
    SearchPageChanged(u32),
    /// Hover over search result song
    HoverSearchSong(Option<u64>),
    /// Hover over search result card (album/playlist)
    HoverSearchCard(Option<u64>),
    /// Measured content area resized
    ContentWidthResized(ContentWidthTarget, iced::Size),
    /// Play search result song
    PlaySearchSong(u64),
    /// Open search result album/playlist
    OpenSearchResult(u64, crate::app::state::SearchTab),
    /// Switch artist page tab
    SwitchArtistTab(crate::ui::pages::playlist::ArtistPageTab),

    // ============ Sidebar Resize ============
    /// Start dragging sidebar resize handle
    SidebarResizeStart,
    /// Stop dragging sidebar resize handle
    SidebarResizeEnd,
    // ============ Player Events (Event-Driven Architecture) ============
    /// Streaming download event (song_id, event)
    StreamingEvent(i64, crate::audio::streaming::StreamingEvent),
    /// Audio thread event
    AudioEvent(crate::audio::AudioEvent),

    // ============ Discord Rich Presence ============
    /// Update Discord presence with current playback state
    DiscordUpdatePresence,
    /// Clear Discord presence
    DiscordClearPresence,

    // ============ Protocol (rustle://) ============
    /// URI received from OS protocol handler or IPC
    UriReceived(String),

    // ============ Context Menu ============
    /// Show context menu for a local song (triggered by right-click)
    RightClickSong(i64),
    /// Show context menu for an NCM/online song
    RightClickNcmSong(Track),
    /// Close context menu
    CloseContextMenu,
    /// Context menu item selected
    ContextMenuAction(ContextMenuAction, i64),
    /// Confirm adding a song to a playlist from the picker modal (song_id, playlist_id)
    PlaylistPickerConfirm(i64, i64),
    /// Add an NCM song to an NCM online playlist (song_ncm_id, ncm_playlist_id)
    AddToNcmPlaylist(u64, u64),
    /// Result of adding song to NCM playlist
    NcmPlaylistAddResult(u64, u64, Result<(), String>),

    // ============ Overlay System (Unified) ============
    /// Dismiss the topmost dismissible overlay
    DismissTopModal,

    // ============ Song Edit ============
    /// Open song edit dialog
    EditSongTags(i64),
    /// Open edit dialog with resolved metadata (DbSong, SongMetadata, cover_path)
    OpenSongEditDialog(
        Box<(
            crate::database::DbSong,
            crate::metadata::SongMetadata,
            Option<PathBuf>,
        )>,
    ),
    /// Song edit field changed
    SongEditFieldChanged {
        song_id: i64,
        field: String,
        value: String,
    },
    /// Song cover replaced
    SongEditCoverReplaced(i64, PathBuf),
    /// Pick a cover image for song edit
    PickSongEditCover(i64),
    /// Save song edits to file
    SaveSongEdits(i64),
    /// Song edits saved successfully
    SongEditsSaved(i64),
    /// Song edits save failed
    SongEditsFailed {
        song_id: i64,
        error: String,
    },
}

/// Context menu action types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMenuAction {
    PlayNow,
    PlayNext,
    AddToFavorites,
    AddToPlaylist,
    ViewArtist,
    ViewAlbum,
    ShowInFolder,
    EditSongTags,
    Download,
    RemoveFromList,
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
            Self::TrendingSongsLoaded(v) => simple!("TrendingSongsLoaded", "{} songs", v.len()),
            Self::UserPlaylistsLoaded(v) => simple!("UserPlaylistsLoaded", "{} playlists", v.len()),
            Self::AddNcmPlaylist(v, play) => {
                simple!("AddNcmPlaylist", "{} songs, play={}", v.len(), play)
            }
            Self::AddNcmPlaylistWithSource(v, play, source) => {
                simple!(
                    "AddNcmPlaylistWithSource",
                    "{} songs, play={}, source={:?}",
                    v.len(),
                    play,
                    source
                )
            }

            // Arc-wrapped types - just show variant name
            Self::DatabaseReady(_) => simple!("DatabaseReady"),
            Self::CoverCacheReady(_) => simple!("CoverCacheReady"),
            Self::TrayStarted(_) => simple!("TrayStarted"),

            // Complex types - show key identifier only
            Self::NcmPlaylistDetailLoaded(d) => simple!("NcmPlaylistDetailLoaded", "id={}", d.id),
            Self::PlaylistPageLoadFailed(id, _) => {
                simple!("PlaylistPageLoadFailed", "id={}", id)
            }
            Self::ArtistDetailLoaded(d) => simple!("ArtistDetailLoaded", "id={}", d.id),
            Self::AlbumDetailLoaded(d) => simple!("AlbumDetailLoaded", "id={}", d.id),
            Self::ArtistAlbumsLoaded(id, albums) => {
                simple!(
                    "ArtistAlbumsLoaded",
                    "page_id={}, albums={}",
                    id,
                    albums.len()
                )
            }
            Self::UserPageDetailLoaded(id, d) => {
                simple!(
                    "UserPageDetailLoaded",
                    "page_id={}, user_id={}",
                    id,
                    d.user_id
                )
            }
            Self::UserPagePlaylistsLoaded(id, playlists) => {
                simple!(
                    "UserPagePlaylistsLoaded",
                    "page_id={}, playlists={}",
                    id,
                    playlists.len()
                )
            }
            Self::UserArtistDetailLoaded(id, d) => {
                simple!(
                    "UserArtistDetailLoaded",
                    "page_id={}, artist_id={}",
                    id,
                    d.id
                )
            }
            Self::PlaylistCreatorDetailLoaded(id, d) => {
                simple!(
                    "PlaylistCreatorDetailLoaded",
                    "playlist_id={}, user_id={}",
                    id,
                    d.user_id
                )
            }
            Self::PlaylistViewLoaded(payload) => {
                simple!("PlaylistViewLoaded", "id={}", payload.view.id)
            }
            Self::NcmPlaylistSongsReady(id, songs) => {
                simple!("NcmPlaylistSongsReady", "id={}, {} songs", id, songs.len())
            }
            Self::PlaybackStateLoaded(_) => simple!("PlaybackStateLoaded"),
            Self::ScanProgressUpdate(_) => simple!("ScanProgressUpdate"),
            Self::WatchedFoldersLoaded(v) => {
                simple!("WatchedFoldersLoaded", "{} folders", v.len())
            }
            Self::LoginSuccess(_) => simple!("LoginSuccess"),
            Self::UserInfoLoaded(_) => simple!("UserInfoLoaded"),
            Self::AutoLoginResult(r, retry) => simple!(
                "AutoLoginResult",
                "success={}, retry={}",
                r.is_some(),
                retry
            ),
            Self::PlayNcmSong(s) => simple!("PlayNcmSong", "id={}", s.id),

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
            Self::MouseWheelScrolled { delta_y } => {
                simple!("MouseWheelScrolled", "{:.2}", delta_y)
            }
            Self::VolumeSliderHovered(h) => simple!("VolumeSliderHovered", "{}", h),
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
            Self::UpdateLyricsFontFamily(f) => {
                simple!("UpdateLyricsFontFamily", "{:?}", f)
            }
            Self::UpdateMaxCacheMb(m) => simple!("UpdateMaxCacheMb", "{}", m),
            Self::UpdateDownloadQuality(q) => simple!("UpdateDownloadQuality", "{:?}", q),
            Self::UpdateDownloadDir(d) => simple!("UpdateDownloadDir", "{:?}", d),
            Self::UpdateDownloadDirDialog => simple!("UpdateDownloadDirDialog"),
            Self::ClearCache => simple!("ClearCache"),
            Self::CacheCleared(n, b) => simple!("CacheCleared", "{} files, {} bytes", n, b),
            Self::RefreshCacheStats => simple!("RefreshCacheStats"),
            Self::EnforceCacheLimit => simple!("EnforceCacheLimit"),
            Self::UpdateAudioOutputDevice(_) => simple!("UpdateAudioOutputDevice"),
            Self::UpdateDiscordEnabled(b) => simple!("UpdateDiscordEnabled", "{}", b),
            Self::UpdateProxyType(t) => simple!("UpdateProxyType", "{:?}", t),
            Self::UpdateProxyHost(_) => simple!("UpdateProxyHost"),
            Self::UpdateProxyPort(_) => simple!("UpdateProxyPort"),
            Self::UpdateProxyUsername(_) => simple!("UpdateProxyUsername"),
            Self::UpdateProxyPassword(_) => simple!("UpdateProxyPassword"),
            Self::ApplyProxySettings => simple!("ApplyProxySettings"),
            Self::ScrollToSection(s) => simple!("ScrollToSection", "{:?}", s),
            Self::SettingsScrolled(y) => simple!("SettingsScrolled", "{:.0}", y),
            Self::MeasureSectionPositions => simple!("MeasureSectionPositions"),
            Self::SectionPositionsMeasured(p) => {
                simple!("SectionPositionsMeasured", "{} positions", p.len())
            }
            Self::StartEditingKeybinding(a) => simple!("StartEditingKeybinding", "{:?}", a),
            Self::CancelEditingKeybinding => simple!("CancelEditingKeybinding"),
            Self::KeybindingKeyPressed(_, _) => simple!("KeybindingKeyPressed"),

            // Database
            Self::DatabaseError(e) => simple!("DatabaseError", "{}", e),
            Self::SongsValidated(n) => simple!("SongsValidated", "{}", n),

            // Import
            Self::StartScan(_) => simple!("StartScan"),
            Self::CancelScan => simple!("CancelScan"),
            Self::ImportedPlaylistCreated(result) => {
                simple!("ImportedPlaylistCreated", "success={}", result.is_ok())
            }
            Self::WatcherEvent(_) => simple!("WatcherEvent"),
            Self::WatchedFolderSyncCompleted(id) => {
                simple!("WatchedFolderSyncCompleted", "{:?}", id)
            }
            Self::ShowInfoToast(_) => simple!("ShowInfoToast"),
            Self::ShowSuccessToast(_) => simple!("ShowSuccessToast"),
            Self::ShowWarningToast(_) => simple!("ShowWarningToast"),
            Self::ShowErrorToast(_) => simple!("ShowErrorToast"),
            Self::HideToast => simple!("HideToast"),
            Self::ClearImportingPlaylist => simple!("ClearImportingPlaylist"),

            // Playlist page
            Self::OpenPlaylist(id) => simple!("OpenPlaylist", "{}", id),
            Self::RequestDeletePlaylist(id) => simple!("RequestDeletePlaylist", "{}", id),
            Self::ConfirmDeletePlaylist => simple!("ConfirmDeletePlaylist"),
            Self::PlaylistDeleted(id) => simple!("PlaylistDeleted", "{}", id),
            Self::PlaySong(id) => simple!("PlaySong", "{}", id),
            Self::HoverSong(id) => simple!("HoverSong", "{:?}", id),
            Self::HoverIcon(id) => simple!("HoverIcon", "{:?}", id),
            Self::HoverSidebar(id) => simple!("HoverSidebar", "{:?}", id),
            Self::TogglePlaylistSearch => simple!("TogglePlaylistSearch"),
            Self::PlaylistSearchChanged(_) => simple!("PlaylistSearchChanged"),
            Self::PlaylistSearchSubmit => simple!("PlaylistSearchSubmit"),
            Self::PlaylistSearchBlur => simple!("PlaylistSearchBlur"),
            Self::ToggleDescriptionExpand => simple!("ToggleDescriptionExpand"),

            // Edit dialog
            Self::EditPlaylist(id) => simple!("EditPlaylist", "{}", id),
            Self::EditPlaylistNameChanged(_) => simple!("EditPlaylistNameChanged"),
            Self::EditPlaylistDescriptionChanged(_) => simple!("EditPlaylistDescriptionChanged"),
            Self::EditPlaylistWatchEnabledChanged(enabled) => {
                simple!("EditPlaylistWatchEnabledChanged", "{}", enabled)
            }
            Self::PickCoverImage => simple!("PickCoverImage"),
            Self::CoverImagePicked(_) => simple!("CoverImagePicked"),
            Self::SavePlaylistEdits => simple!("SavePlaylistEdits"),
            Self::PlaylistUpdated(id) => simple!("PlaylistUpdated", "{}", id),

            // Lyrics
            Self::OpenLyricsPage => simple!("OpenLyricsPage"),
            Self::CloseLyricsPage => simple!("CloseLyricsPage"),
            Self::LyricsScroll(d) => simple!("LyricsScroll", "{:.1}", d),
            Self::LyricsViewportResized(size) => {
                simple!("LyricsViewportResized", "{}x{}", size.width, size.height)
            }
            Self::WindowResized(size) => simple!("WindowResized", "{}x{}", size.width, size.height),
            Self::LyricsFontSystemReady(_) => simple!("LyricsFontSystemReady"),
            Self::LyricsLoaded(id, lines) => {
                simple!("LyricsLoaded", "id={}, {} lines", id, lines.len())
            }
            Self::LyricsLoadFailed(id, _) => simple!("LyricsLoadFailed", "id={}", id),
            Self::FetchLyricsOnline(id, _) => simple!("FetchLyricsOnline", "id={}", id),
            Self::WarmLyricsCache(id, _) => simple!("WarmLyricsCache", "id={}", id),
            Self::LyricsWarmupFinished(id, result) => simple!(
                "LyricsWarmupFinished",
                "id={}, status={}",
                id,
                if result.is_ok() { "ok" } else { "err" }
            ),
            Self::LocalLyricsReady(id, lines) => {
                simple!("LocalLyricsReady", "id={}, {} lines", id, lines.len())
            }
            Self::LyricsEngineLinesReady(id, lines) => {
                simple!("LyricsEngineLinesReady", "id={}, {} lines", id, lines.len())
            }
            Self::LyricsShapedLinesReady(id, generation, lines, bitmaps, _, _, _) => simple!(
                "LyricsShapedLinesReady",
                "id={}, gen={}, {} lines, {} bitmaps",
                id,
                generation,
                lines.len(),
                bitmaps.len()
            ),
            Self::LyricsBackgroundReady(id, _, _, _, _) => {
                simple!("LyricsBackgroundReady", "id={}", id)
            }
            Self::LyricsCoverImageReady(id, _, _, w, h) => {
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
            Self::PreloadReady(idx, _, direction) => {
                simple!("PreloadReady", "idx={}, direction={}", idx, direction)
            }
            Self::PreloadBufferReady(idx, _, direction, buffer, duration) => {
                simple!(
                    "PreloadBufferReady",
                    "idx={}, direction={}, downloaded={}, duration={}s",
                    idx,
                    direction,
                    buffer.downloaded(),
                    duration
                )
            }
            Self::PreloadAudioFailed(idx, direction) => {
                simple!("PreloadAudioFailed", "idx={}, direction={}", idx, direction)
            }

            // Queue management
            Self::PlayPlaylist(id) => simple!("PlayPlaylist", "{}", id),
            Self::PlayQueueIndex(i) => simple!("PlayQueueIndex", "{}", i),
            Self::SongResolvedStreaming(i, _, _, buffer, _) => {
                simple!(
                    "SongResolvedStreaming",
                    "idx={}, buffer={}",
                    i,
                    buffer.is_some()
                )
            }
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
            Self::ExitDialogRememberChanged(b) => simple!("ExitDialogRememberChanged", "{}", b),

            // Tray
            Self::TrayCommand(c) => simple!("TrayCommand", "{:?}", c),

            // Media Controls
            Self::MprisCommand(c) => simple!("MprisCommand", "{:?}", c),
            Self::MprisStartedWithHandle(_, _) => simple!("MprisStartedWithHandle"),
            Self::ShowWindow => simple!("ShowWindow"),
            Self::ShowOrFocusWindow => simple!("ShowOrFocusWindow"),
            Self::ToggleWindow => simple!("ToggleWindow"),
            Self::WindowShown => simple!("WindowShown"),
            Self::WindowFocused => simple!("WindowFocused"),
            Self::WindowUnfocused => simple!("WindowUnfocused"),
            Self::WindowOperationComplete => simple!("WindowOperationComplete"),

            // NCM Login
            Self::TryAutoLogin(retry) => simple!("TryAutoLogin", "retry={}", retry),
            Self::RequestQrCode => simple!("RequestQrCode"),
            Self::QrCodeReady(_, _) => simple!("QrCodeReady"),
            Self::CheckQrStatus(_) => simple!("CheckQrStatus"),
            Self::QrLoginResult(s) => simple!("QrLoginResult", "{:?}", s),
            Self::Logout => simple!("Logout"),
            Self::ToggleLoginPopup => simple!("ToggleLoginPopup"),

            // NCM Homepage
            Self::BannerPlay(i) => simple!("BannerPlay", "{}", i),
            Self::CarouselNavigate(d) => simple!("CarouselNavigate", "{}", d),
            Self::OpenTrendingSongs => simple!("OpenTrendingSongs"),
            Self::ToggleFavorite(id) => simple!("ToggleFavorite", "{}", id),
            Self::FavoriteStatusChanged(id, s) => simple!("FavoriteStatusChanged", "{}, {}", id, s),
            Self::OpenNcmPlaylist(id) => simple!("OpenNcmPlaylist", "{}", id),
            Self::OpenUser(id) => simple!("OpenUser", "{}", id),
            Self::OpenArtist(id) => simple!("OpenArtist", "{}", id),
            Self::OpenAlbum(id) => simple!("OpenAlbum", "{}", id),
            Self::OpenArtistByName(name) => simple!("OpenArtistByName", "{}", name),
            Self::ToggleBannerFavorite(i) => simple!("ToggleBannerFavorite", "{}", i),

            // Cloud Playlist
            Self::ImageDownloadReady(kind, id, _path) => {
                simple!("ImageDownloadReady", "{:?}, {}", kind, id)
            }
            Self::ImageDownloadFailed(kind, id) => {
                simple!("ImageDownloadFailed", "{:?}, {}", kind, id)
            }
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
            Self::HoverDiscoverPlaylist(id) => simple!("HoverDiscoverPlaylist", "{:?}", id),
            Self::PlayDiscoverPlaylist(id) => simple!("PlayDiscoverPlaylist", "{}", id),
            Self::LoadMoreHotPlaylists => simple!("LoadMoreHotPlaylists"),
            Self::SeeAllRecommended => simple!("SeeAllRecommended"),
            Self::SeeAllHot => simple!("SeeAllHot"),

            // Search Page
            Self::SearchSubmit => simple!("SearchSubmit"),
            Self::SearchTabChanged(tab) => simple!("SearchTabChanged", "{:?}", tab),
            Self::SearchResultsLoaded(payload) => {
                simple!(
                    "SearchResultsLoaded",
                    "tab={:?}, tracks={}, artists={}, albums={}, playlists={}",
                    payload.tab,
                    payload.tracks.len(),
                    payload.artists.len(),
                    payload.albums.len(),
                    payload.playlists.len()
                )
            }
            Self::SearchFailed(e) => simple!("SearchFailed", "{}", e),
            Self::SearchPageChanged(page) => simple!("SearchPageChanged", "{}", page),
            Self::HoverSearchSong(id) => simple!("HoverSearchSong", "{:?}", id),
            Self::HoverSearchCard(id) => simple!("HoverSearchCard", "{:?}", id),
            Self::ContentWidthResized(target, size) => {
                simple!(
                    "ContentWidthResized",
                    "target={:?}, {}x{}",
                    target,
                    size.width,
                    size.height
                )
            }
            Self::PlaySearchSong(id) => simple!("PlaySearchSong", "id={}", id),
            Self::OpenSearchResult(id, tab) => {
                simple!("OpenSearchResult", "id={}, tab={:?}", id, tab)
            }
            Self::SwitchArtistTab(tab) => simple!("SwitchArtistTab", "{:?}", tab),

            // Sidebar resize
            Self::SidebarResizeStart => simple!("SidebarResizeStart"),
            Self::SidebarResizeEnd => simple!("SidebarResizeEnd"),

            // Streaming
            Self::StreamingEvent(id, _) => simple!("StreamingEvent", "id={}", id),

            // Audio events
            Self::AudioEvent(event) => simple!("AudioEvent", "{:?}", event),

            // Discord
            Self::DiscordUpdatePresence => simple!("DiscordUpdatePresence"),
            Self::DiscordClearPresence => simple!("DiscordClearPresence"),

            // Protocol
            Self::UriReceived(uri) => simple!("UriReceived", "{}", uri),

            // Context Menu
            Self::RightClickSong(id) => simple!("RightClickSong", "{}", id),
            Self::RightClickNcmSong(info) => simple!("RightClickNcmSong", "{}", info.id),
            Self::CloseContextMenu => simple!("CloseContextMenu"),
            Self::ContextMenuAction(action, id) => {
                simple!("ContextMenuAction", "{:?}, {}", action, id)
            }
            Self::PlaylistPickerConfirm(song_id, playlist_id) => {
                simple!(
                    "PlaylistPickerConfirm",
                    "song={}, playlist={}",
                    song_id,
                    playlist_id
                )
            }
            Self::AddToNcmPlaylist(song_id, playlist_id) => {
                simple!(
                    "AddToNcmPlaylist",
                    "song={}, playlist={}",
                    song_id,
                    playlist_id
                )
            }
            Self::NcmPlaylistAddResult(song_id, playlist_id, result) => {
                simple!(
                    "NcmPlaylistAddResult",
                    "song={}, playlist={}, ok={}",
                    song_id,
                    playlist_id,
                    result.is_ok()
                )
            }

            // Overlay System
            Self::DismissTopModal => simple!("DismissTopModal"),

            Self::EditSongTags(id) => simple!("EditSongTags", "{}", id),
            Self::OpenSongEditDialog(_) => simple!("OpenSongEditDialog"),
            Self::SongEditFieldChanged {
                song_id,
                field,
                value: _,
            } => simple!("SongEditFieldChanged", "{}, {}", song_id, field),
            Self::SongEditCoverReplaced(id, _) => simple!("SongEditCoverReplaced", "{}", id),
            Self::PickSongEditCover(id) => simple!("PickSongEditCover", "{}", id),
            Self::SaveSongEdits(id) => simple!("SaveSongEdits", "{}", id),
            Self::SongEditsSaved(id) => simple!("SongEditsSaved", "{}", id),
            Self::SongEditsFailed { song_id, error: _ } => {
                simple!("SongEditsFailed", "{}", song_id)
            }

            // Download
            Self::DownloadSong(id) => simple!("DownloadSong", "{}", id),
            Self::DownloadUrlResolved(sid, nid, ..) => {
                simple!("DownloadUrlResolved", "{}, ncm={}", sid, nid)
            }
            Self::DownloadPlaylist(id) => simple!("DownloadPlaylist", "{}", id),
            Self::DownloadCancel(id) => simple!("DownloadCancel", "{}", id),
            Self::DownloadProgress(sid, dl, total) => {
                simple!("DownloadProgress", "{}, {}/{}", sid, dl, total)
            }
            Self::DownloadCompleted(sid, path) => {
                simple!("DownloadCompleted", "{}, {}", sid, path)
            }
            Self::DownloadError(sid, e) => simple!("DownloadError", "{}, {}", sid, e),
            Self::DeleteDownloadHistory(sid) => simple!("DeleteDownloadHistory", "{}", sid),
            Self::SwitchDownloadTab(_) => simple!("SwitchDownloadTab"),
            Self::DownloadBatchEnqueue(items) => {
                simple!("DownloadBatchEnqueue", "{} items", items.len())
            }
            Self::RequestDownloadPlaylist(id, name, count) => {
                simple!("RequestDownloadPlaylist", "{}, {}, {}", id, name, count)
            }
            Self::ConfirmDownloadPlaylist => simple!("ConfirmDownloadPlaylist"),
            Self::DownloadsLoaded(v) => simple!("DownloadsLoaded", "{} downloads", v.len()),
        }
    }
}
