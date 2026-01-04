// src/app/state.rs
//! Application state definitions

use iced::time::Instant;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use crate::api::{BannersInfo, NcmClient, SongInfo, SongList, TopList};
use crate::app::SettingsSection;
use crate::audio::AudioProcessingChain;
use crate::database::{Database, DbPlaybackState, DbPlaylist, DbSong};
use crate::platform::media_controls::{MediaCommand, MediaHandle};
use crate::features::import::{CoverCache, FolderWatcher, ScanHandle, ScanProgress, ScanState};
use crate::i18n::Locale;
use crate::ui::animation::{HoverAnimations, SingleHoverAnimation};
use crate::ui::components::{ImportingPlaylist, NavItem};
use crate::ui::effects::background::LyricsBackgroundProgram;
use crate::ui::effects::textured_background::TexturedBackgroundProgram;
use crate::ui::pages;
use crate::ui::widgets::Toast;

/// Main application state
pub struct App {
    /// Core infrastructure (Settings, DB, Audio, System integrations)
    pub core: CoreState,
    /// Business data (Songs, Playlists, Queue)
    pub library: LibraryState,
    /// UI state (Navigation, Page states, Animations)
    pub ui: UiState,
}

/// Core Infrastructure & Services
pub struct CoreState {
    pub db: Option<Arc<Database>>,
    pub db_error: Option<String>,
    pub audio: Option<crate::audio::AudioPlayer>,
    /// Audio processing chain (preamp, EQ, analyzer) - shared with AudioPlayer
    pub audio_chain: AudioProcessingChain,
    pub volume_before_mute: Option<f32>,
    pub settings: crate::features::Settings,
    pub locale: Locale,
    pub is_logged_in: bool,

    // NCM API Client
    pub ncm_client: Option<NcmClient>,
    pub user_info: Option<UserInfo>,

    // System Integrations
    pub cover_cache: Option<Arc<CoverCache>>,
    pub mpris_handle: Option<MediaHandle>,
    pub mpris_rx: Option<
        Arc<
            tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<MediaCommand>>,
        >,
    >,
    pub window_hidden: bool,
    pub window_operation_pending: bool,
    pub is_fullscreen: bool,
    /// Current mouse Y position for drag area detection
    pub mouse_position: iced::Point,
}

impl CoreState {
    /// Initialize core services with loaded settings
    pub fn new(settings: crate::features::Settings, locale: Locale) -> Self {
        // Create shared audio processing chain first
        let audio_chain = AudioProcessingChain::new();

        // Apply settings to the chain
        audio_chain.set_equalizer_enabled(settings.playback.equalizer_enabled);
        audio_chain.set_equalizer_gains(settings.playback.equalizer_values);
        audio_chain.set_preamp(settings.playback.equalizer_preamp);

        // Create audio player with configured device and shared chain
        let audio = Self::create_audio_player(&settings, audio_chain.clone());

        Self {
            db: None,
            db_error: None,
            audio,
            audio_chain,
            volume_before_mute: None,
            settings,
            locale,
            is_logged_in: false,
            ncm_client: None,
            user_info: None,
            cover_cache: None,
            mpris_handle: None,
            mpris_rx: None,
            window_hidden: false,
            window_operation_pending: false,
            is_fullscreen: false,
            mouse_position: iced::Point::ORIGIN,
        }
    }

    /// Create audio player with settings applied
    fn create_audio_player(
        settings: &crate::features::Settings,
        audio_chain: AudioProcessingChain,
    ) -> Option<crate::audio::AudioPlayer> {
        // Try to create player with configured device, fallback to default
        let player = if let Some(ref device_name) = settings.system.audio_output_device {
            match crate::audio::AudioPlayer::with_device(Some(device_name), audio_chain.clone()) {
                Ok(p) => {
                    tracing::info!("Audio player created with device: {}", device_name);
                    p
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to use configured device '{}': {}, falling back to default",
                        device_name,
                        e
                    );
                    match crate::audio::AudioPlayer::new(audio_chain) {
                        Ok(p) => p,
                        Err(e) => {
                            tracing::error!("Failed to create audio player: {}", e);
                            return None;
                        }
                    }
                }
            }
        } else {
            match crate::audio::AudioPlayer::new(audio_chain) {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!("Failed to create audio player: {}", e);
                    return None;
                }
            }
        };

        Some(player)
    }
}

/// User information from NCM
#[derive(Debug, Clone)]
pub struct UserInfo {
    pub user_id: u64,
    pub nickname: String,
    pub avatar_url: String,
    pub avatar_path: Option<PathBuf>,
    /// Pre-loaded avatar image handle for instant rendering
    pub avatar_handle: Option<iced::widget::image::Handle>,
    pub vip_type: i32,
    pub like_songs: HashSet<u64>,
}

impl UserInfo {
    pub fn new(uid: u64, nickname: String, avatar_url: String) -> Self {
        Self {
            user_id: uid,
            nickname,
            avatar_url,
            avatar_path: None,
            avatar_handle: None,
            vip_type: 0,
            like_songs: HashSet::new(),
        }
    }
}

/// Business Logic Data
pub struct LibraryState {
    pub db_songs: Vec<DbSong>,
    pub playlists: Vec<DbPlaylist>,
    pub recently_played: Vec<DbSong>,

    // Playback Data
    pub current_song: Option<DbSong>,
    pub playback_state: Option<DbPlaybackState>,
    pub queue: Vec<DbSong>,
    pub queue_index: Option<usize>,

    // Queue navigation - Single Source of Truth for index calculations
    pub shuffle_cache: crate::app::update::queue_navigator::ShuffleCache,

    // Preload state machine
    pub preload_manager: crate::app::update::preload_manager::PreloadManager,

    // Track which song is currently being resolved for playback
    // Only the resolution result matching this index should trigger playback
    pub pending_resolution_idx: Option<usize>,

    // Import State
    pub scan_state: Option<Arc<ScanState>>,
    pub scan_handle: Option<ScanHandle>,
    pub scan_progress: Option<ScanProgress>,
    pub folder_watcher: Option<FolderWatcher>,
    pub watched_folders: Vec<PathBuf>,
}

impl Default for LibraryState {
    fn default() -> Self {
        Self {
            db_songs: Vec::new(),
            playlists: Vec::new(),
            recently_played: Vec::new(),
            current_song: None,
            playback_state: None,
            queue: Vec::new(),
            queue_index: None,
            shuffle_cache: Default::default(),
            preload_manager: Default::default(),
            pending_resolution_idx: None,
            scan_state: None,
            scan_handle: None,
            scan_progress: None,
            folder_watcher: None,
            watched_folders: Vec::new(),
        }
    }
}

/// Navigation history entry
#[derive(Debug, Clone, PartialEq)]
pub enum NavigationEntry {
    /// Navigation page (Home, Discover, Radio, Settings, AudioEngine)
    Nav(NavItem),
    /// Playlist page with playlist ID
    Playlist(i64),
    /// NCM cloud playlist with playlist ID
    NcmPlaylist(u64),
    /// Recently played
    RecentlyPlayed,
}

/// Navigation history for back/forward functionality
#[derive(Debug, Default)]
pub struct NavigationHistory {
    /// History stack
    pub entries: Vec<NavigationEntry>,
    /// Current position in history (index)
    pub current_index: Option<usize>,
}

impl NavigationHistory {
    /// Push a new entry to history, clearing forward history
    pub fn push(&mut self, entry: NavigationEntry) {
        // Don't push if it's the same as current
        if let Some(idx) = self.current_index {
            if idx < self.entries.len() && self.entries[idx] == entry {
                return;
            }
            // Clear forward history
            self.entries.truncate(idx + 1);
        }
        self.entries.push(entry);
        self.current_index = Some(self.entries.len() - 1);
    }

    /// Go back in history, returns the entry to navigate to
    pub fn go_back(&mut self) -> Option<NavigationEntry> {
        if let Some(idx) = self.current_index {
            if idx > 0 {
                self.current_index = Some(idx - 1);
                return self.entries.get(idx - 1).cloned();
            }
        }
        None
    }

    /// Go forward in history, returns the entry to navigate to
    pub fn go_forward(&mut self) -> Option<NavigationEntry> {
        if let Some(idx) = self.current_index {
            if idx + 1 < self.entries.len() {
                self.current_index = Some(idx + 1);
                return self.entries.get(idx + 1).cloned();
            }
        }
        None
    }

    /// Check if can go back
    pub fn can_go_back(&self) -> bool {
        self.current_index.map(|idx| idx > 0).unwrap_or(false)
    }

    /// Check if can go forward
    pub fn can_go_forward(&self) -> bool {
        self.current_index
            .map(|idx| idx + 1 < self.entries.len())
            .unwrap_or(false)
    }
}

/// UI View State
pub struct UiState {
    pub active_nav: NavItem,
    pub search_query: String,
    pub toast: Option<Toast>,
    pub toast_visible: bool,

    /// Navigation history for back/forward
    pub nav_history: NavigationHistory,

    // Sub-modules
    pub playlist_page: PlaylistPageState,
    pub lyrics: LyricsState,
    pub dialogs: DialogState,
    pub home: HomePageState,
    pub discover: DiscoverPageState,

    // Global UI Layout
    pub active_settings_section: SettingsSection,
    pub editing_keybinding: Option<crate::features::Action>,
    pub queue_visible: bool,

    // Playback Controls UI
    pub is_seeking: bool,
    pub seek_preview_position: f32,
    pub save_position_counter: u32,

    // Sidebar
    pub importing_playlist: Option<ImportingPlaylist>,
    pub sidebar_animations: HoverAnimations<crate::app::message::SidebarId>,
    /// Sidebar width in pixels (draggable)
    pub sidebar_width: f32,
    /// Whether the sidebar resize handle is being dragged
    pub sidebar_dragging: bool,

    // Cache statistics
    pub cache_stats: Option<crate::cache::CacheStats>,
}

impl UiState {
    pub fn new() -> Self {
        Self {
            active_nav: NavItem::Home,
            search_query: String::new(),
            toast: None,
            toast_visible: false,
            nav_history: {
                let mut history = NavigationHistory::default();
                history.push(NavigationEntry::Nav(NavItem::Home));
                history
            },
            active_settings_section: SettingsSection::Account,
            editing_keybinding: None,
            queue_visible: false,
            is_seeking: false,
            seek_preview_position: 0.0,
            save_position_counter: 0,
            importing_playlist: None,
            sidebar_animations: Default::default(),
            sidebar_width: 240.0,
            sidebar_dragging: false,
            cache_stats: None,

            playlist_page: PlaylistPageState {
                current: None,
                viewing_recently_played: false,
                song_animations: Default::default(),
                icon_animations: Default::default(),
                search_expanded: false,
                search_query: String::new(),
                search_animation: Default::default(),
                scroll_state: std::rc::Rc::new(std::cell::RefCell::new(
                    crate::ui::widgets::VirtualListState::default(),
                )),
                pending_cover_downloads: HashSet::new(),
                load_state: Default::default(),
            },

            lyrics: LyricsState {
                is_open: false,
                animation: Default::default(),
                lines: Vec::new(),
                current_line_idx: None,
                last_update: None,
                bg_colors: crate::utils::DominantColors::dark_default(),
                // Initialized directly as requested
                bg_shader: LyricsBackgroundProgram::new(),
                textured_bg_shader: TexturedBackgroundProgram::new(),
                // Engine will be created lazily when FontSystem is ready
                // This avoids blocking app startup with FontSystem::new()
                engine: None,
                shader_start_time: None,
                cached_engine_lines: None,
                cached_shaped_lines: None,
                // FontSystem will be created asynchronously
                shared_font_system: None,
                user_scrolling: false,
                last_scroll_time: None,
                manual_scroll_offset: 0.0,
                viewport_width: 800.0,  // Default, will be updated from view
                viewport_height: 600.0, // Default, will be updated from view
                loading_song_id: None,
                is_loading: false,
                load_error: None,
            },

            dialogs: DialogState {
                import_open: false,
                edit_open: false,
                editing_playlist_id: None,
                edit_name: String::new(),
                edit_description: String::new(),
                edit_cover: None,
                edit_animation: Default::default(),
                delete_pending_id: None,
                delete_animation: Default::default(),
                exit_open: false,
                exit_animation: Default::default(),
                exit_remember: false,
            },

            home: HomePageState {
                banners: Vec::new(),
                banner_images: std::collections::HashMap::new(),
                current_banner: 0,
                top_picks: Vec::new(),
                toplists: Vec::new(),
                trending_songs: Vec::new(),
                song_covers: std::collections::HashMap::new(),
                login_popup_open: false,
                qr_code_path: None,
                qr_unikey: None,
                qr_status: None,
                cloud_songs: Vec::new(),
                user_playlists: Vec::new(),
                current_ncm_playlist_songs: Vec::new(),
                song_hover_animations: Default::default(),
                last_banner: 0,
                carousel_animation: iced::animation::Animation::new(false),
                carousel_direction: 1,
            },

            discover: DiscoverPageState::default(),
        }
    }

    /// Check if any global or submodule animation is currently active
    /// Optimized: O(1) check for hover animations, only checks active/fading states
    pub fn has_active_animations(&self, now: Instant) -> bool {
        // Hover animations are now O(1) - they only track active + fading
        self.sidebar_animations.is_animating(now)
            || self.playlist_page.song_animations.is_animating(now)
            || self.playlist_page.icon_animations.is_animating(now)
            || self.playlist_page.search_animation.is_animating(now)
            || self.lyrics.animation.is_animating(now)
            || self.dialogs.edit_animation.is_animating(now)
            || self.dialogs.exit_animation.is_animating(now)
            || self.dialogs.delete_animation.is_animating(now)
            || self.home.carousel_animation.is_animating(now)
            || self.home.song_hover_animations.is_animating(now)
            || self.discover.card_animations.is_animating(now)
    }

    /// Clean up completed animations to prevent memory leaks
    /// Call this periodically (e.g., on AnimationTick)
    pub fn cleanup_animations(&mut self, now: Instant) {
        self.sidebar_animations.cleanup_completed(now);
        self.playlist_page.song_animations.cleanup_completed(now);
        self.playlist_page.icon_animations.cleanup_completed(now);
        self.home.song_hover_animations.cleanup_completed(now);
        self.discover.card_animations.cleanup_completed(now);
    }

    /// Clear all playlist-related animations when navigating away
    pub fn clear_playlist_animations(&mut self) {
        self.playlist_page.song_animations.clear();
        self.playlist_page.icon_animations.clear();
    }
}

pub struct PlaylistPageState {
    pub current: Option<pages::PlaylistView>,
    pub viewing_recently_played: bool,
    pub song_animations: HoverAnimations<i64>,
    pub icon_animations: HoverAnimations<crate::app::message::IconId>,
    pub search_expanded: bool,
    pub search_query: String,
    pub search_animation: SingleHoverAnimation,
    /// Virtual list scroll state for efficient rendering
    pub scroll_state: std::rc::Rc<std::cell::RefCell<crate::ui::widgets::VirtualListState>>,
    /// Song IDs currently being downloaded (to avoid duplicate requests)
    pub pending_cover_downloads: HashSet<i64>,
    /// Loading state for async playlist loading
    pub load_state: crate::app::update::page_loader::PlaylistLoadState,
}

pub struct LyricsState {
    pub is_open: bool,
    pub animation: SingleHoverAnimation,
    pub lines: Vec<crate::ui::pages::LyricLine>,
    pub current_line_idx: Option<usize>,
    pub last_update: Option<Instant>,

    // Visuals & Shaders
    pub bg_colors: crate::utils::DominantColors,
    pub bg_shader: LyricsBackgroundProgram,
    pub textured_bg_shader: TexturedBackgroundProgram,
    /// 歌词引擎 (RefCell 用于 view() 中的内部可变性)
    pub engine: Option<std::cell::RefCell<crate::features::lyrics::engine::LyricsEngine>>,
    pub shader_start_time: Option<Instant>,
    /// Cached engine lines to avoid recreating every frame
    /// Using Arc for O(1) clone in view function (thread-safe for iced Primitive)
    pub cached_engine_lines:
        Option<std::sync::Arc<Vec<crate::features::lyrics::engine::LyricLineData>>>,
    /// Cached shaped lines (pre-computed in background thread)
    /// 文本布局的唯一数据源
    pub cached_shaped_lines:
        Option<std::sync::Arc<Vec<crate::features::lyrics::engine::CachedShapedLine>>>,
    /// Shared font system for async text shaping (created asynchronously at app startup)
    pub shared_font_system: Option<crate::features::lyrics::engine::SharedFontSystem>,

    // Scrolling
    pub user_scrolling: bool,
    pub last_scroll_time: Option<Instant>,
    pub manual_scroll_offset: f32,

    // Viewport info for line height calculations
    /// Last known viewport width (in logical pixels)
    pub viewport_width: f32,
    /// Last known viewport height (in logical pixels)
    pub viewport_height: f32,

    // Online lyrics loading
    /// Song ID currently loading lyrics for (to avoid duplicate requests)
    pub loading_song_id: Option<i64>,
    /// Whether lyrics are currently being loaded
    pub is_loading: bool,
    /// Error message if lyrics loading failed
    pub load_error: Option<String>,
}

pub struct DialogState {
    pub import_open: bool,

    // Edit
    pub edit_open: bool,
    pub editing_playlist_id: Option<i64>,
    pub edit_name: String,
    pub edit_description: String,
    pub edit_cover: Option<String>,
    pub edit_animation: SingleHoverAnimation,

    // Delete
    pub delete_pending_id: Option<i64>,
    pub delete_animation: SingleHoverAnimation,

    // Exit
    pub exit_open: bool,
    pub exit_animation: SingleHoverAnimation,
    pub exit_remember: bool,
}

/// Discover page view mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiscoverViewMode {
    /// Default view showing both sections with limited items
    #[default]
    Overview,
    /// Full view of recommended playlists
    AllRecommended,
    /// Full view of hot playlists with infinite scroll
    AllHot,
}

/// Discover page state for browsing playlists
pub struct DiscoverPageState {
    /// Current view mode
    pub view_mode: DiscoverViewMode,
    /// Recommended playlists (for logged-in users)
    pub recommended_playlists: Vec<SongList>,
    /// Hot playlists (for all users)
    pub hot_playlists: Vec<SongList>,
    /// Cover image handle cache: playlist_id -> image::Handle
    /// Using Handle instead of PathBuf for instant rendering (no disk IO in render loop)
    pub playlist_covers: std::collections::HashMap<u64, iced::widget::image::Handle>,
    /// GPU allocations to keep covers in GPU memory even when not rendered
    /// This prevents re-loading from disk when returning to the discover page
    pub playlist_cover_allocations: std::collections::HashMap<u64, iced::widget::image::Allocation>,
    /// Hover animations for playlist cards
    pub card_animations: HoverAnimations<u64>,
    /// Loading state for recommended playlists
    pub recommended_loading: bool,
    /// Loading state for hot playlists
    pub hot_loading: bool,
    /// Pagination offset for hot playlists
    pub hot_offset: u16,
    /// Whether more hot playlists are available
    pub hot_has_more: bool,
    /// Whether data has been loaded (to avoid re-fetching)
    pub data_loaded: bool,
    /// Content area width for dynamic grid column calculation
    pub content_width: f32,
}

impl Default for DiscoverPageState {
    fn default() -> Self {
        Self {
            view_mode: DiscoverViewMode::default(),
            recommended_playlists: Vec::new(),
            hot_playlists: Vec::new(),
            playlist_covers: std::collections::HashMap::new(),
            playlist_cover_allocations: std::collections::HashMap::new(),
            card_animations: Default::default(),
            recommended_loading: false,
            hot_loading: false,
            hot_offset: 0,
            hot_has_more: true,
            data_loaded: false,
            // Default width, will be updated from WindowResized
            // Assumes window width ~1280, sidebar 240, padding 64
            content_width: 976.0,
        }
    }
}

/// Homepage state for NCM data
pub struct HomePageState {
    // Carousel banners
    pub banners: Vec<BannersInfo>,
    /// Banner images for Canvas rendering: index -> (PathBuf, width, height)
    /// Canvas requires PathBuf, iced handles its own caching internally
    pub banner_images: std::collections::HashMap<usize, (PathBuf, u32, u32)>,
    pub current_banner: usize,

    // Content sections
    pub top_picks: Vec<SongList>,
    pub toplists: Vec<TopList>,
    pub trending_songs: Vec<SongInfo>,
    /// Song cover handles cache: song_id -> Handle
    /// Using Handle instead of PathBuf for instant rendering (no disk IO in render loop)
    pub song_covers: std::collections::HashMap<u64, iced::widget::image::Handle>,

    // Login popup
    pub login_popup_open: bool,
    pub qr_code_path: Option<PathBuf>,
    pub qr_unikey: Option<String>,
    pub qr_status: Option<String>,

    // Cloud playlist
    pub cloud_songs: Vec<SongInfo>,
    pub user_playlists: Vec<SongList>,
    /// Current NCM playlist songs (for playback)
    pub current_ncm_playlist_songs: Vec<SongInfo>,

    // Hover animations for song list
    pub song_hover_animations: HoverAnimations<u64>,

    // Carousel animation
    pub last_banner: usize,
    pub carousel_animation: iced::animation::Animation<bool>,
    pub carousel_direction: i32,
}
