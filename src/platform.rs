//! Platform abstraction layer
//!
//! This module provides unified interfaces for platform-specific functionality,
//! organized by feature with platform implementations inside each feature module.
//!
//! # Structure
//! - `tray/` - System tray functionality
//! - `media_controls/` - Media control integration (MPRIS on Linux)
//! - `window/` - Window behavior differences
//! - `theme.rs` - Platform-specific theme constants
//! - `keybindings.rs` - Keybinding display format

pub const APP_BINARY_NAME: &str = "rustle";
pub const APP_DISPLAY_NAME: &str = "Rustle";
pub const APP_ID: &str = "life.fxs.rustle";

pub mod discord;
pub mod keybindings;
pub mod media_controls;
pub mod protocol;
pub mod theme;
pub mod tray;
pub mod window;

pub fn init() {
    theme::configure_iced_font_system();
    window::initialize_process();
}

/// Open the parent directory of the given file path in the system file manager.
pub fn open_in_file_manager(path: &std::path::Path) {
    let dir = path.parent().unwrap_or(path);
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer")
            .arg("/select,")
            .arg(path)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn();
    }
}
