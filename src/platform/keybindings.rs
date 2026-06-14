//! Platform-specific keybinding display
//!
//! Provides modifier key symbols and display functions that vary by platform.

use iced::keyboard::Modifiers;

/// Modifier key symbols for display
pub struct ModifierSymbols {
    pub ctrl: &'static str,
    pub alt: &'static str,
    pub shift: &'static str,
    pub cmd: &'static str,
}

/// Platform-specific modifier symbols
#[cfg(target_os = "macos")]
pub const MODIFIER_SYMBOLS: ModifierSymbols = ModifierSymbols {
    ctrl: "⌃",
    alt: "⌥",
    shift: "⇧",
    cmd: "⌘",
};

#[cfg(not(target_os = "macos"))]
pub const MODIFIER_SYMBOLS: ModifierSymbols = ModifierSymbols {
    ctrl: "Ctrl",
    alt: "Alt",
    shift: "Shift",
    cmd: "Win", // Windows key on non-macOS
};

/// Apply the primary modifier to a ModifierSet
/// - macOS: Sets cmd = true
/// - Others: Sets ctrl = true
#[cfg(target_os = "macos")]
pub fn apply_primary_modifier(modifiers: &mut crate::features::keybindings::ModifierSet) {
    modifiers.cmd = true;
}

#[cfg(not(target_os = "macos"))]
pub fn apply_primary_modifier(modifiers: &mut crate::features::keybindings::ModifierSet) {
    modifiers.ctrl = true;
}

/// Check if the cmd modifier matches
/// - macOS: Checks if cmd flag matches logo key state
/// - Others: cmd should not be set (returns true only if cmd is false)
#[cfg(target_os = "macos")]
pub fn matches_cmd_modifier(cmd: bool, modifiers: &Modifiers) -> bool {
    cmd == modifiers.logo()
}

#[cfg(not(target_os = "macos"))]
pub fn matches_cmd_modifier(cmd: bool, _modifiers: &Modifiers) -> bool {
    // On non-macOS, cmd should not be set
    !cmd
}
