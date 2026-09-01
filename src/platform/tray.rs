//! System tray abstraction.
//!
//! The platform backends intentionally have different ownership models:
//! Linux delegates to the StatusNotifierItem service, Windows owns Win32
//! objects on the Iced/Winit event-loop thread, and macOS owns its status item
//! on that same UI thread.

use crate::features::PlayMode;
use crate::i18n::Language;
use std::sync::OnceLock;
use tokio::sync::mpsc;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

const COMMAND_CHANNEL_CAPACITY: usize = 32;

/// Runtime availability of the tray recovery surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayAvailability {
    Starting,
    Available,
    Unavailable(String),
}

impl TrayAvailability {
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }
}

/// Commands sent from the native tray to the application.
#[derive(Debug, Clone)]
pub enum TrayCommand {
    Window(TrayWindowCommand),
    PlayPause,
    NextTrack,
    PrevTrack,
    SetPlayMode(PlayMode),
    ToggleFavorite,
    Quit,
    /// The shell integration disappeared or recovered at runtime.
    AvailabilityChanged(TrayAvailability),
}

#[derive(Debug, Clone, Copy)]
pub enum TrayWindowCommand {
    PrimaryActivation,
    Toggle,
}

impl TrayWindowCommand {
    pub fn resolve_message<Message>(self, show_or_focus: Message, toggle: Message) -> Message {
        match self {
            Self::Toggle => toggle,
            Self::PrimaryActivation => primary_activation_message(show_or_focus, toggle),
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn primary_activation_message<Message>(show_or_focus: Message, _toggle: Message) -> Message {
    show_or_focus
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn primary_activation_message<Message>(_show_or_focus: Message, toggle: Message) -> Message {
    toggle
}

/// Latest playback snapshot projected by each platform backend.
#[derive(Debug, Clone)]
pub struct TrayState {
    pub is_playing: bool,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub play_mode: PlayMode,
    pub ncm_song_id: Option<u64>,
    pub is_favorited: bool,
    pub language: Language,
}

impl TrayState {
    pub fn new(language: Language) -> Self {
        Self {
            language,
            ..Self::default()
        }
    }
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
            language: Language::default(),
        }
    }
}

/// Lightweight application-side handle. Native objects never live here.
#[derive(Clone)]
pub struct TrayHandle {
    #[cfg(target_os = "linux")]
    handle: ksni::Handle<linux::LinuxTray>,
    #[cfg(not(target_os = "linux"))]
    _private: (),
}

#[cfg(target_os = "linux")]
impl TrayHandle {
    /// Submit the newest state to the asynchronous StatusNotifierItem service.
    pub fn update(&self, state: TrayState) {
        let handle = self.handle.clone();
        tokio::spawn(async move {
            if handle
                .update(|tray| tray.update_state(state))
                .await
                .is_none()
            {
                tracing::warn!("Linux system tray service stopped before state update");
            }
        });
    }
}

#[cfg(target_os = "windows")]
impl TrayHandle {
    /// Apply the newest state synchronously on the Win32/Winit UI thread.
    pub fn update(&self, state: TrayState) {
        if let Err(error) = windows::update_state(state) {
            tracing::warn!(%error, "Failed to update Windows system tray state");
        }
    }
}

#[cfg(target_os = "macos")]
impl TrayHandle {
    /// Apply the newest state synchronously on the AppKit main thread.
    pub fn update(&self, state: TrayState) {
        if let Err(error) = macos::update_state(state) {
            tracing::warn!(%error, "Failed to update macOS status item state");
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
impl TrayHandle {
    pub fn update(&self, state: TrayState) {
        let _ = state;
    }
}

pub type TrayResult = std::sync::Arc<tokio::sync::Mutex<mpsc::Receiver<TrayCommand>>>;

static TRAY_HANDLE: OnceLock<TrayHandle> = OnceLock::new();

pub fn get_handle() -> Option<&'static TrayHandle> {
    TRAY_HANDLE.get()
}

/// Confirm that the native recovery surface is currently registered.
///
/// Application state is still used for diagnostics, but the close-to-tray
/// path also consults this live value so a delayed/dropped lifecycle event
/// cannot hide the final window after the shell integration has failed.
pub fn is_available() -> bool {
    #[cfg(target_os = "windows")]
    {
        windows::is_available()
    }

    #[cfg(target_os = "macos")]
    {
        macos::is_available()
    }

    #[cfg(target_os = "linux")]
    {
        TRAY_HANDLE
            .get()
            .is_some_and(|handle| !handle.handle.is_closed())
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        false
    }
}

#[cfg(target_os = "linux")]
async fn init_tray_internal(language: Language) -> anyhow::Result<TrayResult> {
    let (handle, rx) = linux::start_linux_tray(language, COMMAND_CHANNEL_CAPACITY).await?;
    TRAY_HANDLE
        .set(handle)
        .map_err(|_| anyhow::anyhow!("system tray was already initialized"))?;
    tracing::info!("System tray started");
    Ok(std::sync::Arc::new(tokio::sync::Mutex::new(rx)))
}

#[cfg(target_os = "windows")]
fn init_tray_internal(language: Language) -> anyhow::Result<TrayResult> {
    let (handle, rx) = windows::start_windows_tray(language, COMMAND_CHANNEL_CAPACITY)?;
    TRAY_HANDLE
        .set(handle)
        .map_err(|_| anyhow::anyhow!("system tray was already initialized"))?;
    tracing::info!("Windows system tray started");
    Ok(std::sync::Arc::new(tokio::sync::Mutex::new(rx)))
}

#[cfg(target_os = "macos")]
fn init_tray_internal(language: Language) -> anyhow::Result<TrayResult> {
    let (handle, rx) = macos::start_macos_tray(language, COMMAND_CHANNEL_CAPACITY)?;
    TRAY_HANDLE
        .set(handle)
        .map_err(|_| anyhow::anyhow!("system tray was already initialized"))?;
    tracing::info!("macOS status item started");
    Ok(std::sync::Arc::new(tokio::sync::Mutex::new(rx)))
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn init_tray_internal(_language: Language) -> anyhow::Result<TrayResult> {
    let (_tx, rx) = mpsc::channel(1);
    Ok(std::sync::Arc::new(tokio::sync::Mutex::new(rx)))
}

/// Initialize the native integration after the first window-open event.
/// That event is the earliest common point at which the Winit loop is active.
pub fn init_task<F>(language: Language, on_success: F) -> iced::Task<crate::app::Message>
where
    F: FnOnce(TrayResult) -> crate::app::Message + Send + 'static,
{
    #[cfg(target_os = "linux")]
    {
        iced::Task::perform(init_tray_internal(language), move |result| match result {
            Ok(rx) => on_success(rx),
            Err(error) => {
                tracing::warn!(%error, "Failed to start system tray");
                crate::app::Message::TrayUnavailable(error.to_string())
            }
        })
    }

    #[cfg(not(target_os = "linux"))]
    {
        iced::Task::done(match init_tray_internal(language) {
            Ok(rx) => on_success(rx),
            Err(error) => {
                tracing::warn!(%error, "Failed to start system tray");
                crate::app::Message::TrayUnavailable(error.to_string())
            }
        })
    }
}

/// Update native labels immediately when the application language changes.
pub fn set_language(language: Language) {
    #[cfg(target_os = "windows")]
    if let Err(error) = windows::set_language(language) {
        tracing::debug!(%error, "Windows tray language update skipped");
    }

    #[cfg(target_os = "macos")]
    if let Err(error) = macos::set_language(language) {
        tracing::debug!(%error, "macOS tray language update skipped");
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let _ = language;
}

/// Explicitly release native resources while the UI thread is still alive.
pub fn shutdown() {
    #[cfg(target_os = "windows")]
    windows::shutdown();

    #[cfg(target_os = "macos")]
    macos::shutdown();
}
