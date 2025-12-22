//! Internationalization (i18n) support for Rustle
//! Supports multiple languages with easy extensibility
//!
//! Structure:
//! - mod.rs: Core types (Language, Key, Locale) and translation lookup
//! - en.rs: English translations
//! - zh.rs: Chinese translations

mod en;
mod zh;

use std::collections::HashMap;

/// Supported languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Language {
    #[default]
    English,
    Chinese,
}

impl Language {
    /// Get language display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Chinese => "中文",
        }
    }

    /// Get language code
    pub fn code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Chinese => "zh",
        }
    }

    /// All available languages
    pub fn all() -> &'static [Language] {
        &[Language::English, Language::Chinese]
    }
}

/// Translation keys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    // App
    AppName,

    // Navigation
    NavHome,
    NavDiscover,
    NavRadio,
    NavSettings,
    NavAudioEngine,

    // Library - Local
    LibraryTitle,
    LibraryRecentlyPlayed,
    ImportLocalPlaylist,
    // Library - Cloud
    CloudPlaylistsTitle,
    CloudPlaylistsNotLoggedIn,

    // User
    GuestUser,
    NotLoggedIn,
    ClickToLogin,
    FreeAccount,

    // Search
    SearchPlaceholder,

    // Hero Banner
    HeroTitle,
    HeroSubtitle,
    PlayButton,

    // Trending
    TrendingSongs,
    SeeAll,

    // Recently Played
    RecentlyPlayed,
    RecentlyPlayedDescription,
    RecentlyPlayedList,

    // Window Controls
    Minimize,
    Maximize,
    Close,
    Settings,

    // Navigation Controls
    Back,
    Forward,

    // Settings Page - Tabs
    SettingsTitle,
    SettingsTabPlayback,
    SettingsTabDisplay,
    SettingsTabSystem,
    SettingsTabStorage,
    SettingsTabShortcuts,
    SettingsTabAbout,
    SettingsTabAccount,

    // Settings - Playback Section
    SettingsPlaybackTitle,
    SettingsMusicQuality,
    SettingsMusicQualityDesc,
    SettingsFadeInOut,
    SettingsFadeInOutDesc,
    SettingsVolumeNormalization,
    SettingsVolumeNormalizationDesc,
    SettingsEqualizer,
    SettingsEqualizerDesc,

    // Audio Engine Page
    AudioEngineTitle,
    AudioEngineDesc,
    AudioEngineEqualizer,
    AudioEngineEqualizerDesc,
    AudioEngineVolumeVisualization,
    AudioEngineVolumeVisualizationDesc,
    AudioEngineWaveform,
    AudioEngineWaveformDesc,

    // Settings - Account Section
    SettingsAccountTitle,
    SettingsAccountNotLoggedIn,
    SettingsAccountLoggedInAs,
    SettingsAccountVipStatus,
    SettingsAccountLogout,

    // Settings - Display Section
    SettingsDisplayTitle,
    SettingsDarkMode,
    SettingsLanguage,
    SettingsPowerSavingMode,
    SettingsPowerSavingModeDesc,
    SettingsCloseBehavior,
    SettingsCloseBehaviorAsk,
    SettingsCloseBehaviorExit,
    SettingsCloseBehaviorMinimize,

    // Settings - System Section
    SettingsSystemTitle,
    SettingsAudioDevice,
    SettingsAudioBuffer,
    SettingsAudioBufferDesc,
    SettingsDefaultDevice,

    // Settings - Network Section
    SettingsNetworkTitle,
    SettingsTabNetwork,
    SettingsProxyType,
    SettingsProxyHost,
    SettingsProxyPort,
    SettingsProxyUsername,
    SettingsProxyPassword,
    SettingsProxyNone,
    SettingsProxySystem,

    // Settings - Storage Section
    SettingsStorageTitle,
    SettingsCacheLocation,
    SettingsCacheSize,
    SettingsMaxCache,
    SettingsClearCache,
    SettingsClearCacheDesc,
    SettingsClearButton,

    // Settings - Shortcuts Section
    SettingsShortcutsTitle,
    SettingsShortcutsPlayback,
    SettingsShortcutsNavigation,
    SettingsShortcutsUI,
    SettingsShortcutsGeneral,

    // Settings - About Section
    SettingsAboutTitle,
    SettingsAppName,
    SettingsVersion,
    SettingsDeveloper,
    SettingsDescription,

    // Shortcut Actions
    ActionPlayPause,
    ActionNextTrack,
    ActionPrevTrack,
    ActionVolumeUp,
    ActionVolumeDown,
    ActionVolumeMute,
    ActionSeekForward,
    ActionSeekBackward,
    ActionGoHome,
    ActionGoSearch,
    ActionGoQueue,
    ActionGoSettings,
    ActionToggleQueue,
    ActionToggleSidebar,
    ActionToggleFullscreen,
    ActionEscape,
    ActionDelete,
    ActionSelectAll,

    // Playlist Page
    PlaylistTypeLabel,
    PlaylistLikes,
    PlaylistSongCount,
    PlaylistCustomSort,
    PlaylistHeaderNumber,
    PlaylistHeaderTitle,
    PlaylistHeaderAlbum,
    PlaylistHeaderAddedDate,

    // Discover Page
    DiscoverRecommended,
    DiscoverHot,
    DiscoverSeeAll,

    // Common UI
    Loading,
    Cancel,
    Save,
    Delete,
    Refresh,

    // Lyrics Page
    LyricsNoLyrics,
    LyricsPureMusic,

    // Audio Engine
    AudioEngineEqualizerDisabled,
    AudioEngineSpectrum,

    // Queue Panel
    QueueTitle,
    QueueSongCount,
    QueueEmpty,

    // Playlist View
    PlaylistNoSongs,

    // Login Popup
    LoginScanQr,
    LoginGeneratingQr,
    LoginRefreshQr,
    LoginLoggedIn,
    LoginLogout,

    // Delete Playlist Dialog
    DeletePlaylistTitle,
    DeletePlaylistConfirm,

    // Edit Playlist Dialog
    EditPlaylistTitle,
    EditPlaylistChangeCover,
    EditPlaylistName,
    EditPlaylistNamePlaceholder,
    EditPlaylistDesc,
    EditPlaylistDescPlaceholder,

    // Exit Dialog
    ExitDialogTitle,
    ExitDialogMessage,
    ExitDialogExit,
    ExitDialogMinimize,
}

/// Get translation for a key in the specified language
pub fn t(lang: Language, key: Key) -> &'static str {
    let translations: &HashMap<Key, &'static str> = match lang {
        Language::English => en::translations(),
        Language::Chinese => zh::translations(),
    };

    translations.get(&key).copied().unwrap_or("???")
}

/// Localization context that can be passed around
#[derive(Debug, Clone, Copy, Default)]
pub struct Locale {
    pub language: Language,
}

impl Locale {
    pub fn new(language: Language) -> Self {
        Self { language }
    }

    /// Get translation for a key
    pub fn get(&self, key: Key) -> &'static str {
        t(self.language, key)
    }
}
