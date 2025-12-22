//! Settings update handlers

use crate::app::SettingsSection;
use crate::app::message::Message;
use crate::app::state::App;
use crate::cache;
use crate::features::keybindings::{KeyBinding, KeyCode, ModifierSet};
use iced::Task;
use iced::keyboard::Key;

/// Section positions when user is NOT logged in
const SECTION_POSITIONS_LOGGED_OUT: [(SettingsSection, f32); 8] = [
    (SettingsSection::Account, 0.0),
    (SettingsSection::Playback, 150.0),
    (SettingsSection::Display, 500.0),
    (SettingsSection::System, 850.0),
    (SettingsSection::Network, 1000.0),
    (SettingsSection::Storage, 1150.0),
    (SettingsSection::Shortcuts, 1390.0),
    (SettingsSection::About, 1965.0),
];

/// Offset to add when user IS logged in (Account section is larger)
const LOGGED_IN_OFFSET: f32 = 80.0;

/// Get scroll position for a section based on login state
fn get_section_scroll_position(section: SettingsSection, is_logged_in: bool) -> f32 {
    let base_pos = SECTION_POSITIONS_LOGGED_OUT
        .iter()
        .find(|(s, _)| *s == section)
        .map(|(_, pos)| *pos)
        .unwrap_or(0.0);

    // Add offset for logged in users (except Account which stays at 0)
    if is_logged_in && section != SettingsSection::Account {
        base_pos + LOGGED_IN_OFFSET
    } else {
        base_pos
    }
}

/// Get section from scroll position based on login state
fn get_section_from_scroll_position(y_offset: f32, is_logged_in: bool) -> SettingsSection {
    // Adjust offset for logged in state
    let adjusted_y = if is_logged_in {
        y_offset - LOGGED_IN_OFFSET
    } else {
        y_offset
    };

    // Add a small offset (50px) to trigger section change slightly before reaching it
    let search_offset = adjusted_y + 50.0;

    let mut current_section = SettingsSection::Account;
    for (section, pos) in SECTION_POSITIONS_LOGGED_OUT.iter() {
        if search_offset >= *pos {
            current_section = *section;
        } else {
            break;
        }
    }
    current_section
}

/// Convert iced Key to our KeyCode
fn key_to_keycode(key: &Key) -> Option<KeyCode> {
    match key {
        Key::Character(c) => {
            let c = c.to_lowercase();
            match c.as_str() {
                "a" => Some(KeyCode::A),
                "b" => Some(KeyCode::B),
                "c" => Some(KeyCode::C),
                "d" => Some(KeyCode::D),
                "e" => Some(KeyCode::E),
                "f" => Some(KeyCode::F),
                "g" => Some(KeyCode::G),
                "h" => Some(KeyCode::H),
                "i" => Some(KeyCode::I),
                "j" => Some(KeyCode::J),
                "k" => Some(KeyCode::K),
                "l" => Some(KeyCode::L),
                "m" => Some(KeyCode::M),
                "n" => Some(KeyCode::N),
                "o" => Some(KeyCode::O),
                "p" => Some(KeyCode::P),
                "q" => Some(KeyCode::Q),
                "r" => Some(KeyCode::R),
                "s" => Some(KeyCode::S),
                "t" => Some(KeyCode::T),
                "u" => Some(KeyCode::U),
                "v" => Some(KeyCode::V),
                "w" => Some(KeyCode::W),
                "x" => Some(KeyCode::X),
                "y" => Some(KeyCode::Y),
                "z" => Some(KeyCode::Z),
                "0" => Some(KeyCode::Key0),
                "1" => Some(KeyCode::Key1),
                "2" => Some(KeyCode::Key2),
                "3" => Some(KeyCode::Key3),
                "4" => Some(KeyCode::Key4),
                "5" => Some(KeyCode::Key5),
                "6" => Some(KeyCode::Key6),
                "7" => Some(KeyCode::Key7),
                "8" => Some(KeyCode::Key8),
                "9" => Some(KeyCode::Key9),
                _ => None,
            }
        }
        Key::Named(named) => {
            use iced::keyboard::key::Named;
            match named {
                Named::Space => Some(KeyCode::Space),
                Named::Enter => Some(KeyCode::Enter),
                Named::Escape => Some(KeyCode::Escape),
                Named::Tab => Some(KeyCode::Tab),
                Named::Backspace => Some(KeyCode::Backspace),
                Named::Delete => Some(KeyCode::Delete),
                Named::ArrowUp => Some(KeyCode::Up),
                Named::ArrowDown => Some(KeyCode::Down),
                Named::ArrowLeft => Some(KeyCode::Left),
                Named::ArrowRight => Some(KeyCode::Right),
                Named::Home => Some(KeyCode::Home),
                Named::End => Some(KeyCode::End),
                Named::PageUp => Some(KeyCode::PageUp),
                Named::PageDown => Some(KeyCode::PageDown),
                Named::F1 => Some(KeyCode::F1),
                Named::F2 => Some(KeyCode::F2),
                Named::F3 => Some(KeyCode::F3),
                Named::F4 => Some(KeyCode::F4),
                Named::F5 => Some(KeyCode::F5),
                Named::F6 => Some(KeyCode::F6),
                Named::F7 => Some(KeyCode::F7),
                Named::F8 => Some(KeyCode::F8),
                Named::F9 => Some(KeyCode::F9),
                Named::F10 => Some(KeyCode::F10),
                Named::F11 => Some(KeyCode::F11),
                Named::F12 => Some(KeyCode::F12),
                _ => None,
            }
        }
        Key::Unidentified => None,
    }
}

impl App {
    /// Handle settings-related messages
    pub fn handle_settings(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::UpdateCloseBehavior(behavior) => {
                self.core.settings.close_behavior = *behavior;
                Some(Task::perform(async { Message::SaveSettings }, |m| m))
            }
            Message::UpdateFadeInOut(enabled) => {
                self.core.settings.playback.fade_in_out = *enabled;
                Some(Task::perform(async { Message::SaveSettings }, |m| m))
            }
            Message::UpdateVolumeNormalization(enabled) => {
                self.core.settings.playback.volume_normalization = *enabled;
                Some(Task::perform(async { Message::SaveSettings }, |m| m))
            }
            Message::UpdateMusicQuality(quality) => {
                self.core.settings.playback.music_quality = *quality;
                // Update NcmClient's quality setting
                if let Some(client) = &self.core.ncm_client {
                    client.set_quality(quality.to_api_rate());
                }
                tracing::info!("Music quality changed to: {:?}", quality);
                Some(Task::perform(async { Message::SaveSettings }, |m| m))
            }
            Message::UpdateEqualizerEnabled(enabled) => {
                self.core.settings.playback.equalizer_enabled = *enabled;
                // Apply to audio processing chain
                self.core.audio_chain.set_equalizer_enabled(*enabled);
                Some(Task::perform(async { Message::SaveSettings }, |m| m))
            }
            Message::UpdateEqualizerPreset(preset) => {
                use crate::features::EqualizerPreset;
                self.core.settings.playback.equalizer_preset = *preset;
                // Apply preset values (unless custom)
                if *preset != EqualizerPreset::Custom {
                    let values = preset.values();
                    self.core.settings.playback.equalizer_values = values;
                    // Apply to audio processing chain
                    self.core.audio_chain.set_equalizer_gains(values);
                }
                Some(Task::perform(async { Message::SaveSettings }, |m| m))
            }
            Message::UpdateEqualizerValues(values) => {
                self.core.settings.playback.equalizer_values = *values;
                // When manually adjusting, switch to custom preset
                self.core.settings.playback.equalizer_preset =
                    crate::features::EqualizerPreset::Custom;
                // Apply to audio processing chain
                self.core.audio_chain.set_equalizer_gains(*values);
                Some(Task::perform(async { Message::SaveSettings }, |m| m))
            }
            Message::UpdateEqualizerPreamp(preamp) => {
                self.core.settings.playback.equalizer_preamp = *preamp;
                // Apply to audio processing chain
                self.core.audio_chain.set_preamp(*preamp);
                Some(Task::perform(async { Message::SaveSettings }, |m| m))
            }
            Message::UpdateSpectrumDecay(decay) => {
                self.core.settings.playback.spectrum_decay = *decay;
                // Apply to audio analysis
                self.core.audio_chain.analysis().set_decay(*decay);
                Some(Task::perform(async { Message::SaveSettings }, |m| m))
            }
            Message::UpdateSpectrumBarsMode(bars_mode) => {
                self.core.settings.playback.spectrum_bars_mode = *bars_mode;
                Some(Task::perform(async { Message::SaveSettings }, |m| m))
            }
            Message::UpdateDarkMode(enabled) => {
                self.core.settings.display.dark_mode = *enabled;
                tracing::info!("Dark mode: {}", enabled);
                Some(Task::perform(async { Message::SaveSettings }, |m| m))
            }
            Message::UpdateAppLanguage(language) => {
                self.core.settings.display.language = language.clone();
                // Update locale for i18n
                let lang = if language == "zh" {
                    crate::i18n::Language::Chinese
                } else {
                    crate::i18n::Language::English
                };
                self.core.locale = crate::i18n::Locale::new(lang);
                tracing::info!("Language changed to: {}", language);
                Some(Task::perform(async { Message::SaveSettings }, |m| m))
            }
            Message::UpdatePowerSavingMode(enabled) => {
                self.core.settings.display.power_saving_mode = *enabled;
                tracing::info!("Power saving mode: {}", enabled);
                Some(Task::perform(async { Message::SaveSettings }, |m| m))
            }
            Message::UpdateMaxCacheMb(size_mb) => {
                self.core.settings.storage.max_cache_mb = *size_mb;
                // Save settings and enforce the new cache limit
                Some(Task::batch([
                    Task::perform(async { Message::SaveSettings }, |m| m),
                    Task::done(Message::EnforceCacheLimit),
                ]))
            }
            Message::ClearCache => Some(Task::perform(
                async {
                    match cache::clear_all_cache() {
                        Ok(result) => {
                            Message::CacheCleared(result.files_deleted, result.bytes_freed)
                        }
                        Err(e) => {
                            tracing::error!("Failed to clear cache: {}", e);
                            Message::CacheCleared(0, 0)
                        }
                    }
                },
                |m| m,
            )),
            Message::CacheCleared(files, bytes) => {
                tracing::info!(
                    "Cache cleared: {} files, {} MB freed",
                    files,
                    bytes / (1024 * 1024)
                );
                // Recalculate cache stats after clearing
                Some(Task::perform(async { Message::RefreshCacheStats }, |m| m))
            }
            Message::RefreshCacheStats => {
                let stats = cache::calculate_cache_stats();
                self.ui.cache_stats = Some(stats);
                Some(Task::none())
            }
            Message::EnforceCacheLimit => {
                let max_mb = self.core.settings.storage.max_cache_mb;
                Some(Task::perform(
                    async move {
                        match cache::enforce_cache_limit(max_mb) {
                            Ok(result) => {
                                if result.files_deleted > 0 {
                                    tracing::info!(
                                        "Cache limit enforced: {} files deleted, {} MB freed",
                                        result.files_deleted,
                                        result.mb_freed()
                                    );
                                }
                                Message::RefreshCacheStats
                            }
                            Err(e) => {
                                tracing::error!("Failed to enforce cache limit: {}", e);
                                Message::RefreshCacheStats
                            }
                        }
                    },
                    |m| m,
                ))
            }
            Message::UpdateAudioOutputDevice(device) => {
                self.core.settings.system.audio_output_device = device.clone();
                // Switch audio output device
                if let Some(player) = &mut self.core.audio {
                    match player.switch_device(device.as_deref()) {
                        Ok(Some((path, position, was_playing))) => {
                            // Resume playback on new device
                            if let Err(e) = player.play(path) {
                                tracing::error!(
                                    "Failed to resume playback after device switch: {}",
                                    e
                                );
                            } else {
                                // Seek to previous position
                                if let Err(e) = player.seek(position) {
                                    tracing::warn!("Failed to seek after device switch: {}", e);
                                }
                                // Pause if wasn't playing
                                if !was_playing {
                                    player.pause();
                                }
                            }
                        }
                        Ok(None) => {
                            tracing::info!("Audio device switched (no track was playing)");
                        }
                        Err(e) => {
                            tracing::error!("Failed to switch audio device: {}", e);
                        }
                    }
                }
                Some(Task::perform(async { Message::SaveSettings }, |m| m))
            }
            Message::UpdateAudioBufferSize(size) => {
                self.core.settings.system.audio_buffer_size = *size;
                tracing::info!("Audio buffer size changed to: {}", size);
                Some(Task::perform(async { Message::SaveSettings }, |m| m))
            }
            Message::UpdateProxyType(proxy_type) => {
                self.core.settings.network.proxy_type = *proxy_type;
                tracing::info!("Proxy type changed to: {:?}", proxy_type);
                Some(Task::batch([
                    Task::perform(async { Message::SaveSettings }, |m| m),
                    Task::done(Message::ApplyProxySettings),
                ]))
            }
            Message::UpdateProxyHost(host) => {
                self.core.settings.network.proxy_host = host.clone();
                Some(Task::batch([
                    Task::perform(async { Message::SaveSettings }, |m| m),
                    Task::done(Message::ApplyProxySettings),
                ]))
            }
            Message::UpdateProxyPort(port_str) => {
                if let Ok(port) = port_str.parse::<u16>() {
                    self.core.settings.network.proxy_port = port;
                    Some(Task::batch([
                        Task::perform(async { Message::SaveSettings }, |m| m),
                        Task::done(Message::ApplyProxySettings),
                    ]))
                } else if port_str.is_empty() {
                    self.core.settings.network.proxy_port = 0;
                    Some(Task::none())
                } else {
                    Some(Task::none())
                }
            }
            Message::UpdateProxyUsername(username) => {
                self.core.settings.network.proxy_username = if username.is_empty() {
                    None
                } else {
                    Some(username.clone())
                };
                Some(Task::batch([
                    Task::perform(async { Message::SaveSettings }, |m| m),
                    Task::done(Message::ApplyProxySettings),
                ]))
            }
            Message::UpdateProxyPassword(password) => {
                self.core.settings.network.proxy_password = if password.is_empty() {
                    None
                } else {
                    Some(password.clone())
                };
                Some(Task::batch([
                    Task::perform(async { Message::SaveSettings }, |m| m),
                    Task::done(Message::ApplyProxySettings),
                ]))
            }
            Message::ApplyProxySettings => {
                if let Some(client) = &mut self.core.ncm_client {
                    if let Some(proxy_url) = self.core.settings.network.proxy_url() {
                        match client.set_proxy(proxy_url.clone()) {
                            Ok(()) => tracing::info!("Proxy applied: {}", proxy_url),
                            Err(e) => tracing::error!("Failed to apply proxy: {}", e),
                        }
                    } else {
                        tracing::info!("Proxy disabled");
                        // When proxy is disabled, recreate client without proxy
                        // and sync quality setting
                        let quality = self.core.settings.playback.music_quality.to_api_rate();
                        if let Some(cookie_jar) = client.client.cookie_jar().cloned() {
                            *client = crate::api::NcmClient::from_cookie_jar(cookie_jar);
                        } else {
                            *client = crate::api::NcmClient::new();
                        }
                        client.set_quality(quality);
                    }
                }
                Some(Task::none())
            }
            Message::ScrollToSection(section) => {
                self.ui.active_settings_section = *section;
                // Get target scroll position for section based on login state
                let is_logged_in = self.core.is_logged_in;
                let target_y = get_section_scroll_position(*section, is_logged_in);
                Some(iced::widget::operation::scroll_to(
                    iced::widget::Id::new("settings_scroll"),
                    iced::widget::scrollable::AbsoluteOffset {
                        x: Some(0.0),
                        y: Some(target_y),
                    },
                ))
            }
            Message::SettingsScrolled(y_offset) => {
                // Update active section based on scroll position and login state
                let is_logged_in = self.core.is_logged_in;
                self.ui.active_settings_section =
                    get_section_from_scroll_position(*y_offset, is_logged_in);
                Some(Task::none())
            }
            Message::StartEditingKeybinding(action) => {
                self.ui.editing_keybinding = Some(*action);
                Some(Task::none())
            }
            Message::CancelEditingKeybinding => {
                self.ui.editing_keybinding = None;
                Some(Task::none())
            }
            Message::KeybindingKeyPressed(key, modifiers) => {
                if let Some(action) = self.ui.editing_keybinding {
                    // Check if Delete/Backspace was pressed to clear the keybinding
                    if matches!(
                        key,
                        iced::keyboard::Key::Named(
                            iced::keyboard::key::Named::Delete
                                | iced::keyboard::key::Named::Backspace
                        )
                    ) {
                        // Clear the keybinding (set to empty)
                        self.core.settings.keybindings.set(action, vec![]);
                        self.ui.editing_keybinding = None;
                        return Some(Task::perform(async { Message::SaveSettings }, |m| m));
                    }

                    // Convert iced key to our KeyCode
                    if let Some(key_code) = key_to_keycode(key) {
                        let binding = KeyBinding {
                            modifiers: ModifierSet {
                                ctrl: modifiers.control(),
                                alt: modifiers.alt(),
                                shift: modifiers.shift(),
                            },
                            key: key_code,
                        };
                        self.core.settings.keybindings.set(action, vec![binding]);
                        self.ui.editing_keybinding = None;
                        return Some(Task::perform(async { Message::SaveSettings }, |m| m));
                    }
                }
                Some(Task::none())
            }
            Message::SaveSettings => {
                if let Err(e) = self.core.settings.save() {
                    tracing::error!("Failed to save settings: {}", e);
                } else {
                    tracing::info!("Settings saved successfully");
                }
                Some(Task::none())
            }
            _ => None,
        }
    }
}
