//! System tray abstraction
//!
//! Provides a unified interface for system tray functionality across platforms.
//!
//! - Linux: Uses ksni (freedesktop StatusNotifierItem)
//! - Windows/macOS: Uses tray-icon
//! - WASM: No-op (not available)

use crate::features::PlayMode;
use tokio::sync::mpsc;

// Platform-specific implementations
#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "windows", target_os = "macos"))]
mod windows;

/// Commands that can be sent from the tray to the application
#[derive(Debug, Clone)]
pub enum TrayCommand {
    /// Show window and bring to front (left click behavior for Windows/macOS)
    ShowOrFocusWindow,
    /// Toggle show/hide window (Linux behavior)
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
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    _phantom: (),
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

        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            let _ = state; // Suppress unused warning
        }
    }
}

/// Result type for tray initialization
pub type TrayResult = std::sync::Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<TrayCommand>>>;

/// Global tray handle for updates
static TRAY_HANDLE: once_cell::sync::OnceCell<TrayHandle> = once_cell::sync::OnceCell::new();

/// Get the global tray handle
pub fn get_handle() -> Option<&'static TrayHandle> {
    TRAY_HANDLE.get()
}

/// Start the system tray service
/// Returns a handle to control the tray and a receiver for commands
#[cfg(target_os = "linux")]
async fn start_tray() -> anyhow::Result<(TrayHandle, mpsc::UnboundedReceiver<TrayCommand>)> {
    linux::start_linux_tray().await
}

/// Start the system tray service synchronously (Windows/macOS only)
/// Returns a handle to control the tray and a receiver for commands
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn start_tray_sync() -> anyhow::Result<(TrayHandle, mpsc::UnboundedReceiver<TrayCommand>)> {
    windows::start_native_tray_sync()
}

/// Initialize tray and store handle globally (Linux - async)
#[cfg(target_os = "linux")]
async fn init_tray_internal() -> anyhow::Result<TrayResult> {
    let (handle, rx) = start_tray().await?;
    TRAY_HANDLE.set(handle).ok();
    tracing::info!("System tray started");
    Ok(std::sync::Arc::new(tokio::sync::Mutex::new(rx)))
}

/// Initialize tray and store handle globally (Windows/macOS - sync)
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn init_tray_internal() -> anyhow::Result<TrayResult> {
    let (handle, rx) = start_tray_sync()?;
    TRAY_HANDLE.set(handle).ok();
    tracing::info!("System tray started");
    Ok(std::sync::Arc::new(tokio::sync::Mutex::new(rx)))
}

/// Initialize tray (WASM/unsupported - no-op)
#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn init_tray_internal() -> anyhow::Result<TrayResult> {
    let (_tx, rx) = mpsc::unbounded_channel();
    Ok(std::sync::Arc::new(tokio::sync::Mutex::new(rx)))
}

/// Create an iced Task that initializes the system tray
/// 
/// This is the unified entry point - it handles platform differences internally:
/// - Linux: Uses Task::perform (async)
/// - Windows/macOS: Uses Task::done (sync, must run on main thread)
/// - WASM: No-op
pub fn init_task<F>(on_success: F) -> iced::Task<crate::app::Message>
where
    F: FnOnce(TrayResult) -> crate::app::Message + Send + 'static,
{
    #[cfg(target_os = "linux")]
    {
        iced::Task::perform(init_tray_internal(), move |result| match result {
            Ok(rx) => on_success(rx),
            Err(e) => {
                tracing::warn!("Failed to start system tray: {}", e);
                crate::app::Message::Noop
            }
        })
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        iced::Task::done(match init_tray_internal() {
            Ok(rx) => on_success(rx),
            Err(e) => {
                tracing::warn!("Failed to start system tray: {}", e);
                crate::app::Message::Noop
            }
        })
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        iced::Task::done(match init_tray_internal() {
            Ok(rx) => on_success(rx),
            Err(_) => crate::app::Message::Noop,
        })
    }
}

/// Check if system tray is available on this platform
pub fn is_available() -> bool {
    cfg!(any(target_os = "linux", target_os = "windows", target_os = "macos"))
}
