//! Customizable keyboard shortcuts system
//!
//! This module provides a flexible keybinding system that allows users
//! to customize all keyboard shortcuts in the application.

use std::collections::HashMap;
use std::path::Path;

use iced::keyboard::{Key, Modifiers};
use serde::{Deserialize, Serialize};

/// All bindable actions in the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    // Playback controls
    PlayPause,
    NextTrack,
    PrevTrack,
    VolumeUp,
    VolumeDown,
    VolumeMute,
    SeekForward,
    SeekBackward,

    // Navigation
    GoHome,
    GoSearch,

    // UI controls
    ToggleQueue,
    ToggleFullscreen,
}

impl Action {
    /// Get all available actions
    pub fn all() -> &'static [Action] {
        &[
            Action::PlayPause,
            Action::NextTrack,
            Action::PrevTrack,
            Action::VolumeUp,
            Action::VolumeDown,
            Action::VolumeMute,
            Action::SeekForward,
            Action::SeekBackward,
            Action::GoHome,
            Action::GoSearch,
            Action::ToggleQueue,
            Action::ToggleFullscreen,
        ]
    }

    /// Get human-readable name for the action
    pub fn display_name(&self) -> &'static str {
        match self {
            Action::PlayPause => "播放/暂停",
            Action::NextTrack => "下一首",
            Action::PrevTrack => "上一首",
            Action::VolumeUp => "增加音量",
            Action::VolumeDown => "减少音量",
            Action::VolumeMute => "静音",
            Action::SeekForward => "快进",
            Action::SeekBackward => "快退",
            Action::GoHome => "返回首页",
            Action::GoSearch => "搜索",
            Action::ToggleQueue => "显示/隐藏队列",
            Action::ToggleFullscreen => "全屏",
        }
    }
}

/// A keyboard shortcut consisting of modifiers and a key
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyBinding {
    /// Modifier keys (Ctrl, Alt, Shift, etc.)
    pub modifiers: ModifierSet,
    /// The main key
    pub key: KeyCode,
}

impl KeyBinding {
    /// Create a new keybinding
    pub fn new(key: KeyCode) -> Self {
        Self {
            modifiers: ModifierSet::default(),
            key,
        }
    }

    /// Add Ctrl modifier
    pub fn ctrl(mut self) -> Self {
        self.modifiers.ctrl = true;
        self
    }

    /// Add Shift modifier
    pub fn shift(mut self) -> Self {
        self.modifiers.shift = true;
        self
    }

    /// Add Alt modifier
    pub fn alt(mut self) -> Self {
        self.modifiers.alt = true;
        self
    }

    /// Check if this keybinding matches the given key event
    pub fn matches(&self, key: &Key, modifiers: &Modifiers) -> bool {
        self.key.matches(key) && self.modifiers.matches(modifiers)
    }

    /// Format as human-readable string
    pub fn display(&self) -> String {
        let mut parts = Vec::new();

        if self.modifiers.ctrl {
            parts.push("Ctrl");
        }
        if self.modifiers.alt {
            parts.push("Alt");
        }
        if self.modifiers.shift {
            parts.push("Shift");
        }

        parts.push(self.key.display());
        parts.join("+")
    }
}

/// Set of modifier keys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ModifierSet {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl ModifierSet {
    /// Check if modifiers match
    pub fn matches(&self, modifiers: &Modifiers) -> bool {
        self.ctrl == modifiers.control()
            && self.alt == modifiers.alt()
            && self.shift == modifiers.shift()
    }
}

/// Supported key codes for binding
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyCode {
    // Letters
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    // Numbers
    Key0,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,

    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // Navigation
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,

    // Special
    Space,
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,

    // Media keys
    MediaPlayPause,
    MediaNext,
    MediaPrev,
    VolumeUp,
    VolumeDown,
    VolumeMute,
}

impl KeyCode {
    /// Check if this key code matches an iced Key
    pub fn matches(&self, key: &Key) -> bool {
        match key {
            Key::Character(c) => {
                let c = c.to_lowercase();
                matches!(
                    (self, c.as_str()),
                    (KeyCode::A, "a")
                        | (KeyCode::B, "b")
                        | (KeyCode::C, "c")
                        | (KeyCode::D, "d")
                        | (KeyCode::E, "e")
                        | (KeyCode::F, "f")
                        | (KeyCode::G, "g")
                        | (KeyCode::H, "h")
                        | (KeyCode::I, "i")
                        | (KeyCode::J, "j")
                        | (KeyCode::K, "k")
                        | (KeyCode::L, "l")
                        | (KeyCode::M, "m")
                        | (KeyCode::N, "n")
                        | (KeyCode::O, "o")
                        | (KeyCode::P, "p")
                        | (KeyCode::Q, "q")
                        | (KeyCode::R, "r")
                        | (KeyCode::S, "s")
                        | (KeyCode::T, "t")
                        | (KeyCode::U, "u")
                        | (KeyCode::V, "v")
                        | (KeyCode::W, "w")
                        | (KeyCode::X, "x")
                        | (KeyCode::Y, "y")
                        | (KeyCode::Z, "z")
                        | (KeyCode::Key0, "0")
                        | (KeyCode::Key1, "1")
                        | (KeyCode::Key2, "2")
                        | (KeyCode::Key3, "3")
                        | (KeyCode::Key4, "4")
                        | (KeyCode::Key5, "5")
                        | (KeyCode::Key6, "6")
                        | (KeyCode::Key7, "7")
                        | (KeyCode::Key8, "8")
                        | (KeyCode::Key9, "9")
                )
            }
            Key::Named(named) => {
                use iced::keyboard::key::Named;
                matches!(
                    (self, named),
                    (KeyCode::Space, Named::Space)
                        | (KeyCode::Enter, Named::Enter)
                        | (KeyCode::Escape, Named::Escape)
                        | (KeyCode::Tab, Named::Tab)
                        | (KeyCode::Backspace, Named::Backspace)
                        | (KeyCode::Delete, Named::Delete)
                        | (KeyCode::Up, Named::ArrowUp)
                        | (KeyCode::Down, Named::ArrowDown)
                        | (KeyCode::Left, Named::ArrowLeft)
                        | (KeyCode::Right, Named::ArrowRight)
                        | (KeyCode::Home, Named::Home)
                        | (KeyCode::End, Named::End)
                        | (KeyCode::PageUp, Named::PageUp)
                        | (KeyCode::PageDown, Named::PageDown)
                        | (KeyCode::F1, Named::F1)
                        | (KeyCode::F2, Named::F2)
                        | (KeyCode::F3, Named::F3)
                        | (KeyCode::F4, Named::F4)
                        | (KeyCode::F5, Named::F5)
                        | (KeyCode::F6, Named::F6)
                        | (KeyCode::F7, Named::F7)
                        | (KeyCode::F8, Named::F8)
                        | (KeyCode::F9, Named::F9)
                        | (KeyCode::F10, Named::F10)
                        | (KeyCode::F11, Named::F11)
                        | (KeyCode::F12, Named::F12)
                        | (KeyCode::MediaPlayPause, Named::MediaPlayPause)
                        | (KeyCode::MediaNext, Named::MediaTrackNext)
                        | (KeyCode::MediaPrev, Named::MediaTrackPrevious)
                        | (KeyCode::VolumeUp, Named::AudioVolumeUp)
                        | (KeyCode::VolumeDown, Named::AudioVolumeDown)
                        | (KeyCode::VolumeMute, Named::AudioVolumeMute)
                )
            }
            Key::Unidentified => false,
        }
    }

    /// Get display name for the key
    pub fn display(&self) -> &'static str {
        match self {
            KeyCode::A => "A",
            KeyCode::B => "B",
            KeyCode::C => "C",
            KeyCode::D => "D",
            KeyCode::E => "E",
            KeyCode::F => "F",
            KeyCode::G => "G",
            KeyCode::H => "H",
            KeyCode::I => "I",
            KeyCode::J => "J",
            KeyCode::K => "K",
            KeyCode::L => "L",
            KeyCode::M => "M",
            KeyCode::N => "N",
            KeyCode::O => "O",
            KeyCode::P => "P",
            KeyCode::Q => "Q",
            KeyCode::R => "R",
            KeyCode::S => "S",
            KeyCode::T => "T",
            KeyCode::U => "U",
            KeyCode::V => "V",
            KeyCode::W => "W",
            KeyCode::X => "X",
            KeyCode::Y => "Y",
            KeyCode::Z => "Z",
            KeyCode::Key0 => "0",
            KeyCode::Key1 => "1",
            KeyCode::Key2 => "2",
            KeyCode::Key3 => "3",
            KeyCode::Key4 => "4",
            KeyCode::Key5 => "5",
            KeyCode::Key6 => "6",
            KeyCode::Key7 => "7",
            KeyCode::Key8 => "8",
            KeyCode::Key9 => "9",
            KeyCode::F1 => "F1",
            KeyCode::F2 => "F2",
            KeyCode::F3 => "F3",
            KeyCode::F4 => "F4",
            KeyCode::F5 => "F5",
            KeyCode::F6 => "F6",
            KeyCode::F7 => "F7",
            KeyCode::F8 => "F8",
            KeyCode::F9 => "F9",
            KeyCode::F10 => "F10",
            KeyCode::F11 => "F11",
            KeyCode::F12 => "F12",
            KeyCode::Up => "↑",
            KeyCode::Down => "↓",
            KeyCode::Left => "←",
            KeyCode::Right => "→",
            KeyCode::Home => "Home",
            KeyCode::End => "End",
            KeyCode::PageUp => "PageUp",
            KeyCode::PageDown => "PageDown",
            KeyCode::Space => "Space",
            KeyCode::Enter => "Enter",
            KeyCode::Escape => "Esc",
            KeyCode::Tab => "Tab",
            KeyCode::Backspace => "Backspace",
            KeyCode::Delete => "Delete",
            KeyCode::MediaPlayPause => "Media Play",
            KeyCode::MediaNext => "Media Next",
            KeyCode::MediaPrev => "Media Prev",
            KeyCode::VolumeUp => "Vol+",
            KeyCode::VolumeDown => "Vol-",
            KeyCode::VolumeMute => "Mute",
        }
    }
}

/// The keybindings configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBindings {
    /// Map from action to keybinding
    bindings: HashMap<Action, Vec<KeyBinding>>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        let mut bindings = HashMap::new();

        // Default keybindings
        // Playback
        bindings.insert(
            Action::PlayPause,
            vec![
                KeyBinding::new(KeyCode::Space),
                KeyBinding::new(KeyCode::MediaPlayPause),
            ],
        );
        bindings.insert(
            Action::NextTrack,
            vec![
                KeyBinding::new(KeyCode::N).ctrl(),
                KeyBinding::new(KeyCode::MediaNext),
            ],
        );
        bindings.insert(
            Action::PrevTrack,
            vec![
                KeyBinding::new(KeyCode::P).ctrl(),
                KeyBinding::new(KeyCode::MediaPrev),
            ],
        );
        bindings.insert(
            Action::VolumeUp,
            vec![
                KeyBinding::new(KeyCode::Up).ctrl(),
                KeyBinding::new(KeyCode::VolumeUp),
            ],
        );
        bindings.insert(
            Action::VolumeDown,
            vec![
                KeyBinding::new(KeyCode::Down).ctrl(),
                KeyBinding::new(KeyCode::VolumeDown),
            ],
        );
        bindings.insert(
            Action::VolumeMute,
            vec![
                KeyBinding::new(KeyCode::M).ctrl(),
                KeyBinding::new(KeyCode::VolumeMute),
            ],
        );
        bindings.insert(
            Action::SeekForward,
            vec![KeyBinding::new(KeyCode::Right).ctrl()],
        );
        bindings.insert(
            Action::SeekBackward,
            vec![KeyBinding::new(KeyCode::Left).ctrl()],
        );

        // Navigation
        bindings.insert(Action::GoHome, vec![KeyBinding::new(KeyCode::H).ctrl()]);
        bindings.insert(Action::GoSearch, vec![KeyBinding::new(KeyCode::K).ctrl()]);

        // UI
        bindings.insert(Action::ToggleQueue, vec![KeyBinding::new(KeyCode::Q)]);
        bindings.insert(
            Action::ToggleFullscreen,
            vec![KeyBinding::new(KeyCode::F11)],
        );

        Self { bindings }
    }
}

impl KeyBindings {
    /// Create empty keybindings
    pub fn empty() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    /// Load keybindings from a JSON file
    pub fn load_from_file(path: &Path) -> Result<Self, KeyBindingsError> {
        let content =
            std::fs::read_to_string(path).map_err(|e| KeyBindingsError::Io(e.to_string()))?;
        let bindings: Self =
            serde_json::from_str(&content).map_err(|e| KeyBindingsError::Parse(e.to_string()))?;
        Ok(bindings)
    }

    /// Save keybindings to a JSON file
    pub fn save_to_file(&self, path: &Path) -> Result<(), KeyBindingsError> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| KeyBindingsError::Parse(e.to_string()))?;
        std::fs::write(path, content).map_err(|e| KeyBindingsError::Io(e.to_string()))?;
        Ok(())
    }

    /// Get the keybindings for an action
    pub fn get(&self, action: &Action) -> Option<&Vec<KeyBinding>> {
        self.bindings.get(action)
    }

    /// Set keybindings for an action
    pub fn set(&mut self, action: Action, bindings: Vec<KeyBinding>) {
        self.bindings.insert(action, bindings);
    }

    /// Add a keybinding for an action
    pub fn add(&mut self, action: Action, binding: KeyBinding) {
        self.bindings.entry(action).or_default().push(binding);
    }

    /// Remove all keybindings for an action
    pub fn clear(&mut self, action: &Action) {
        self.bindings.remove(action);
    }

    /// Find the action that matches the given key event
    pub fn find_action(&self, key: &Key, modifiers: &Modifiers) -> Option<Action> {
        for (action, bindings) in &self.bindings {
            for binding in bindings {
                if binding.matches(key, modifiers) {
                    return Some(*action);
                }
            }
        }
        None
    }

    /// Get display string for an action's keybinding
    pub fn display_for_action(&self, action: &Action) -> String {
        self.bindings
            .get(action)
            .and_then(|b| b.first())
            .map(|b| b.display())
            .unwrap_or_else(|| "None".to_string())
    }
}

/// Errors that can occur with keybindings
#[derive(Debug, Clone)]
pub enum KeyBindingsError {
    Io(String),
    Parse(String),
}

impl std::fmt::Display for KeyBindingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyBindingsError::Io(e) => write!(f, "IO error: {}", e),
            KeyBindingsError::Parse(e) => write!(f, "Parse error: {}", e),
        }
    }
}

impl std::error::Error for KeyBindingsError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_bindings() {
        let bindings = KeyBindings::default();
        assert!(bindings.get(&Action::PlayPause).is_some());
    }

    #[test]
    fn test_keybinding_display() {
        let binding = KeyBinding::new(KeyCode::P).ctrl().shift();
        assert_eq!(binding.display(), "Ctrl+Shift+P");
    }
}
