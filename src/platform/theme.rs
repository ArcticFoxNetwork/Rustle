//! Platform-specific theme constants
//!
//! Provides font weights and other theme values that vary by platform.

use iced::font::Weight;

/// Bold font weight
/// - macOS: Semibold (SF Pro looks better with Semibold)
/// - Linux/Windows: Bold
/// - WASM: Bold (browser default)
#[cfg(target_os = "macos")]
pub const BOLD_WEIGHT: Weight = Weight::Semibold;

#[cfg(all(not(target_os = "macos"), not(target_arch = "wasm32")))]
pub const BOLD_WEIGHT: Weight = Weight::Bold;

#[cfg(target_arch = "wasm32")]
pub const BOLD_WEIGHT: Weight = Weight::Bold;

/// Medium font weight
/// - macOS: Medium
/// - Linux/Windows: Normal
/// - WASM: Normal (browser default)
#[cfg(target_os = "macos")]
pub const MEDIUM_WEIGHT: Weight = Weight::Medium;

#[cfg(all(not(target_os = "macos"), not(target_arch = "wasm32")))]
pub const MEDIUM_WEIGHT: Weight = Weight::Normal;

#[cfg(target_arch = "wasm32")]
pub const MEDIUM_WEIGHT: Weight = Weight::Normal;
