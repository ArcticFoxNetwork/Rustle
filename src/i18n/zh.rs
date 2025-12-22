//! Chinese translations (简体中文)

use super::Key;
use once_cell::sync::Lazy;
use std::collections::HashMap;

static TRANSLATIONS: Lazy<HashMap<Key, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();

    // App
    m.insert(Key::AppName, "Rustle");

    // Navigation
    m.insert(Key::NavHome, "首页");
    m.insert(Key::NavDiscover, "发现");
    m.insert(Key::NavRadio, "电台");
    m.insert(Key::NavSettings, "设置");
    m.insert(Key::NavAudioEngine, "音频引擎");

    // Library - Local
    m.insert(Key::LibraryTitle, "音乐库");
    m.insert(Key::LibraryRecentlyPlayed, "最近播放");
    m.insert(Key::ImportLocalPlaylist, "导入本地歌单");
    // Library - Cloud
    m.insert(Key::CloudPlaylistsTitle, "云端歌单");
    m.insert(Key::CloudPlaylistsNotLoggedIn, "登录以查看云端歌单");

    // User
    m.insert(Key::GuestUser, "访客");
    m.insert(Key::NotLoggedIn, "未登录");
    m.insert(Key::ClickToLogin, "点击登录");
    m.insert(Key::FreeAccount, "免费账户");

    // Search
    m.insert(Key::SearchPlaceholder, "搜索歌曲、艺术家、专辑...");

    // Hero Banner
    m.insert(Key::HeroTitle, "2024 全球热门");
    m.insert(Key::HeroSubtitle, "来自世界各地的热门歌曲");
    m.insert(Key::PlayButton, "播放");

    // Trending
    m.insert(Key::TrendingSongs, "热门歌曲");
    m.insert(Key::SeeAll, "查看全部");

    // Recently Played
    m.insert(Key::RecentlyPlayed, "最近播放");
    m.insert(Key::RecentlyPlayedDescription, "最近播放的200首歌曲");
    m.insert(Key::RecentlyPlayedList, "最近播放");

    // Window Controls
    m.insert(Key::Minimize, "最小化");
    m.insert(Key::Maximize, "最大化");
    m.insert(Key::Close, "关闭");
    m.insert(Key::Settings, "设置");

    // Navigation Controls
    m.insert(Key::Back, "后退");
    m.insert(Key::Forward, "前进");

    // Settings Page - Tabs
    m.insert(Key::SettingsTitle, "设置");
    m.insert(Key::SettingsTabAccount, "账号");
    m.insert(Key::SettingsTabPlayback, "播放");
    m.insert(Key::SettingsTabDisplay, "界面");
    m.insert(Key::SettingsTabSystem, "系统");
    m.insert(Key::SettingsTabStorage, "存储");
    m.insert(Key::SettingsTabShortcuts, "快捷键");
    m.insert(Key::SettingsTabAbout, "关于");

    // Settings - Playback Section
    m.insert(Key::SettingsPlaybackTitle, "播放设置");
    m.insert(Key::SettingsMusicQuality, "音质选择");
    m.insert(Key::SettingsMusicQualityDesc, "选择在线播放的音频质量");
    m.insert(Key::SettingsFadeInOut, "淡入淡出");
    m.insert(Key::SettingsFadeInOutDesc, "播放和暂停时平滑过渡音量");
    m.insert(Key::SettingsVolumeNormalization, "音量标准化");
    m.insert(
        Key::SettingsVolumeNormalizationDesc,
        "自动调整音量使所有歌曲音量一致",
    );
    m.insert(Key::SettingsEqualizer, "均衡器");
    m.insert(Key::SettingsEqualizerDesc, "启用音频均衡器调节");

    // Audio Engine Page
    m.insert(Key::AudioEngineTitle, "Rustle 音频引擎");
    m.insert(Key::AudioEngineDesc, "高级音频处理与可视化");
    m.insert(Key::AudioEngineEqualizer, "均衡器");
    m.insert(Key::AudioEngineEqualizerDesc, "10 频段参数均衡器");
    m.insert(Key::AudioEngineVolumeVisualization, "音量可视化");
    m.insert(Key::AudioEngineVolumeVisualizationDesc, "实时音量电平显示");
    m.insert(Key::AudioEngineWaveform, "波形显示");
    m.insert(Key::AudioEngineWaveformDesc, "音频波形实时可视化");

    // Settings - Account Section
    m.insert(Key::SettingsAccountTitle, "账号设置");
    m.insert(Key::SettingsAccountNotLoggedIn, "当前未登录");
    m.insert(Key::SettingsAccountLoggedInAs, "当前登录账号");
    m.insert(Key::SettingsAccountVipStatus, "VIP 状态");
    m.insert(Key::SettingsAccountLogout, "退出登录");

    // Settings - Display Section
    m.insert(Key::SettingsDisplayTitle, "界面与显示");
    m.insert(Key::SettingsDarkMode, "深色模式");
    m.insert(Key::SettingsLanguage, "应用语言");
    m.insert(Key::SettingsPowerSavingMode, "省电模式");
    m.insert(
        Key::SettingsPowerSavingModeDesc,
        "关闭动画和特效，降低 CPU 占用",
    );
    m.insert(Key::SettingsCloseBehavior, "关闭按钮行为");
    m.insert(Key::SettingsCloseBehaviorAsk, "询问");
    m.insert(Key::SettingsCloseBehaviorExit, "退出");
    m.insert(Key::SettingsCloseBehaviorMinimize, "最小化到托盘");

    // Settings - System Section
    m.insert(Key::SettingsSystemTitle, "系统设置");
    m.insert(Key::SettingsAudioDevice, "音频输出设备");
    m.insert(Key::SettingsAudioBuffer, "音频缓冲区");
    m.insert(Key::SettingsAudioBufferDesc, "较大的缓冲区可减少音频卡顿");
    m.insert(Key::SettingsDefaultDevice, "默认设备");

    // Settings - Network Section
    m.insert(Key::SettingsNetworkTitle, "网络设置");
    m.insert(Key::SettingsTabNetwork, "网络");
    m.insert(Key::SettingsProxyType, "代理类型");
    m.insert(Key::SettingsProxyHost, "代理地址");
    m.insert(Key::SettingsProxyPort, "代理端口");
    m.insert(Key::SettingsProxyUsername, "用户名");
    m.insert(Key::SettingsProxyPassword, "密码");
    m.insert(Key::SettingsProxyNone, "无代理");
    m.insert(Key::SettingsProxySystem, "系统代理");

    // Settings - Storage Section
    m.insert(Key::SettingsStorageTitle, "存储设置");
    m.insert(Key::SettingsCacheLocation, "缓存位置");
    m.insert(Key::SettingsCacheSize, "当前缓存大小");
    m.insert(Key::SettingsMaxCache, "最大缓存占用");
    m.insert(Key::SettingsClearCache, "清除缓存");
    m.insert(Key::SettingsClearCacheDesc, "删除所有缓存的音频文件");
    m.insert(Key::SettingsClearButton, "清除");

    // Settings - Shortcuts Section
    m.insert(Key::SettingsShortcutsTitle, "快捷键设置");
    m.insert(Key::SettingsShortcutsPlayback, "播放控制");
    m.insert(Key::SettingsShortcutsNavigation, "导航");
    m.insert(Key::SettingsShortcutsUI, "界面");
    m.insert(Key::SettingsShortcutsGeneral, "通用");

    // Settings - About Section
    m.insert(Key::SettingsAboutTitle, "关于");
    m.insert(Key::SettingsAppName, "应用名称");
    m.insert(Key::SettingsVersion, "版本");
    m.insert(Key::SettingsDeveloper, "开发者");
    m.insert(
        Key::SettingsDescription,
        "一个基于 Rust 的现代化本地音乐播放器",
    );

    // Shortcut Actions
    m.insert(Key::ActionPlayPause, "播放/暂停");
    m.insert(Key::ActionNextTrack, "下一首");
    m.insert(Key::ActionPrevTrack, "上一首");
    m.insert(Key::ActionVolumeUp, "增加音量");
    m.insert(Key::ActionVolumeDown, "减少音量");
    m.insert(Key::ActionVolumeMute, "静音");
    m.insert(Key::ActionSeekForward, "快进");
    m.insert(Key::ActionSeekBackward, "快退");
    m.insert(Key::ActionGoHome, "返回首页");
    m.insert(Key::ActionGoSearch, "搜索");
    m.insert(Key::ActionGoQueue, "播放队列");
    m.insert(Key::ActionGoSettings, "设置");
    m.insert(Key::ActionToggleQueue, "显示/隐藏队列");
    m.insert(Key::ActionToggleSidebar, "显示/隐藏侧边栏");
    m.insert(Key::ActionToggleFullscreen, "全屏");
    m.insert(Key::ActionEscape, "取消/关闭");
    m.insert(Key::ActionDelete, "删除");
    m.insert(Key::ActionSelectAll, "全选");

    // Playlist Page
    m.insert(Key::PlaylistTypeLabel, "歌单");
    m.insert(Key::PlaylistLikes, "{} 次点赞");
    m.insert(Key::PlaylistSongCount, "{} 首歌曲");
    m.insert(Key::PlaylistCustomSort, "自定义排序");
    m.insert(Key::PlaylistHeaderNumber, "#");
    m.insert(Key::PlaylistHeaderTitle, "标题");
    m.insert(Key::PlaylistHeaderAlbum, "专辑");
    m.insert(Key::PlaylistHeaderAddedDate, "添加日期");

    // Discover Page
    m.insert(Key::DiscoverRecommended, "推荐歌单");
    m.insert(Key::DiscoverHot, "热门歌单");
    m.insert(Key::DiscoverSeeAll, "查看全部");

    // Common UI
    m.insert(Key::Loading, "加载中...");
    m.insert(Key::Cancel, "取消");
    m.insert(Key::Save, "保存");
    m.insert(Key::Delete, "删除");
    m.insert(Key::Refresh, "刷新");

    // Lyrics Page
    m.insert(Key::LyricsNoLyrics, "暂无歌词");
    m.insert(Key::LyricsPureMusic, "纯音乐，请欣赏");

    // Audio Engine
    m.insert(Key::AudioEngineEqualizerDisabled, "均衡器已关闭");
    m.insert(Key::AudioEngineSpectrum, "频谱");

    // Queue Panel
    m.insert(Key::QueueTitle, "播放队列");
    m.insert(Key::QueueSongCount, "{} 首");
    m.insert(Key::QueueEmpty, "队列为空");

    // Playlist View
    m.insert(Key::PlaylistNoSongs, "暂无歌曲");

    // Login Popup
    m.insert(Key::LoginScanQr, "扫码登录");
    m.insert(Key::LoginGeneratingQr, "生成二维码中...");
    m.insert(Key::LoginRefreshQr, "刷新二维码");
    m.insert(Key::LoginLoggedIn, "已登录");
    m.insert(Key::LoginLogout, "退出登录");

    // Delete Playlist Dialog
    m.insert(Key::DeletePlaylistTitle, "删除歌单");
    m.insert(Key::DeletePlaylistConfirm, "确定要删除这个歌单吗？");

    // Edit Playlist Dialog
    m.insert(Key::EditPlaylistTitle, "编辑歌单");
    m.insert(Key::EditPlaylistChangeCover, "更换封面");
    m.insert(Key::EditPlaylistName, "歌单名称");
    m.insert(Key::EditPlaylistNamePlaceholder, "输入歌单名称...");
    m.insert(Key::EditPlaylistDesc, "歌单描述");
    m.insert(Key::EditPlaylistDescPlaceholder, "输入歌单描述（可选）...");

    // Exit Dialog
    m.insert(Key::ExitDialogTitle, "退出应用");
    m.insert(
        Key::ExitDialogMessage,
        "你想要关闭应用还是最小化到系统托盘？",
    );
    m.insert(Key::ExitDialogExit, "退出");
    m.insert(Key::ExitDialogMinimize, "最小化到托盘");

    m
});

pub fn translations() -> &'static HashMap<Key, &'static str> {
    &TRANSLATIONS
}
