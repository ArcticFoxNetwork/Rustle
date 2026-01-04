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

pub mod keybindings;
pub mod media_controls;
pub mod theme;
pub mod tray;
pub mod window;
