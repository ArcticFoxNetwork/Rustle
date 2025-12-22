//! Feature modules - business logic separated from UI
//!
//! Each feature module contains the core logic for a specific functionality.
//! Features should not depend on UI components directly.

pub mod import;
pub mod keybindings;
pub mod lyrics;
pub mod media;
#[cfg(target_os = "linux")]
pub mod mpris;
pub mod settings;
#[cfg(target_os = "linux")]
pub mod tray;

pub use keybindings::{Action, KeyBindings};

#[cfg(target_os = "linux")]
pub use mpris::{MprisCommand, MprisHandle, MprisMetadata, MprisPlaybackStatus, MprisState};
pub use settings::{CloseBehavior, EqualizerPreset, MusicQuality, PlayMode, ProxyType, Settings};
#[cfg(target_os = "linux")]
pub use tray::{TrayCommand, TrayHandle, TrayState};
