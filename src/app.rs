//! Main application module

pub mod helpers;
mod message;
mod state;
mod update;
mod view;

use iced::{Task, Theme};
use std::sync::Arc;

use crate::i18n::{Language, Locale};
pub use message::{IconId, Message, SettingsSection, SidebarId};
pub use state::{
    App, CoreState, DiscoverPageState, DiscoverViewMode, HomePageState, LibraryState, UiState,
    UserInfo,
};

impl App {
    /// Create new application instance
    pub fn new() -> (Self, Task<Message>) {
        // 1. Load settings first to initialize locale correctly
        let settings = crate::features::Settings::load();
        let locale = {
            let lang = if settings.display.language == "zh" {
                Language::Chinese
            } else {
                Language::English
            };
            Locale::new(lang)
        };

        // 2. Initialize sub-states
        let core = CoreState::new(settings, locale);
        let library = LibraryState::default();
        let ui = UiState::new();

        let app = Self { core, library, ui };

        // 3. Open main window
        let (window_id, open_window) = iced::window::open(iced::window::Settings {
            size: iced::Size::new(1400.0, 900.0),
            exit_on_close_request: false,
            decorations: false,
            #[cfg(target_os = "linux")]
            platform_specific: iced::window::settings::PlatformSpecific {
                application_id: "rustle".to_string(),
                ..Default::default()
            },
            #[cfg(target_os = "macos")]
            platform_specific: iced::window::settings::PlatformSpecific {
                title_hidden: true,
                titlebar_transparent: true,
                fullsize_content_view: true,
            },
            #[cfg(target_os = "windows")]
            platform_specific: Default::default(),
            ..Default::default()
        });
        tracing::info!("Opening main window with id: {:?}", window_id);

        // 4. Initialize async tasks
        let init_task = Task::batch([
            open_window.discard(),
            Task::perform(helpers::init_database(), |result| match result {
                Ok(db) => Message::DatabaseReady(Arc::new(db)),
                Err(e) => Message::DatabaseError(e.to_string()),
            }),
            Task::perform(helpers::init_cover_cache(), |result| match result {
                Ok(cache) => Message::CoverCacheReady(Arc::new(cache)),
                Err(e) => Message::DatabaseError(format!("Cover cache error: {}", e)),
            }),
            #[cfg(target_os = "linux")]
            Task::perform(helpers::init_tray(), |result| match result {
                Ok(rx) => Message::TrayStarted(rx),
                Err(e) => {
                    tracing::warn!("Failed to start system tray: {}", e);
                    Message::Noop
                }
            }),
            #[cfg(target_os = "linux")]
            Task::perform(helpers::init_mpris(), |result| match result {
                Ok((handle, rx)) => Message::MprisStartedWithHandle(handle, rx),
                Err(e) => {
                    tracing::warn!("Failed to start MPRIS: {}", e);
                    Message::Noop
                }
            }),
            Task::perform(helpers::init_font_system(), |font_system| {
                Message::LyricsFontSystemReady(font_system)
            }),
            Task::done(Message::TryAutoLogin(0)),
            Task::done(Message::EnforceCacheLimit),
        ]);

        (app, init_task)
    }

    /// Application theme for a specific window
    pub fn theme(&self, _window_id: iced::window::Id) -> Theme {
        // Access settings via core state
        if self.core.settings.display.dark_mode {
            Theme::Dark
        } else {
            Theme::Light
        }
    }

    /// Dynamic window title based on current playback state
    pub fn title(&self, _window_id: iced::window::Id) -> String {
        // Access current song via library state
        if let Some(song) = &self.library.current_song {
            format!("Rustle - {}", song.title)
        } else {
            "Rustle - Music".to_string()
        }
    }

    /// Subscriptions for animations, playback, keyboard events, and window close
    pub fn subscription(&self) -> iced::Subscription<Message> {
        use iced::keyboard;
        use iced::time::{Duration, Instant};

        let now = Instant::now();

        // Check if power saving mode is enabled
        let power_saving = self.core.settings.display.power_saving_mode;

        // 1. UI animations (disabled in power saving mode)
        let has_animations = if power_saving {
            false
        } else {
            self.ui.has_active_animations(now)
        };

        // 2. Playback state
        let is_playing = self
            .core
            .audio
            .as_ref()
            .map(|p| p.is_playing())
            .unwrap_or(false);

        // 3. Lyrics page needs continuous updates for smooth scrolling
        let lyrics_needs_frames = if power_saving {
            false
        } else {
            self.ui.lyrics.is_open
        };

        // 4. Audio engine visualization
        let audio_engine_needs_frames = if power_saving {
            false
        } else {
            self.ui.active_nav == crate::ui::components::NavItem::AudioEngine && is_playing
        };

        // 5. Keyboard events
        let keyboard_sub = if !self.core.window_hidden {
            keyboard::listen().filter_map(|event| match event {
                keyboard::Event::KeyPressed { key, modifiers, .. } => {
                    Some(Message::KeyPressed(key, modifiers))
                }
                _ => None,
            })
        } else {
            iced::Subscription::none()
        };

        // 6. Window events
        let close_request_sub = iced::window::close_requests().map(|_id| Message::RequestClose);
        let close_event_sub =
            iced::window::close_events().map(|_id| Message::WindowOperationComplete);

        // 7. Animation subscription (~60fps when needed)
        let animation_sub = if has_animations || lyrics_needs_frames || audio_engine_needs_frames {
            iced::window::frames().map(|_| Message::AnimationTick)
        } else {
            iced::Subscription::none()
        };

        // 8. Playback monitoring (100ms/500ms intervals)
        let playback_sub = if is_playing {
            let interval = if power_saving { 500 } else { 100 };
            iced::time::every(Duration::from_millis(interval)).map(|_| Message::PlaybackTick)
        } else {
            iced::Subscription::none()
        };

        // 9. Carousel auto-advance (5s)
        let carousel_sub =
            if !power_saving && !self.ui.home.banners.is_empty() && !self.core.window_hidden {
                iced::time::every(Duration::from_secs(5)).map(|_| Message::CarouselTick)
            } else {
                iced::Subscription::none()
            };

        // 10. Window resize
        let resize_sub =
            iced::window::resize_events().map(|(_id, size)| Message::WindowResized(size));

        // 11. Mouse events for window dragging (top 48px area)
        let mouse_sub = if !self.core.window_hidden {
            iced::event::listen().filter_map(|event| match event {
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                    iced::mouse::Button::Left,
                )) => Some(Message::MousePressed),
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Some(Message::MouseMoved(position))
                }
                _ => None,
            })
        } else {
            iced::Subscription::none()
        };

        // Batch all subscriptions
        iced::Subscription::batch([
            keyboard_sub,
            close_request_sub,
            close_event_sub,
            animation_sub, // Animation updates (vsync rate)
            playback_sub,  // Playback monitoring (100ms intervals)
            carousel_sub,
            resize_sub,
            mouse_sub,
        ])
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new().0
    }
}

/// Subscription decision logic for testability
pub mod subscription_logic {
    pub fn needs_animation_subscription(
        has_animations: bool,
        lyrics_needs_frames: bool,
        audio_engine_needs_frames: bool,
    ) -> bool {
        has_animations || lyrics_needs_frames || audio_engine_needs_frames
    }

    pub fn needs_playback_subscription(is_playing: bool) -> bool {
        is_playing
    }

    pub fn subscription_decisions(
        has_animations: bool,
        lyrics_needs_frames: bool,
        audio_engine_needs_frames: bool,
        is_playing: bool,
    ) -> (bool, bool) {
        let needs_animation = needs_animation_subscription(
            has_animations,
            lyrics_needs_frames,
            audio_engine_needs_frames,
        );
        let needs_playback = needs_playback_subscription(is_playing);
        (needs_animation, needs_playback)
    }
}

#[cfg(test)]
mod tests {
    use super::subscription_logic::*;

    mod property_playback_independence {
        use super::*;

        #[test]
        fn playback_active_no_animations() {
            // When playing with no animations, playback subscription should be active
            let (needs_animation, needs_playback) = subscription_decisions(
                false, // has_animations
                false, // lyrics_needs_frames
                false, // audio_engine_needs_frames
                true,  // is_playing
            );
            assert!(
                !needs_animation,
                "No animation subscription when no animations"
            );
            assert!(
                needs_playback,
                "Playback subscription must be active when playing"
            );
        }

        #[test]
        fn playback_active_with_lyrics_page() {
            // When playing with lyrics page open, BOTH subscriptions should be active
            let (needs_animation, needs_playback) = subscription_decisions(
                false, // has_animations
                true,  // lyrics_needs_frames (lyrics page open)
                false, // audio_engine_needs_frames
                true,  // is_playing
            );
            assert!(needs_animation, "Animation subscription for lyrics page");
            assert!(
                needs_playback,
                "Playback subscription must be active when playing"
            );
        }

        #[test]
        fn playback_active_with_ui_animations() {
            // When playing with UI animations, BOTH subscriptions should be active
            let (needs_animation, needs_playback) = subscription_decisions(
                true,  // has_animations
                false, // lyrics_needs_frames
                false, // audio_engine_needs_frames
                true,  // is_playing
            );
            assert!(needs_animation, "Animation subscription for UI animations");
            assert!(
                needs_playback,
                "Playback subscription must be active when playing"
            );
        }

        #[test]
        fn playback_active_with_audio_engine_page() {
            // When playing with audio engine visualization, BOTH subscriptions should be active
            let (needs_animation, needs_playback) = subscription_decisions(
                false, // has_animations
                false, // lyrics_needs_frames
                true,  // audio_engine_needs_frames
                true,  // is_playing
            );
            assert!(
                needs_animation,
                "Animation subscription for audio visualization"
            );
            assert!(
                needs_playback,
                "Playback subscription must be active when playing"
            );
        }

        #[test]
        fn playback_active_with_all_animations() {
            // When playing with all animation sources active, BOTH subscriptions should be active
            let (needs_animation, needs_playback) = subscription_decisions(
                true, // has_animations
                true, // lyrics_needs_frames
                true, // audio_engine_needs_frames
                true, // is_playing
            );
            assert!(needs_animation, "Animation subscription for all sources");
            assert!(
                needs_playback,
                "Playback subscription must be active when playing"
            );
        }

        #[test]
        fn playback_inactive_no_subscriptions() {
            // When not playing and no animations, no subscriptions needed
            let (needs_animation, needs_playback) = subscription_decisions(
                false, // has_animations
                false, // lyrics_needs_frames
                false, // audio_engine_needs_frames
                false, // is_playing
            );
            assert!(!needs_animation, "No animation subscription");
            assert!(!needs_playback, "No playback subscription when not playing");
        }

        #[test]
        fn playback_inactive_with_animations() {
            // When not playing but animations active, only animation subscription
            let (needs_animation, needs_playback) = subscription_decisions(
                true,  // has_animations
                false, // lyrics_needs_frames
                false, // audio_engine_needs_frames
                false, // is_playing
            );
            assert!(needs_animation, "Animation subscription for UI animations");
            assert!(!needs_playback, "No playback subscription when not playing");
        }
    }

    mod property_coexistence {
        use super::*;

        #[test]
        fn both_subscriptions_when_lyrics_and_playing() {
            // Critical case: lyrics page open while playing
            let (needs_animation, needs_playback) = subscription_decisions(
                false, // has_animations
                true,  // lyrics_needs_frames
                false, // audio_engine_needs_frames
                true,  // is_playing
            );
            assert!(
                needs_animation && needs_playback,
                "Both subscriptions must coexist when lyrics page is open and playing"
            );
        }

        #[test]
        fn both_subscriptions_when_audio_engine_and_playing() {
            // Audio engine visualization while playing
            let (needs_animation, needs_playback) = subscription_decisions(
                false, // has_animations
                false, // lyrics_needs_frames
                true,  // audio_engine_needs_frames
                true,  // is_playing
            );
            assert!(
                needs_animation && needs_playback,
                "Both subscriptions must coexist when audio engine page is open and playing"
            );
        }

        #[test]
        fn both_subscriptions_when_ui_animations_and_playing() {
            // UI animations (hover, fade) while playing
            let (needs_animation, needs_playback) = subscription_decisions(
                true,  // has_animations
                false, // lyrics_needs_frames
                false, // audio_engine_needs_frames
                true,  // is_playing
            );
            assert!(
                needs_animation && needs_playback,
                "Both subscriptions must coexist when UI animations are active and playing"
            );
        }

        #[test]
        fn subscriptions_are_independent() {
            // Verify that animation state changes don't affect playback subscription
            // and vice versa

            // Case 1: Animation state changes, playback stays active
            let (_, playback1) = subscription_decisions(false, false, false, true);
            let (_, playback2) = subscription_decisions(true, false, false, true);
            let (_, playback3) = subscription_decisions(false, true, false, true);
            let (_, playback4) = subscription_decisions(false, false, true, true);

            assert!(
                playback1 && playback2 && playback3 && playback4,
                "Playback subscription must be independent of animation state"
            );

            // Case 2: Playback state changes, animation stays active
            let (anim1, _) = subscription_decisions(true, false, false, false);
            let (anim2, _) = subscription_decisions(true, false, false, true);

            assert!(
                anim1 && anim2,
                "Animation subscription must be independent of playback state"
            );
        }
    }

    mod property_playback_isolation {
        use super::*;

        #[test]
        fn playback_only_depends_on_is_playing() {
            // Test all combinations of animation states with is_playing = true
            let animation_combinations = [
                (false, false, false),
                (true, false, false),
                (false, true, false),
                (false, false, true),
                (true, true, false),
                (true, false, true),
                (false, true, true),
                (true, true, true),
            ];

            for (has_anim, lyrics, audio_engine) in animation_combinations {
                let needs_playback = needs_playback_subscription(true);
                assert!(
                    needs_playback,
                    "Playback subscription must be active when is_playing=true, \
                     regardless of animation state (has_anim={}, lyrics={}, audio_engine={})",
                    has_anim, lyrics, audio_engine
                );
            }

            // Test all combinations with is_playing = false
            for (has_anim, lyrics, audio_engine) in animation_combinations {
                let needs_playback = needs_playback_subscription(false);
                assert!(
                    !needs_playback,
                    "Playback subscription must be inactive when is_playing=false, \
                     regardless of animation state (has_anim={}, lyrics={}, audio_engine={})",
                    has_anim, lyrics, audio_engine
                );
            }
        }
    }
}
