//! System tray integration
//!
//! Provides system tray icon with menu for controlling the application
//! when minimized to tray.
//!
//! - Linux: Uses ksni (freedesktop)
//! - Windows: Uses tray-icon
//! - macOS: Uses tray-icon

use super::PlayMode;
use tokio::sync::mpsc;

/// Commands that can be sent from the tray to the application
#[derive(Debug, Clone)]
pub enum TrayCommand {
    /// Toggle show/hide window
    ToggleWindow,
    /// Toggle play/pause
    PlayPause,
    /// Play next track
    NextTrack,
    /// Play previous track
    PrevTrack,
    /// Set play mode
    SetPlayMode(PlayMode),
    /// Toggle favorite status for current song
    ToggleFavorite,
    /// Quit the application
    Quit,
}

/// State shared between tray and application
#[derive(Debug, Clone)]
pub struct TrayState {
    /// Whether music is currently playing
    pub is_playing: bool,
    /// Current song title
    pub title: Option<String>,
    /// Current artist
    pub artist: Option<String>,
    /// Current play mode
    pub play_mode: PlayMode,
    /// Current song NCM ID (if NCM song, for favorite toggle)
    pub ncm_song_id: Option<u64>,
    /// Whether current song is favorited
    pub is_favorited: bool,
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            is_playing: false,
            title: None,
            artist: None,
            play_mode: PlayMode::Sequential,
            ncm_song_id: None,
            is_favorited: false,
        }
    }
}

/// Handle to control the tray from the application
#[derive(Clone)]
pub struct TrayHandle {
    #[cfg(target_os = "linux")]
    handle: ksni::Handle<linux::LinuxTray>,
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    tx: mpsc::UnboundedSender<TrayState>,
}

impl TrayHandle {
    /// Update the tray state (call when playback state changes)
    pub async fn update(&self, state: TrayState) {
        #[cfg(target_os = "linux")]
        {
            let _ = self
                .handle
                .update(|tray| {
                    tray.update_state(state);
                })
                .await;
        }

        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            let _ = self.tx.send(state);
        }
    }
}

/// Start the system tray service (async, Linux only)
/// Returns a handle to control the tray and a receiver for commands
#[cfg(target_os = "linux")]
pub async fn start_tray() -> anyhow::Result<(TrayHandle, mpsc::UnboundedReceiver<TrayCommand>)> {
    linux::start_linux_tray().await
}

/// Start the system tray service synchronously (Windows/macOS only)
/// Returns a handle to control the tray and a receiver for commands
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub fn start_tray_sync() -> anyhow::Result<(TrayHandle, mpsc::UnboundedReceiver<TrayCommand>)> {
    native::start_native_tray_sync()
}

// Platform-specific implementations
#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use ksni::{Icon, MenuItem, Status, ToolTip, Tray as KsniTray, TrayMethods, menu::*};

    /// Linux system tray implementation using ksni
    pub struct LinuxTray {
        /// Channel to send commands to the application
        tx: mpsc::UnboundedSender<TrayCommand>,
        /// Current state
        state: TrayState,
    }

    impl LinuxTray {
        /// Create a new tray instance
        pub fn new(tx: mpsc::UnboundedSender<TrayCommand>) -> Self {
            Self {
                tx,
                state: TrayState::default(),
            }
        }

        /// Update the tray state
        pub fn update_state(&mut self, state: TrayState) {
            self.state = state;
        }
    }

    impl KsniTray for LinuxTray {
        fn id(&self) -> String {
            "rustle-music".to_string()
        }

        fn title(&self) -> String {
            "Rustle".to_string()
        }

        fn category(&self) -> ksni::Category {
            ksni::Category::ApplicationStatus
        }

        fn status(&self) -> Status {
            Status::Active
        }

        fn icon_name(&self) -> String {
            String::new()
        }

        fn icon_pixmap(&self) -> Vec<Icon> {
            create_icon()
        }

        fn tool_tip(&self) -> ToolTip {
            let title = "Rustle Music Player".to_string();
            let description = match (&self.state.title, &self.state.artist) {
                (Some(t), Some(a)) => format!("{} - {}", t, a),
                (Some(t), None) => t.clone(),
                _ if self.state.is_playing => "Playing...".to_string(),
                _ => "Not playing".to_string(),
            };

            ToolTip {
                title,
                description,
                icon_name: String::new(),
                icon_pixmap: vec![],
            }
        }

        fn menu(&self) -> Vec<MenuItem<Self>> {
            create_menu(&self.state, &self.tx)
        }

        fn activate(&mut self, _x: i32, _y: i32) {
            let _ = self.tx.send(TrayCommand::ToggleWindow);
        }
    }

    pub async fn start_linux_tray()
    -> anyhow::Result<(TrayHandle, mpsc::UnboundedReceiver<TrayCommand>)> {
        let (tx, rx) = mpsc::unbounded_channel();
        let tray = LinuxTray::new(tx);

        let handle = tray
            .spawn()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to start Linux tray: {}", e))?;

        Ok((TrayHandle { handle }, rx))
    }

    fn create_icon() -> Vec<Icon> {
        static ICON_DATA: &[u8] = include_bytes!("../../assets/icons/icon_256.png");

        if let Ok(img) = image::load_from_memory(ICON_DATA) {
            let rgba = img
                .resize(32, 32, image::imageops::FilterType::Lanczos3)
                .to_rgba8();
            let (width, height) = rgba.dimensions();

            let mut data = Vec::with_capacity((width * height * 4) as usize);
            for pixel in rgba.pixels() {
                data.push(pixel[3]); // A
                data.push(pixel[0]); // R
                data.push(pixel[1]); // G
                data.push(pixel[2]); // B
            }

            return vec![Icon {
                width: width as i32,
                height: height as i32,
                data,
            }];
        }

        // Fallback icon
        let size = 32;
        let mut data = vec![0u8; size * size * 4];
        for y in 0..size {
            for x in 0..size {
                let dx = x as f32 - size as f32 / 2.0;
                let dy = y as f32 - size as f32 / 2.0;
                let dist = (dx * dx + dy * dy).sqrt();

                let idx = (y * size + x) * 4;
                if dist < size as f32 / 2.0 - 2.0 {
                    data[idx] = 255; // A
                    data[idx + 1] = 255; // R
                    data[idx + 2] = 105; // G
                    data[idx + 3] = 180; // B
                }
            }
        }

        vec![Icon {
            width: size as i32,
            height: size as i32,
            data,
        }]
    }

    fn create_menu(
        state: &TrayState,
        _tx: &mpsc::UnboundedSender<TrayCommand>,
    ) -> Vec<MenuItem<LinuxTray>> {
        let play_label = if state.is_playing { "暂停" } else { "播放" };
        let play_icon = if state.is_playing {
            "media-playback-pause-symbolic"
        } else {
            "media-playback-start-symbolic"
        };

        let play_mode_index = match state.play_mode {
            PlayMode::Sequential => 0,
            PlayMode::LoopAll => 1,
            PlayMode::LoopOne => 2,
            PlayMode::Shuffle => 3,
        };

        vec![
            // Now playing info
            if let Some(title) = &state.title {
                let label = match &state.artist {
                    Some(artist) => format!("♪ {} - {}", title, artist),
                    None => format!("♪ {}", title),
                };
                StandardItem {
                    label,
                    enabled: false,
                    ..Default::default()
                }
                .into()
            } else {
                StandardItem {
                    label: "Rustle Music".to_string(),
                    enabled: false,
                    ..Default::default()
                }
                .into()
            },
            MenuItem::Separator,
            // Playback controls
            StandardItem {
                label: play_label.to_string(),
                icon_name: play_icon.to_string(),
                activate: Box::new(|tray: &mut LinuxTray| {
                    let _ = tray.tx.send(TrayCommand::PlayPause);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "上一首".to_string(),
                icon_name: "media-skip-backward-symbolic".to_string(),
                activate: Box::new(|tray: &mut LinuxTray| {
                    let _ = tray.tx.send(TrayCommand::PrevTrack);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "下一首".to_string(),
                icon_name: "media-skip-forward-symbolic".to_string(),
                activate: Box::new(|tray: &mut LinuxTray| {
                    let _ = tray.tx.send(TrayCommand::NextTrack);
                }),
                ..Default::default()
            }
            .into(),
            // Favorite button
            if state.ncm_song_id.is_some() {
                let (fav_label, fav_icon) = if state.is_favorited {
                    ("取消收藏", "starred-symbolic")
                } else {
                    ("收藏", "non-starred-symbolic")
                };
                StandardItem {
                    label: fav_label.to_string(),
                    icon_name: fav_icon.to_string(),
                    activate: Box::new(|tray: &mut LinuxTray| {
                        let _ = tray.tx.send(TrayCommand::ToggleFavorite);
                    }),
                    ..Default::default()
                }
                .into()
            } else {
                StandardItem {
                    label: "收藏".to_string(),
                    icon_name: "non-starred-symbolic".to_string(),
                    enabled: false,
                    ..Default::default()
                }
                .into()
            },
            MenuItem::Separator,
            // Play mode submenu
            SubMenu {
                label: "播放模式".to_string(),
                icon_name: "media-playlist-consecutive-symbolic".to_string(),
                submenu: vec![
                    RadioGroup {
                        selected: play_mode_index,
                        select: Box::new(|tray: &mut LinuxTray, index| {
                            let mode = match index {
                                0 => PlayMode::Sequential,
                                1 => PlayMode::LoopAll,
                                2 => PlayMode::LoopOne,
                                3 => PlayMode::Shuffle,
                                _ => PlayMode::Sequential,
                            };
                            let _ = tray.tx.send(TrayCommand::SetPlayMode(mode));
                        }),
                        options: vec![
                            RadioItem {
                                label: "顺序播放".to_string(),
                                icon_name: "media-playlist-consecutive-symbolic".to_string(),
                                ..Default::default()
                            },
                            RadioItem {
                                label: "列表循环".to_string(),
                                icon_name: "media-playlist-repeat-symbolic".to_string(),
                                ..Default::default()
                            },
                            RadioItem {
                                label: "单曲循环".to_string(),
                                icon_name: "media-playlist-repeat-song-symbolic".to_string(),
                                ..Default::default()
                            },
                            RadioItem {
                                label: "随机播放".to_string(),
                                icon_name: "media-playlist-shuffle-symbolic".to_string(),
                                ..Default::default()
                            },
                        ],
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            // Window control
            StandardItem {
                label: "显示/隐藏窗口".to_string(),
                icon_name: "view-restore-symbolic".to_string(),
                activate: Box::new(|tray: &mut LinuxTray| {
                    let _ = tray.tx.send(TrayCommand::ToggleWindow);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            // Quit
            StandardItem {
                label: "退出".to_string(),
                icon_name: "application-exit-symbolic".to_string(),
                activate: Box::new(|tray: &mut LinuxTray| {
                    let _ = tray.tx.send(TrayCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
mod native {
    use super::*;
    use tray_icon::{
        MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent,
        menu::{
            CheckMenuItem, Menu, MenuId, MenuItem as NativeMenuItem, PredefinedMenuItem, Submenu,
        },
    };

    // Menu item IDs
    const PLAY_PAUSE_ID: &str = "play_pause";
    const PREV_TRACK_ID: &str = "prev_track";
    const NEXT_TRACK_ID: &str = "next_track";
    const TOGGLE_FAVORITE_ID: &str = "toggle_favorite";
    const SEQUENTIAL_ID: &str = "sequential";
    const LOOP_ALL_ID: &str = "loop_all";
    const LOOP_ONE_ID: &str = "loop_one";
    const SHUFFLE_ID: &str = "shuffle";
    const TOGGLE_WINDOW_ID: &str = "toggle_window";
    const QUIT_ID: &str = "quit";

    /// Wrapper to make menu items Send+Sync (they're only accessed from main thread)
    struct MenuItemsWrapper {
        play_pause: *const NativeMenuItem,
        sequential: *const CheckMenuItem,
        loop_all: *const CheckMenuItem,
        loop_one: *const CheckMenuItem,
        shuffle: *const CheckMenuItem,
    }

    // SAFETY: Menu items are only accessed from the main thread
    unsafe impl Send for MenuItemsWrapper {}
    unsafe impl Sync for MenuItemsWrapper {}

    static MENU_ITEMS: std::sync::OnceLock<MenuItemsWrapper> = std::sync::OnceLock::new();

    /// Update menu items based on current state
    pub fn update_menu_state(state: &TrayState) {
        if let Some(items) = MENU_ITEMS.get() {
            // SAFETY: These pointers are valid for the lifetime of the application
            unsafe {
                // Update play/pause label
                let play_label = if state.is_playing { "暂停" } else { "播放" };
                (*items.play_pause).set_text(play_label);

                // Update play mode checkmarks
                (*items.sequential).set_checked(matches!(state.play_mode, PlayMode::Sequential));
                (*items.loop_all).set_checked(matches!(state.play_mode, PlayMode::LoopAll));
                (*items.loop_one).set_checked(matches!(state.play_mode, PlayMode::LoopOne));
                (*items.shuffle).set_checked(matches!(state.play_mode, PlayMode::Shuffle));
            }
        }
    }

    #[allow(dead_code)]
    pub async fn start_native_tray()
    -> anyhow::Result<(TrayHandle, mpsc::UnboundedReceiver<TrayCommand>)> {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (state_tx, mut state_rx) = mpsc::unbounded_channel();

        // Load icon
        let icon = load_icon()?;

        // Create initial menu
        let (menu, play_pause, sequential, loop_all, loop_one, shuffle) =
            create_native_menu_with_items(&TrayState::default())?;

        // Leak menu items and store pointers
        let play_pause = Box::leak(Box::new(play_pause));
        let sequential = Box::leak(Box::new(sequential));
        let loop_all = Box::leak(Box::new(loop_all));
        let loop_one = Box::leak(Box::new(loop_one));
        let shuffle = Box::leak(Box::new(shuffle));

        let _ = MENU_ITEMS.set(MenuItemsWrapper {
            play_pause: play_pause as *const _,
            sequential: sequential as *const _,
            loop_all: loop_all as *const _,
            loop_one: loop_one as *const _,
            shuffle: shuffle as *const _,
        });

        // Create tray icon
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip("Rustle Music Player")
            .with_icon(icon)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create tray icon: {}", e))?;

        // Leak the tray icon to keep it alive for the lifetime of the application
        Box::leak(Box::new(tray));

        // 设置事件处理器，将事件转发到 channel
        let cmd_tx_menu = cmd_tx.clone();
        tray_icon::menu::MenuEvent::set_event_handler(Some(
            move |event: tray_icon::menu::MenuEvent| {
                let id_str = event.id.0.as_str();
                let command = match id_str {
                    PLAY_PAUSE_ID => Some(TrayCommand::PlayPause),
                    PREV_TRACK_ID => Some(TrayCommand::PrevTrack),
                    NEXT_TRACK_ID => Some(TrayCommand::NextTrack),
                    TOGGLE_FAVORITE_ID => Some(TrayCommand::ToggleFavorite),
                    SEQUENTIAL_ID => Some(TrayCommand::SetPlayMode(PlayMode::Sequential)),
                    LOOP_ALL_ID => Some(TrayCommand::SetPlayMode(PlayMode::LoopAll)),
                    LOOP_ONE_ID => Some(TrayCommand::SetPlayMode(PlayMode::LoopOne)),
                    SHUFFLE_ID => Some(TrayCommand::SetPlayMode(PlayMode::Shuffle)),
                    TOGGLE_WINDOW_ID => Some(TrayCommand::ToggleWindow),
                    QUIT_ID => Some(TrayCommand::Quit),
                    _ => None,
                };
                if let Some(cmd) = command {
                    let _ = cmd_tx_menu.send(cmd);
                }
            },
        ));

        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| match event {
            TrayIconEvent::Click {
                button,
                button_state,
                ..
            } => {
                if button == MouseButton::Left && button_state == MouseButtonState::Up {
                    let _ = cmd_tx.send(TrayCommand::ToggleWindow);
                }
            }
            _ => {}
        }));

        // Handle state updates
        tokio::spawn(async move {
            while let Some(state) = state_rx.recv().await {
                tracing::debug!("Tray state updated: {:?}", state);
                update_menu_state(&state);
            }
        });

        Ok((TrayHandle { tx: state_tx }, cmd_rx))
    }

    /// Synchronous version for Windows main thread requirement
    pub fn start_native_tray_sync()
    -> anyhow::Result<(TrayHandle, mpsc::UnboundedReceiver<TrayCommand>)> {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (state_tx, mut state_rx) = mpsc::unbounded_channel::<TrayState>();

        // Load icon
        let icon = load_icon()?;

        // Create initial menu with items for updates
        let (menu, play_pause, sequential, loop_all, loop_one, shuffle) =
            create_native_menu_with_items(&TrayState::default())?;

        // Leak menu items and store pointers (only if not already set)
        let play_pause = Box::leak(Box::new(play_pause));
        let sequential = Box::leak(Box::new(sequential));
        let loop_all = Box::leak(Box::new(loop_all));
        let loop_one = Box::leak(Box::new(loop_one));
        let shuffle = Box::leak(Box::new(shuffle));

        let _ = MENU_ITEMS.set(MenuItemsWrapper {
            play_pause: play_pause as *const _,
            sequential: sequential as *const _,
            loop_all: loop_all as *const _,
            loop_one: loop_one as *const _,
            shuffle: shuffle as *const _,
        });

        // Create tray icon
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false) // Only show menu on right click
            .with_tooltip("Rustle Music Player")
            .with_icon(icon)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create tray icon: {}", e))?;

        // Leak the tray icon to keep it alive for the lifetime of the application
        Box::leak(Box::new(tray));

        // Set up event handlers that forward events to our channel
        let cmd_tx_menu = cmd_tx.clone();
        tray_icon::menu::MenuEvent::set_event_handler(Some(
            move |event: tray_icon::menu::MenuEvent| {
                let id_str = event.id.0.as_str();
                tracing::info!("Menu event received: {}", id_str);
                let command = match id_str {
                    PLAY_PAUSE_ID => Some(TrayCommand::PlayPause),
                    PREV_TRACK_ID => Some(TrayCommand::PrevTrack),
                    NEXT_TRACK_ID => Some(TrayCommand::NextTrack),
                    TOGGLE_FAVORITE_ID => Some(TrayCommand::ToggleFavorite),
                    SEQUENTIAL_ID => Some(TrayCommand::SetPlayMode(PlayMode::Sequential)),
                    LOOP_ALL_ID => Some(TrayCommand::SetPlayMode(PlayMode::LoopAll)),
                    LOOP_ONE_ID => Some(TrayCommand::SetPlayMode(PlayMode::LoopOne)),
                    SHUFFLE_ID => Some(TrayCommand::SetPlayMode(PlayMode::Shuffle)),
                    TOGGLE_WINDOW_ID => Some(TrayCommand::ToggleWindow),
                    QUIT_ID => Some(TrayCommand::Quit),
                    _ => None,
                };
                if let Some(cmd) = command {
                    tracing::info!("Sending tray command: {:?}", cmd);
                    if let Err(e) = cmd_tx_menu.send(cmd) {
                        tracing::error!("Failed to send tray command: {}", e);
                    }
                }
            },
        ));

        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| match event {
            TrayIconEvent::Click {
                button,
                button_state,
                ..
            } => {
                tracing::info!("Tray icon clicked: {:?} {:?}", button, button_state);
                if button == MouseButton::Left && button_state == MouseButtonState::Up {
                    tracing::info!("Sending ToggleWindow command");
                    if let Err(e) = cmd_tx.send(TrayCommand::ToggleWindow) {
                        tracing::error!("Failed to send ToggleWindow command: {}", e);
                    }
                }
            }
            _ => {}
        }));

        // Spawn task to handle state updates
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                while let Some(state) = state_rx.recv().await {
                    tracing::debug!("Tray state updated: {:?}", state);
                    update_menu_state(&state);
                }
            });
        });

        Ok((TrayHandle { tx: state_tx }, cmd_rx))
    }

    fn load_icon() -> anyhow::Result<tray_icon::Icon> {
        static ICON_DATA: &[u8] = include_bytes!("../../assets/icons/icon_256.png");

        let img = image::load_from_memory(ICON_DATA)
            .map_err(|e| anyhow::anyhow!("Failed to load icon: {}", e))?;

        let rgba = img
            .resize(32, 32, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        let (width, height) = rgba.dimensions();

        tray_icon::Icon::from_rgba(rgba.into_raw(), width, height)
            .map_err(|e| anyhow::anyhow!("Failed to create icon: {}", e))
    }

    fn create_native_menu_with_items(
        state: &TrayState,
    ) -> anyhow::Result<(
        Menu,
        NativeMenuItem,
        CheckMenuItem,
        CheckMenuItem,
        CheckMenuItem,
        CheckMenuItem,
    )> {
        let menu = Menu::new();

        // Now playing info (disabled item)
        let now_playing_text = if let Some(title) = &state.title {
            match &state.artist {
                Some(artist) => format!("♪ {} - {}", title, artist),
                None => format!("♪ {}", title),
            }
        } else {
            "Rustle Music".to_string()
        };

        let now_playing =
            NativeMenuItem::with_id(MenuId::new("now_playing"), now_playing_text, false, None);
        menu.append(&now_playing).ok();

        // Separator
        menu.append(&PredefinedMenuItem::separator()).ok();

        // Playback controls
        let play_label = if state.is_playing { "暂停" } else { "播放" };
        let play_pause =
            NativeMenuItem::with_id(MenuId::new(PLAY_PAUSE_ID), play_label, true, None);
        menu.append(&play_pause).ok();

        let prev_track = NativeMenuItem::with_id(MenuId::new(PREV_TRACK_ID), "上一首", true, None);
        menu.append(&prev_track).ok();

        let next_track = NativeMenuItem::with_id(MenuId::new(NEXT_TRACK_ID), "下一首", true, None);
        menu.append(&next_track).ok();

        // Favorite button (only for NCM songs)
        if state.ncm_song_id.is_some() {
            let fav_label = if state.is_favorited {
                "取消收藏"
            } else {
                "收藏"
            };
            let favorite =
                NativeMenuItem::with_id(MenuId::new(TOGGLE_FAVORITE_ID), fav_label, true, None);
            menu.append(&favorite).ok();
        }

        // Separator
        menu.append(&PredefinedMenuItem::separator()).ok();

        // Play mode submenu
        let play_mode_menu = Submenu::new("播放模式", true);

        let sequential = CheckMenuItem::with_id(
            MenuId::new(SEQUENTIAL_ID),
            "顺序播放",
            true,
            matches!(state.play_mode, PlayMode::Sequential),
            None,
        );
        let loop_all = CheckMenuItem::with_id(
            MenuId::new(LOOP_ALL_ID),
            "列表循环",
            true,
            matches!(state.play_mode, PlayMode::LoopAll),
            None,
        );
        let loop_one = CheckMenuItem::with_id(
            MenuId::new(LOOP_ONE_ID),
            "单曲循环",
            true,
            matches!(state.play_mode, PlayMode::LoopOne),
            None,
        );
        let shuffle = CheckMenuItem::with_id(
            MenuId::new(SHUFFLE_ID),
            "随机播放",
            true,
            matches!(state.play_mode, PlayMode::Shuffle),
            None,
        );

        play_mode_menu.append(&sequential).ok();
        play_mode_menu.append(&loop_all).ok();
        play_mode_menu.append(&loop_one).ok();
        play_mode_menu.append(&shuffle).ok();

        menu.append(&play_mode_menu).ok();

        // Separator
        menu.append(&PredefinedMenuItem::separator()).ok();

        // Window control
        let toggle_window =
            NativeMenuItem::with_id(MenuId::new(TOGGLE_WINDOW_ID), "显示/隐藏窗口", true, None);
        menu.append(&toggle_window).ok();

        // Separator
        menu.append(&PredefinedMenuItem::separator()).ok();

        // Quit
        let quit = NativeMenuItem::with_id(MenuId::new(QUIT_ID), "退出", true, None);
        menu.append(&quit).ok();

        Ok((menu, play_pause, sequential, loop_all, loop_one, shuffle))
    }

    #[allow(dead_code)]
    fn create_native_menu(state: &TrayState) -> anyhow::Result<Menu> {
        create_native_menu_with_items(state).map(|(menu, _, _, _, _, _)| menu)
    }
}
