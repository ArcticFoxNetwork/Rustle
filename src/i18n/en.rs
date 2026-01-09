//! English translations

use super::Key;
use once_cell::sync::Lazy;
use std::collections::HashMap;

static TRANSLATIONS: Lazy<HashMap<Key, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();

    // App
    m.insert(Key::AppName, "Rustle");

    // Navigation
    m.insert(Key::NavHome, "Home");
    m.insert(Key::NavDiscover, "Discover");
    m.insert(Key::NavRadio, "Radio");
    m.insert(Key::NavSettings, "Settings");
    m.insert(Key::NavAudioEngine, "Audio Engine");

    // Library - Local
    m.insert(Key::LibraryTitle, "Library");
    m.insert(Key::LibraryRecentlyPlayed, "Recently Played");
    m.insert(Key::ImportLocalPlaylist, "Import Local Playlist");
    // Library - Cloud
    m.insert(Key::CloudPlaylistsTitle, "Cloud Playlists");
    m.insert(
        Key::CloudPlaylistsNotLoggedIn,
        "Login to view cloud playlists",
    );

    // User
    m.insert(Key::GuestUser, "Guest");
    m.insert(Key::NotLoggedIn, "Not logged in");
    m.insert(Key::ClickToLogin, "Click to login");
    m.insert(Key::FreeAccount, "Free Account");

    // Search
    m.insert(Key::SearchPlaceholder, "Search songs, artists, albums...");

    // Hero Banner
    m.insert(Key::HeroTitle, "Global Hits 2024");
    m.insert(Key::HeroSubtitle, "The biggest songs from around the world");
    m.insert(Key::PlayButton, "Play");

    // Trending
    m.insert(Key::TrendingSongs, "Trending Songs");
    m.insert(Key::SeeAll, "See All");

    // Recently Played
    m.insert(Key::RecentlyPlayed, "Recently Played");
    m.insert(Key::RecentlyPlayedDescription, "Last 200 songs played");
    m.insert(Key::RecentlyPlayedList, "Recently Played");

    // Window Controls
    m.insert(Key::Minimize, "Minimize");
    m.insert(Key::Maximize, "Maximize");
    m.insert(Key::Close, "Close");
    m.insert(Key::Settings, "Settings");

    // Navigation Controls
    m.insert(Key::Back, "Back");
    m.insert(Key::Forward, "Forward");

    // Settings Page - Tabs
    m.insert(Key::SettingsTitle, "Settings");
    m.insert(Key::SettingsTabAccount, "Account");
    m.insert(Key::SettingsTabPlayback, "Playback");
    m.insert(Key::SettingsTabDisplay, "Display");
    m.insert(Key::SettingsTabSystem, "System");
    m.insert(Key::SettingsTabStorage, "Storage");
    m.insert(Key::SettingsTabShortcuts, "Shortcuts");
    m.insert(Key::SettingsTabAbout, "About");

    // Settings - Playback Section
    m.insert(Key::SettingsPlaybackTitle, "Playback Settings");
    m.insert(Key::SettingsMusicQuality, "Music Quality");
    m.insert(
        Key::SettingsMusicQualityDesc,
        "Select audio quality for online streaming",
    );
    m.insert(Key::SettingsFadeInOut, "Fade In/Out");
    m.insert(
        Key::SettingsFadeInOutDesc,
        "Smooth volume transition when playing/pausing",
    );
    m.insert(Key::SettingsVolumeNormalization, "Volume Normalization");
    m.insert(
        Key::SettingsVolumeNormalizationDesc,
        "Auto-adjust volume for consistent playback",
    );
    m.insert(Key::SettingsEqualizer, "Equalizer");
    m.insert(Key::SettingsEqualizerDesc, "Enable audio equalizer");

    // Audio Engine Page
    m.insert(Key::AudioEngineTitle, "Rustle Audio Engine");
    m.insert(
        Key::AudioEngineDesc,
        "Advanced audio processing and visualization",
    );
    m.insert(Key::AudioEngineEqualizer, "Equalizer");
    m.insert(
        Key::AudioEngineEqualizerDesc,
        "10-band parametric equalizer",
    );
    m.insert(Key::AudioEngineVolumeVisualization, "Volume Visualization");
    m.insert(
        Key::AudioEngineVolumeVisualizationDesc,
        "Real-time volume level display",
    );
    m.insert(Key::AudioEngineWaveform, "Waveform Display");
    m.insert(
        Key::AudioEngineWaveformDesc,
        "Real-time audio waveform visualization",
    );

    // Settings - Account Section
    m.insert(Key::SettingsAccountTitle, "Account Settings");
    m.insert(Key::SettingsAccountNotLoggedIn, "Not logged in");
    m.insert(Key::SettingsAccountLoggedInAs, "Logged in as");
    m.insert(Key::SettingsAccountVipStatus, "VIP Status");
    m.insert(Key::SettingsAccountLogout, "Log Out");

    // Settings - Display Section
    m.insert(Key::SettingsDisplayTitle, "Display & Interface");
    m.insert(Key::SettingsDarkMode, "Dark Mode");
    m.insert(Key::SettingsLanguage, "Language");
    m.insert(Key::SettingsPowerSavingMode, "Power Saving Mode");
    m.insert(
        Key::SettingsPowerSavingModeDesc,
        "Disable animations and effects to reduce CPU usage",
    );
    m.insert(Key::SettingsCloseBehavior, "Close Button Behavior");
    m.insert(Key::SettingsCloseBehaviorAsk, "Ask");
    m.insert(Key::SettingsCloseBehaviorExit, "Exit");
    m.insert(Key::SettingsCloseBehaviorMinimize, "Minimize to Tray");

    // Settings - System Section
    m.insert(Key::SettingsSystemTitle, "System Settings");
    m.insert(Key::SettingsAudioDevice, "Audio Output Device");
    m.insert(Key::SettingsAudioBuffer, "Audio Buffer");
    m.insert(
        Key::SettingsAudioBufferDesc,
        "Larger buffer reduces audio stuttering",
    );
    m.insert(Key::SettingsDefaultDevice, "Default Device");

    // Settings - Network Section
    m.insert(Key::SettingsNetworkTitle, "Network Settings");
    m.insert(Key::SettingsTabNetwork, "Network");
    m.insert(Key::SettingsProxyType, "Proxy Type");
    m.insert(Key::SettingsProxyHost, "Proxy Host");
    m.insert(Key::SettingsProxyPort, "Proxy Port");
    m.insert(Key::SettingsProxyUsername, "Username");
    m.insert(Key::SettingsProxyPassword, "Password");
    m.insert(Key::SettingsProxyNone, "No Proxy");
    m.insert(Key::SettingsProxySystem, "System Proxy");

    // Settings - Storage Section
    m.insert(Key::SettingsStorageTitle, "Storage Settings");
    m.insert(Key::SettingsCacheLocation, "Cache Location");
    m.insert(Key::SettingsCacheSize, "Current Cache Size");
    m.insert(Key::SettingsMaxCache, "Max Cache Size");
    m.insert(Key::SettingsClearCache, "Clear Cache");
    m.insert(Key::SettingsClearCacheDesc, "Delete all cached audio files");
    m.insert(Key::SettingsClearButton, "Clear");

    // Settings - Shortcuts Section
    m.insert(Key::SettingsShortcutsTitle, "Keyboard Shortcuts");
    m.insert(Key::SettingsShortcutsPlayback, "Playback Controls");
    m.insert(Key::SettingsShortcutsNavigation, "Navigation");
    m.insert(Key::SettingsShortcutsUI, "Interface");
    m.insert(Key::SettingsShortcutsGeneral, "General");

    // Settings - About Section
    m.insert(Key::SettingsAboutTitle, "About");
    m.insert(Key::SettingsAppName, "App Name");
    m.insert(Key::SettingsVersion, "Version");
    m.insert(Key::SettingsDeveloper, "Developer");
    m.insert(
        Key::SettingsDescription,
        "A modern local music player built with Rust",
    );

    // Shortcut Actions
    m.insert(Key::ActionPlayPause, "Play/Pause");
    m.insert(Key::ActionNextTrack, "Next Track");
    m.insert(Key::ActionPrevTrack, "Previous Track");
    m.insert(Key::ActionVolumeUp, "Volume Up");
    m.insert(Key::ActionVolumeDown, "Volume Down");
    m.insert(Key::ActionVolumeMute, "Mute");
    m.insert(Key::ActionSeekForward, "Seek Forward");
    m.insert(Key::ActionSeekBackward, "Seek Backward");
    m.insert(Key::ActionGoHome, "Go Home");
    m.insert(Key::ActionGoSearch, "Search");
    m.insert(Key::ActionGoQueue, "Queue");
    m.insert(Key::ActionGoSettings, "Settings");
    m.insert(Key::ActionToggleQueue, "Toggle Queue");
    m.insert(Key::ActionToggleSidebar, "Toggle Sidebar");
    m.insert(Key::ActionToggleFullscreen, "Fullscreen");
    m.insert(Key::ActionEscape, "Cancel/Close");
    m.insert(Key::ActionDelete, "Delete");
    m.insert(Key::ActionSelectAll, "Select All");

    // Playlist Page
    m.insert(Key::PlaylistTypeLabel, "Playlist");
    m.insert(Key::PlaylistLikes, "{} likes");
    m.insert(Key::PlaylistSongCount, "{} songs");
    m.insert(Key::PlaylistCustomSort, "Custom Sort");
    m.insert(Key::PlaylistHeaderNumber, "#");
    m.insert(Key::PlaylistHeaderTitle, "Title");
    m.insert(Key::PlaylistHeaderAlbum, "Album");
    m.insert(Key::PlaylistHeaderAddedDate, "Added Date");

    // Discover Page
    m.insert(Key::DiscoverRecommended, "Recommended Playlists");
    m.insert(Key::DiscoverHot, "Hot Playlists");
    m.insert(Key::DiscoverSeeAll, "See All");
    m.insert(Key::DiscoverDailyRecommend, "Daily Recommend");
    m.insert(Key::DiscoverDailyRecommendDesc, "Personalized for you, updated daily at 6:00");
    m.insert(Key::DiscoverDailyRecommendCreator, "NetEase Music");
    m.insert(Key::DiscoverLoadFailed, "Failed to load daily recommend");
    m.insert(Key::DiscoverPlaylistLoadFailed, "Failed to load playlist");

    // Common UI
    m.insert(Key::Loading, "Loading...");
    m.insert(Key::Cancel, "Cancel");
    m.insert(Key::Save, "Save");
    m.insert(Key::Delete, "Delete");
    m.insert(Key::Refresh, "Refresh");

    // Lyrics Page
    m.insert(Key::LyricsNoLyrics, "No lyrics available");
    m.insert(Key::LyricsPureMusic, "Instrumental");

    // Audio Engine
    m.insert(Key::AudioEngineEqualizerDisabled, "Equalizer disabled");
    m.insert(Key::AudioEngineSpectrum, "Spectrum");

    // Queue Panel
    m.insert(Key::QueueTitle, "Play Queue");
    m.insert(Key::QueueSongCount, "{} songs");
    m.insert(Key::QueueEmpty, "Queue is empty");

    // Playlist View
    m.insert(Key::PlaylistNoSongs, "No songs");

    // Login Popup
    m.insert(Key::LoginScanQr, "Scan QR to Login");
    m.insert(Key::LoginGeneratingQr, "Generating QR code...");
    m.insert(Key::LoginRefreshQr, "Refresh QR Code");
    m.insert(Key::LoginLoggedIn, "Logged In");
    m.insert(Key::LoginLogout, "Log Out");

    // Delete Playlist Dialog
    m.insert(Key::DeletePlaylistTitle, "Delete Playlist");
    m.insert(
        Key::DeletePlaylistConfirm,
        "Are you sure you want to delete this playlist?",
    );

    // Edit Playlist Dialog
    m.insert(Key::EditPlaylistTitle, "Edit Playlist");
    m.insert(Key::EditPlaylistChangeCover, "Change Cover");
    m.insert(Key::EditPlaylistName, "Playlist Name");
    m.insert(Key::EditPlaylistNamePlaceholder, "Enter playlist name...");
    m.insert(Key::EditPlaylistDesc, "Description");
    m.insert(
        Key::EditPlaylistDescPlaceholder,
        "Enter description (optional)...",
    );

    // Exit Dialog
    m.insert(Key::ExitDialogTitle, "Exit Application");
    m.insert(
        Key::ExitDialogMessage,
        "Do you want to exit or minimize to system tray?",
    );
    m.insert(Key::ExitDialogExit, "Exit");
    m.insert(Key::ExitDialogMinimize, "Minimize to Tray");

    m
});

pub fn translations() -> &'static HashMap<Key, &'static str> {
    &TRANSLATIONS
}
