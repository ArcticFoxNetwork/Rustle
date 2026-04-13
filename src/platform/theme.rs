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

/// Configures the global `iced` font database so generic sans-serif text
/// resolves to the intended platform UI family.
pub fn configure_iced_font_system() {
    #[cfg(target_os = "macos")]
    configure_iced_sans_serif_family(".SF NS");

    #[cfg(target_os = "windows")]
    configure_iced_sans_serif_family("Segoe UI");
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn configure_iced_sans_serif_family(family: &'static str) {
    let mut font_system = iced::advanced::graphics::text::font_system()
        .write()
        .expect("lock iced font system");

    font_system.raw().db_mut().set_sans_serif_family(family);
}
