// src/app/update/database.rs
//! Database message handlers

use iced::Task;

use crate::app::helpers::{
    load_playback_state, load_playlists, load_queue, load_songs, validate_songs,
};
use crate::app::message::Message;
use crate::app::state::App;
use crate::ui::pages;

impl App {
    /// Handle database-related messages
    pub fn handle_database(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::DatabaseReady(db) => {
                tracing::info!("Database initialized successfully");
                self.core.db = Some(db.clone());

                // First validate songs in background, then load data
                Some(Task::batch([
                    // Background validation - runs first to clean up invalid entries
                    Task::perform(validate_songs(db.clone()), Message::SongsValidated),
                    // Load data (will run in parallel, but validation is fast)
                    Task::perform(load_songs(db.clone()), Message::SongsLoaded),
                    Task::perform(load_playlists(db.clone()), Message::PlaylistsLoaded),
                    Task::perform(load_playback_state(db.clone()), |state| match state {
                        Some(state) => Message::PlaybackStateLoaded(state),
                        None => Message::DatabaseError("No playback state".into()),
                    }),
                    Task::perform(load_queue(db.clone()), Message::QueueRestored),
                ]))
            }

            Message::DatabaseError(err) => {
                tracing::error!("Database error: {}", err);
                self.core.db_error = Some(err.clone());
                Some(Task::none())
            }

            Message::SongsValidated(removed_count) => {
                if *removed_count > 0 {
                    tracing::info!("Validated songs: {} invalid entries removed", removed_count);
                    // Reload songs after validation to get clean list
                    if let Some(db) = &self.core.db {
                        return Some(Task::perform(load_songs(db.clone()), Message::SongsLoaded));
                    }
                }
                Some(Task::none())
            }

            Message::SongsLoaded(songs) => {
                tracing::info!("Loaded {} songs from database", songs.len());
                self.library.db_songs = songs.clone();
                Some(Task::none())
            }

            Message::PlaylistsLoaded(playlists) => {
                tracing::info!("Loaded {} playlists from database", playlists.len());
                self.library.playlists = playlists.clone();
                Some(Task::none())
            }

            Message::PlaybackStateLoaded(state) => {
                tracing::info!(
                    "Loaded playback state: position={}, volume={}",
                    state.position_secs,
                    state.volume
                );
                if let Some(player) = &mut self.core.audio {
                    player.set_volume(state.volume as f32);
                }
                self.library.playback_state = Some(state.clone());
                Some(Task::none())
            }

            Message::QueueRestored(queue) => {
                tracing::info!("Restored {} songs in queue", queue.len());
                self.library.queue = queue.clone();

                // Initialize shuffle cache for preloading (must be done before preload)
                self.cache_shuffle_indices();

                if let Some(state) = &self.library.playback_state {
                    if state.queue_position >= 0
                        && (state.queue_position as usize) < self.library.queue.len()
                    {
                        let idx = state.queue_position as usize;
                        self.library.queue_index = Some(idx);
                        if let Some(song) = self.library.queue.get(idx) {
                            self.library.current_song = Some(song.clone());

                            // Check if this is an NCM song (negative ID or ncm:// path)
                            let is_ncm = song.id < 0 || song.file_path.starts_with("ncm://");

                            if is_ncm {
                                // NCM song - resolve and load just like local songs
                                tracing::info!(
                                    "Restoring NCM song: {} - {}",
                                    song.title,
                                    song.artist
                                );

                                // Trigger resolution task
                                if let Some(client) = self.core.ncm_client.clone() {
                                    let song_clone = song.clone();
                                    let saved_position = state.position_secs;
                                    let client = std::sync::Arc::new(client);

                                    return Some(Task::perform(
                                        async move {
                                            // Create a dummy channel for restore
                                            let (event_tx, _event_rx) = tokio::sync::mpsc::channel(1);
                                            crate::app::update::song_resolver::resolve_song(
                                                client,
                                                &song_clone,
                                                event_tx,
                                            )
                                            .await
                                        },
                                        move |result| {
                                            Message::SongResolvedForRestore(
                                                idx,
                                                result,
                                                saved_position,
                                            )
                                        },
                                    ));
                                } else {
                                    tracing::warn!("NCM client not available for song restoration");
                                }
                            } else {
                                // Local song - load into audio player and trigger preload
                                if let Some(player) = &mut self.core.audio {
                                    let path_buf = std::path::PathBuf::from(&song.file_path);
                                    if path_buf.exists() {
                                        if let Err(e) = player.play(path_buf) {
                                            tracing::warn!("Failed to load song on startup: {}", e);
                                        } else {
                                            // Pause immediately and seek to saved position
                                            player.pause();
                                            let position = std::time::Duration::from_secs_f64(
                                                state.position_secs,
                                            );
                                            let _ = player.seek(position);
                                            // Update cached position for UI display
                                            player.update_paused_position(position);
                                            tracing::info!(
                                                "Loaded song and seeked to {:?}",
                                                position
                                            );
                                        }
                                    }
                                }

                                // Trigger preload for adjacent tracks after queue is restored
                                let preload_task = self.preload_adjacent_tracks_with_ncm();
                                return Some(preload_task);
                            }
                        }
                    }
                }
                Some(Task::none())
            }

            Message::SongResolvedForRestore(idx, result, saved_position) => {
                // Handle NCM song resolution result during app startup
                if let Some(resolved) = result {
                    tracing::info!("NCM song resolved for restore: {:?}", resolved.file_path);

                    // Update song in queue with resolved file path and cover
                    if let Some(song) = self.library.queue.get_mut(*idx) {
                        song.file_path = resolved.file_path.clone();
                        if let Some(cover) = &resolved.cover_path {
                            song.cover_path = Some(cover.clone());
                        }
                    }

                    // Update current_song if this is the current song
                    if self.library.queue_index == Some(*idx) {
                        if let Some(song) = self.library.queue.get(*idx) {
                            self.library.current_song = Some(song.clone());
                        }

                        // Load into audio player
                        if let Some(player) = &mut self.core.audio {
                            let path_buf = std::path::PathBuf::from(&resolved.file_path);
                            if let Err(e) = player.play(path_buf) {
                                tracing::warn!("Failed to load NCM song on startup: {}", e);
                            } else {
                                // Pause immediately and seek to saved position
                                player.pause();
                                let position = std::time::Duration::from_secs_f64(*saved_position);
                                let _ = player.seek(position);
                                player.update_paused_position(position);
                                tracing::info!("Loaded NCM song and seeked to {:?}", position);
                            }
                        }

                        // Trigger preload for adjacent tracks after NCM song is restored
                        let preload_task = self.preload_adjacent_tracks_with_ncm();
                        return Some(preload_task);
                    }
                } else {
                    tracing::warn!("Failed to resolve NCM song for restore at index {}", idx);
                }
                Some(Task::none())
            }

            Message::RecentlyPlayedLoaded(songs) => {
                tracing::info!("Loaded {} recently played songs", songs.len());
                self.library.recently_played = songs.clone();

                // Create a playlist view for recently played
                let song_views: Vec<pages::PlaylistSongView> = songs
                    .iter()
                    .enumerate()
                    .map(|(i, song)| {
                        let duration_secs = song.duration_secs as u64;
                        let mins = duration_secs / 60;
                        let secs = duration_secs % 60;

                        pages::PlaylistSongView::new(
                            song.id,
                            i + 1,
                            song.title.clone(),
                            song.artist.clone(),
                            song.album.clone(),
                            format!("{}:{:02}", mins, secs),
                            self.core
                                .locale
                                .get(crate::i18n::Key::RecentlyPlayedList)
                                .to_string(),
                            song.cover_path.clone(),
                            self.library
                                .current_song
                                .as_ref()
                                .map(|s| s.id)
                                .unwrap_or(0)
                                == song.id,
                        )
                    })
                    .collect();

                // Calculate total duration
                let total_secs: u64 = songs.iter().map(|s| s.duration_secs as u64).sum();
                let total_mins = total_secs / 60;
                let total_hours = total_mins / 60;
                let remaining_mins = total_mins % 60;
                let total_duration = if total_hours > 0 {
                    format!("约 {} 小时 {} 分钟", total_hours, remaining_mins)
                } else {
                    format!("{} 分钟", total_mins)
                };

                // Create playlist view with special ID for recently played
                let playlist_view = pages::PlaylistView {
                    id: -1, // Special ID for recently played
                    name: self
                        .core
                        .locale
                        .get(crate::i18n::Key::RecentlyPlayed)
                        .to_string(),
                    description: Some(
                        self.core
                            .locale
                            .get(crate::i18n::Key::RecentlyPlayedDescription)
                            .to_string(),
                    ),
                    cover_path: None,
                    owner: "本地".to_string(),
                    owner_avatar_path: None,
                    creator_id: 0,
                    song_count: songs.len() as u32,
                    total_duration,
                    like_count: String::new(),
                    songs: song_views,
                    palette: crate::utils::ColorPalette::default(), // Use default colors
                    is_local: true,
                    is_subscribed: false,
                };

                self.ui.playlist_page.current = Some(playlist_view);

                Some(Task::none())
            }

            _ => None,
        }
    }
}
