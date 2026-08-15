//! Customizable keyboard shortcuts system
//!
//! This module provides a flexible keybinding system that allows users
//! to customize all keyboard shortcuts in the application.

use std::collections::HashMap;

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
    FocusSearch,

    // UI controls
    ToggleQueue,
    ToggleFullscreen,
}

impl Action {
    pub const ALL: [Self; 12] = [
        Self::PlayPause,
        Self::NextTrack,
        Self::PrevTrack,
        Self::VolumeUp,
        Self::VolumeDown,
        Self::VolumeMute,
        Self::SeekForward,
        Self::SeekBackward,
        Self::GoHome,
        Self::FocusSearch,
        Self::ToggleQueue,
        Self::ToggleFullscreen,
    ];
}

/// Identifies whether the local or operating-system global shortcut is edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutScope {
    Local,
    Global,
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

    pub fn primary(mut self) -> Self {
        crate::platform::keybindings::apply_primary_modifier(&mut self.modifiers);
        self
    }

    /// Check if this keybinding matches the given key event
    pub fn matches(&self, key: &Key, modifiers: &Modifiers) -> bool {
        self.key.matches(key) && self.modifiers.matches(modifiers)
    }

    /// Format as human-readable string
    pub fn display(&self) -> String {
        use crate::platform::keybindings::MODIFIER_SYMBOLS;
        let mut parts = Vec::new();

        if self.modifiers.cmd {
            parts.push(MODIFIER_SYMBOLS.cmd);
        }
        if self.modifiers.ctrl {
            parts.push(MODIFIER_SYMBOLS.ctrl);
        }
        if self.modifiers.alt {
            parts.push(MODIFIER_SYMBOLS.alt);
        }
        if self.modifiers.shift {
            parts.push(MODIFIER_SYMBOLS.shift);
        }

        parts.push(self.key.display());
        parts.join("+")
    }
}

/// Set of modifier keys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ModifierSet {
    pub ctrl: bool,
    #[serde(default)]
    pub cmd: bool,
    pub alt: bool,
    pub shift: bool,
}

impl ModifierSet {
    /// Check if modifiers match
    pub fn matches(&self, modifiers: &Modifiers) -> bool {
        let ctrl_match = self.ctrl == modifiers.control();
        let cmd_match = crate::platform::keybindings::matches_cmd_modifier(self.cmd, modifiers);

        ctrl_match && cmd_match && self.alt == modifiers.alt() && self.shift == modifiers.shift()
    }

    /// Check every physical modifier for a global shortcut.
    ///
    /// Application-local shortcuts preserve the existing platform-specific
    /// Command handling, while native global shortcuts allow Super/Win on all
    /// supported desktops and therefore need an exact logo-key comparison.
    fn matches_global(&self, modifiers: &Modifiers) -> bool {
        self.ctrl == modifiers.control()
            && self.cmd == modifiers.logo()
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
                        | (KeyCode::Space, " ")
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
    /// Map from action to application-local keybindings.
    bindings: HashMap<Action, Vec<KeyBinding>>,
    /// Map from action to operating-system global keybindings.
    #[serde(default = "default_global_bindings")]
    global_bindings: HashMap<Action, Vec<KeyBinding>>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            bindings: default_local_bindings(),
            global_bindings: default_global_bindings(),
        }
    }
}

fn default_local_bindings() -> HashMap<Action, Vec<KeyBinding>> {
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
            KeyBinding::new(KeyCode::N).primary(),
            KeyBinding::new(KeyCode::MediaNext),
        ],
    );
    bindings.insert(
        Action::PrevTrack,
        vec![
            KeyBinding::new(KeyCode::P).primary(),
            KeyBinding::new(KeyCode::MediaPrev),
        ],
    );
    bindings.insert(
        Action::VolumeUp,
        vec![
            KeyBinding::new(KeyCode::Up).primary(),
            KeyBinding::new(KeyCode::VolumeUp),
        ],
    );
    bindings.insert(
        Action::VolumeDown,
        vec![
            KeyBinding::new(KeyCode::Down).primary(),
            KeyBinding::new(KeyCode::VolumeDown),
        ],
    );
    bindings.insert(
        Action::VolumeMute,
        vec![
            KeyBinding::new(KeyCode::M).primary(),
            KeyBinding::new(KeyCode::VolumeMute),
        ],
    );
    bindings.insert(
        Action::SeekForward,
        vec![KeyBinding::new(KeyCode::Right).primary()],
    );
    bindings.insert(
        Action::SeekBackward,
        vec![KeyBinding::new(KeyCode::Left).primary()],
    );

    // Navigation
    bindings.insert(Action::GoHome, vec![KeyBinding::new(KeyCode::H).primary()]);
    bindings.insert(
        Action::FocusSearch,
        vec![KeyBinding::new(KeyCode::K).primary()],
    );

    // UI
    bindings.insert(Action::ToggleQueue, vec![KeyBinding::new(KeyCode::Q)]);
    bindings.insert(
        Action::ToggleFullscreen,
        vec![KeyBinding::new(KeyCode::F11)],
    );

    bindings
}

fn default_global_bindings() -> HashMap<Action, Vec<KeyBinding>> {
    let local_bindings = default_local_bindings();

    Action::ALL
        .into_iter()
        .filter_map(|action| {
            local_bindings
                .get(&action)
                .and_then(|bindings| bindings.first())
                .map(|binding| {
                    (
                        action,
                        vec![KeyBinding {
                            modifiers: ModifierSet {
                                ctrl: true,
                                alt: true,
                                ..ModifierSet::default()
                            },
                            key: binding.key.clone(),
                        }],
                    )
                })
        })
        .collect()
}

impl KeyBindings {
    /// Set application-local keybindings for an action.
    pub fn set(&mut self, action: Action, bindings: Vec<KeyBinding>) {
        self.bindings.insert(action, bindings);
    }

    /// Set operating-system global keybindings for an action.
    pub fn set_global(&mut self, action: Action, bindings: Vec<KeyBinding>) {
        self.global_bindings.insert(action, bindings);
    }

    /// Find the action that matches the given key event
    pub fn find_action(&self, key: &Key, modifiers: &Modifiers) -> Option<Action> {
        Self::find_matching_action(&self.bindings, key, modifiers)
    }

    /// Find the global action that matches a focused-window keyboard event.
    ///
    /// This is used to avoid executing both the local and global paths when a
    /// user intentionally configures the same combination for both scopes.
    pub fn find_global_action(&self, key: &Key, modifiers: &Modifiers) -> Option<Action> {
        Action::ALL.into_iter().find(|action| {
            self.global_bindings.get(action).is_some_and(|bindings| {
                bindings.iter().any(|binding| {
                    binding.key.matches(key) && binding.modifiers.matches_global(modifiers)
                })
            })
        })
    }

    /// Get display string for an action's keybinding
    pub fn display_for_action(&self, action: &Action) -> String {
        Self::display_binding(self.local_binding(action))
    }

    /// Get the display string for an action's global keybinding.
    pub fn display_global_for_action(&self, action: &Action) -> String {
        Self::display_binding(self.global_binding(action))
    }

    /// Get the first application-local binding for an action.
    pub fn local_binding(&self, action: &Action) -> Option<&KeyBinding> {
        self.bindings
            .get(action)
            .and_then(|bindings| bindings.first())
    }

    /// Get the first global binding for an action.
    pub fn global_binding(&self, action: &Action) -> Option<&KeyBinding> {
        self.global_bindings
            .get(action)
            .and_then(|bindings| bindings.first())
    }

    /// Iterate configured global bindings in a deterministic action order.
    pub fn configured_global_bindings(&self) -> impl Iterator<Item = (Action, &KeyBinding)> {
        Action::ALL.into_iter().filter_map(|action| {
            self.global_binding(&action)
                .map(|binding| (action, binding))
        })
    }

    fn find_matching_action(
        bindings_by_action: &HashMap<Action, Vec<KeyBinding>>,
        key: &Key,
        modifiers: &Modifiers,
    ) -> Option<Action> {
        Action::ALL.into_iter().find(|action| {
            bindings_by_action.get(action).is_some_and(|bindings| {
                bindings
                    .iter()
                    .any(|binding| binding.matches(key, modifiers))
            })
        })
    }

    fn display_binding(binding: Option<&KeyBinding>) -> String {
        binding
            .map(|b| b.display())
            .unwrap_or_else(|| "None".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_space_binding_matches_character_and_named_space() {
        let bindings = KeyBindings::default();
        let modifiers = Modifiers::default();

        assert_eq!(
            bindings.find_action(&Key::Character(" ".into()), &modifiers),
            Some(Action::PlayPause)
        );
        assert_eq!(
            bindings.find_action(&Key::Named(iced::keyboard::key::Named::Space), &modifiers),
            Some(Action::PlayPause)
        );
    }

    #[test]
    fn test_keybinding_display() {
        let binding = KeyBinding {
            modifiers: ModifierSet {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            key: KeyCode::P,
        };
        assert_eq!(binding.display(), "Ctrl+Shift+P");
    }

    #[test]
    fn default_global_bindings_use_ctrl_alt_and_local_primary_key() {
        let bindings = KeyBindings::default();

        for action in Action::ALL {
            let local = bindings.local_binding(&action).unwrap();
            let global = bindings.global_binding(&action).unwrap();
            assert_eq!(global.key, local.key);
            assert_eq!(
                global.modifiers,
                ModifierSet {
                    ctrl: true,
                    alt: true,
                    ..ModifierSet::default()
                }
            );
        }

        assert_eq!(
            bindings.display_global_for_action(&Action::PlayPause),
            "Ctrl+Alt+Space"
        );
    }

    #[test]
    fn missing_global_bindings_migrate_to_defaults() {
        let mut serialized = serde_json::to_value(KeyBindings::default()).unwrap();
        serialized
            .as_object_mut()
            .unwrap()
            .remove("global_bindings");

        let migrated: KeyBindings = serde_json::from_value(serialized).unwrap();

        assert_eq!(
            migrated.display_global_for_action(&Action::PlayPause),
            "Ctrl+Alt+Space"
        );
        assert_eq!(
            migrated.global_binding(&Action::NextTrack).unwrap().key,
            KeyCode::N
        );
    }

    #[test]
    fn global_matching_compares_the_logo_modifier_exactly() {
        let mut bindings = KeyBindings::default();
        bindings.set_global(
            Action::ToggleQueue,
            vec![KeyBinding {
                modifiers: ModifierSet {
                    cmd: true,
                    ..ModifierSet::default()
                },
                key: KeyCode::Q,
            }],
        );

        assert_eq!(
            bindings.find_global_action(&Key::Character("q".into()), &Modifiers::LOGO),
            Some(Action::ToggleQueue)
        );
        assert_eq!(
            bindings.find_global_action(&Key::Character("q".into()), &Modifiers::default()),
            None
        );
    }
}
