//! Feature modules - business logic separated from UI
//!
//! Each feature module contains the core logic for a specific functionality.
//! Features should not depend on UI components directly.

pub mod import;
pub mod keybindings;
pub mod lyrics;
pub mod media;
pub mod settings;

pub use keybindings::{Action, KeyBindings};

pub use crate::platform::tray::TrayCommand;

pub use settings::{CloseBehavior, EqualizerPreset, MusicQuality, PlayMode, ProxyType, Settings};
