// src/app/update/discord.rs
//! Discord Rich Presence message handlers

use iced::Task;

use crate::app::message::Message;
use crate::app::state::App;
use crate::audio::AudioEvent;
use crate::platform::discord as discord_mod;

impl App {
    /// Handle Discord Rich Presence messages
    pub fn handle_discord(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::DiscordUpdatePresence => {
                let activity = self.build_discord_activity();
                if let Some(act) = activity {
                    let enabled = self.core.settings.system.discord_enabled;
                    Some(Task::run(
                        futures_util::stream::once(async move {
                            if enabled {
                                discord_mod::send_activity_oneshot(act).await;
                            }
                        }),
                        |_| Message::Noop,
                    ))
                } else {
                    Some(Task::none())
                }
            }

            Message::DiscordClearPresence => Some(Task::run(
                futures_util::stream::once(async move {
                    discord_mod::clear_activity_oneshot().await;
                }),
                |_| Message::Noop,
            )),

            _ => None,
        }
    }

    /// Trigger a Discord presence update from an AudioEvent.
    pub fn maybe_update_discord(&mut self, event: &AudioEvent) -> Option<Task<Message>> {
        if !self.core.settings.system.discord_enabled {
            return None;
        }

        match event {
            AudioEvent::Started { .. } | AudioEvent::Resumed => {
                Some(Task::done(Message::DiscordUpdatePresence))
            }
            AudioEvent::Paused { .. } | AudioEvent::Stopped | AudioEvent::Finished => {
                Some(Task::done(Message::DiscordClearPresence))
            }
            AudioEvent::SeekComplete { .. } => Some(Task::done(Message::DiscordUpdatePresence)),
            _ => None,
        }
    }

    /// Build a Discord activity from the current playback state.
    fn build_discord_activity(&self) -> Option<discord_rich_presence::activity::Activity<'static>> {
        let song = self.playback.current_song.as_ref()?;
        let runtime = &self.playback.runtime;

        let title = if song.title.is_empty() {
            None
        } else {
            Some(song.title.as_str())
        };
        let artist = if song.artist.is_empty() {
            None
        } else {
            Some(song.artist.as_str())
        };
        let album = if song.album.is_empty() {
            None
        } else {
            Some(song.album.as_str())
        };
        let art_url = song.cover_path.as_deref();
        let filename = if song.file_path.is_empty() {
            None
        } else {
            std::path::Path::new(&song.file_path)
                .file_name()
                .and_then(|n| n.to_str())
        };

        let position = runtime.display_position;
        let duration = runtime.info.duration;

        Some(discord_mod::build_activity_safe(
            title, artist, album, art_url, filename, position, duration,
        ))
    }
}
