// src/app/update/playback.rs
//! Playback control message handlers

use iced::Task;

use crate::app::helpers::update_tray_state_full;
use crate::app::message::Message;
use crate::app::state::App;

impl App {
    /// Handle playback-related messages
    pub fn handle_playback(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::PlaySong(id) => {
                tracing::info!("Playing song id: {}", id);
                // Find song in queue or add it
                if let Some(idx) = self.library.queue.iter().position(|s| s.id == *id) {
                    return Some(self.play_song_at_index(idx));
                }

                // Try to find in DB songs
                if let Some(song) = self.library.db_songs.iter().find(|s| s.id == *id).cloned() {
                    self.library.queue.push(song);
                    let idx = self.library.queue.len() - 1;
                    return Some(self.play_song_at_index(idx));
                }

                // Try NCM playlist songs
                if *id < 0 {
                    let ncm_id = (-*id) as u64;
                    if let Some(song_info) = self
                        .ui
                        .home
                        .current_ncm_playlist_songs
                        .iter()
                        .find(|s| s.id == ncm_id)
                    {
                        let db_song = crate::database::DbSong {
                            id: -(song_info.id as i64),
                            file_path: String::new(),
                            title: song_info.name.clone(),
                            artist: song_info.singer.clone(),
                            album: song_info.album.clone(),
                            duration_secs: (song_info.duration / 1000) as i64,
                            track_number: None,
                            year: None,
                            genre: None,
                            cover_path: if song_info.pic_url.is_empty() {
                                None
                            } else {
                                Some(song_info.pic_url.clone())
                            },
                            file_hash: None,
                            file_size: 0,
                            format: Some("mp3".to_string()),
                            play_count: 0,
                            last_played: None,
                            last_modified: 0,
                            created_at: 0,
                        };
                        self.library.queue.push(db_song);
                        let idx = self.library.queue.len() - 1;
                        return Some(self.play_song_at_index(idx));
                    }
                }

                Some(Task::none())
            }

            Message::TogglePlayback => {
                tracing::info!("TogglePlayback message received");
                Some(self.toggle_playback())
            }

            Message::NextSong => Some(self.play_next_song()),

            Message::PrevSong => Some(self.play_prev_song()),

            Message::SeekPreview(position) => {
                self.ui.is_seeking = true;
                self.ui.seek_preview_position = *position;
                if let Some(player) = &self.core.audio {
                    player.stop_fade();
                }
                Some(Task::none())
            }

            Message::SeekRelease => Some(self.apply_seek()),

            Message::SetVolume(volume) => {
                if let Some(player) = &mut self.core.audio {
                    player.set_volume(*volume);
                    if let Some(db) = &self.core.db {
                        let db = db.clone();
                        let vol = *volume as f64;
                        tokio::spawn(async move {
                            let _ = db.update_volume(vol).await;
                        });
                    }
                }
                Some(Task::none())
            }

            Message::PlaybackTick => Some(self.handle_playback_tick()),

            Message::CyclePlayMode => {
                self.core.settings.play_mode = self.core.settings.play_mode.next();
                let _ = self.core.settings.save();
                tracing::info!(
                    "Play mode changed to: {}",
                    self.core.settings.play_mode.display_name()
                );

                let (title, artist) = self
                    .library
                    .current_song
                    .as_ref()
                    .map(|s| (Some(s.title.clone()), Some(s.artist.clone())))
                    .unwrap_or((None, None));
                let is_playing = self
                    .core
                    .audio
                    .as_ref()
                    .map(|p| p.is_playing())
                    .unwrap_or(false);
                update_tray_state_full(is_playing, title, artist, self.core.settings.play_mode);

                // Clear shuffle cache and re-calculate for new mode
                self.clear_shuffle_cache();
                self.cache_shuffle_indices();
                let _ = self.preload_adjacent_tracks_with_ncm();
                Some(Task::none())
            }

            _ => None,
        }
    }

    /// Toggle playback state
    fn toggle_playback(&mut self) -> Task<Message> {
        use crate::audio::PlaybackStatus;

        // Get current status first (immutable borrow)
        let status = match &self.core.audio {
            Some(player) => player.get_info().status,
            None => {
                tracing::warn!("toggle_playback: No audio player");
                return Task::none();
            }
        };

        tracing::info!("toggle_playback: current status = {:?}", status);

        match status {
            PlaybackStatus::Stopped => {
                // No audio loaded, try to play current song
                // Check if we have a current song and queue index
                if let Some(idx) = self.library.queue_index {
                    // Use play_song_at_index which handles NCM song resolution
                    return self.play_song_at_index(idx);
                }

                // Fallback: try to play from current_song directly (for local songs)
                let song = self.library.current_song.as_ref();
                if let Some(song) = song {
                    // Check if it's an NCM song that needs resolution
                    let is_ncm = song.id < 0
                        || song.file_path.is_empty()
                        || song.file_path.starts_with("ncm://");
                    if is_ncm {
                        // NCM song without queue index - can't play
                        tracing::warn!("Cannot play NCM song without queue index");
                        return Task::none();
                    }

                    let file_path = song.file_path.clone();
                    let title = song.title.clone();
                    let artist = song.artist.clone();
                    let playback_pos = self
                        .library
                        .playback_state
                        .as_ref()
                        .filter(|s| s.position_secs > 0.0)
                        .map(|s| s.position_secs);
                    let fade_in = self.core.settings.playback.fade_in_out;
                    let normalize = self.core.settings.playback.volume_normalization;

                    let path = std::path::PathBuf::from(&file_path);
                    if let Some(player) = &mut self.core.audio {
                        if let Err(e) = player.play_with_fade(path, fade_in) {
                            tracing::error!("Failed to play: {}", e);
                        } else {
                            if normalize {
                                player.set_track_gain(1.0);
                            }
                            if let Some(pos) = playback_pos {
                                let seek_pos = std::time::Duration::from_secs_f64(pos);
                                let _ = player.seek(seek_pos);
                            }
                        }
                    }
                    self.update_tray_and_mpris(true, Some(title), Some(artist));
                }
            }
            PlaybackStatus::Playing => {
                // Currently playing, pause it
                // Save position first (needs immutable borrow)
                let position_info = self
                    .core
                    .audio
                    .as_ref()
                    .map(|p| p.get_info().position.as_secs_f64());
                if let (Some(pos), Some(db), Some(song)) =
                    (position_info, &self.core.db, &self.library.current_song)
                {
                    let db = db.clone();
                    let song_id = song.id;
                    let queue_pos = self.library.queue_index.unwrap_or(0) as i64;
                    tokio::spawn(async move {
                        let _ = db
                            .update_playback_position(Some(song_id), queue_pos, pos)
                            .await;
                    });
                }

                // Now pause (needs mutable borrow)
                let fade = self.core.settings.playback.fade_in_out;
                if let Some(player) = &mut self.core.audio {
                    if fade {
                        player.pause_with_fade(true);
                    } else {
                        player.pause();
                    }
                }
                self.update_tray_and_mpris_current(false);
            }
            PlaybackStatus::Pausing => {
                // Currently fading out, force immediate pause
                if let Some(player) = &mut self.core.audio {
                    player.stop_fade();
                    player.pause();
                }
                self.update_tray_and_mpris_current(false);
            }
            PlaybackStatus::Paused => {
                // Currently paused, resume it
                let fade = self.core.settings.playback.fade_in_out;
                if let Some(player) = &mut self.core.audio {
                    if fade {
                        player.resume_with_fade(true);
                    } else {
                        player.resume();
                    }
                }
                self.update_tray_and_mpris_current(true);
            }
        }

        Task::none()
    }

    /// Update tray icon and MPRIS state with specific song info
    fn update_tray_and_mpris(
        &mut self,
        is_playing: bool,
        title: Option<String>,
        artist: Option<String>,
    ) {
        update_tray_state_full(is_playing, title, artist, self.core.settings.play_mode);
        self.update_mpris_state();
    }

    /// Update tray icon and MPRIS state using current song
    fn update_tray_and_mpris_current(&mut self, is_playing: bool) {
        let (title, artist) = self
            .library
            .current_song
            .as_ref()
            .map(|s| (Some(s.title.clone()), Some(s.artist.clone())))
            .unwrap_or((None, None));
        update_tray_state_full(is_playing, title, artist, self.core.settings.play_mode);
        self.update_mpris_state();
    }

    /// Apply seek position when user releases slider
    fn apply_seek(&mut self) -> Task<Message> {
        if self.ui.is_seeking {
            if let Some(player) = &mut self.core.audio {
                let info = player.get_info();
                if info.duration.as_secs_f32() > 0.0 {
                    // Audio is loaded, seek directly
                    let seek_pos = std::time::Duration::from_secs_f32(
                        self.ui.seek_preview_position * info.duration.as_secs_f32(),
                    );
                    match player.seek(seek_pos) {
                        Ok(_) => {
                            if !player.is_playing() {
                                player.update_paused_position(seek_pos);
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Seek failed: {}", e);
                            // For formats that don't support seeking (like AAC),
                            // show a toast and continue playing from current position
                            if e.contains("not supported") {
                                return Task::done(Message::ShowToast(
                                    "该格式不支持拖动进度条".to_string(),
                                ));
                            }
                        }
                    }
                    self.update_mpris_state();
                } else if let Some(song) = &self.library.current_song {
                    // No audio loaded - try to load the file and seek
                    let path = std::path::PathBuf::from(&song.file_path);
                    if !song.file_path.is_empty() && path.exists() {
                        let duration = song.duration_secs as f32;
                        if let Err(e) = player.play(path) {
                            tracing::error!("Failed to play: {}", e);
                        } else {
                            let seek_pos = std::time::Duration::from_secs_f32(
                                self.ui.seek_preview_position * duration,
                            );
                            if let Err(e) = player.seek(seek_pos) {
                                if e.contains("not supported") {
                                    self.ui.is_seeking = false;
                                    return Task::done(Message::ShowToast(
                                        "该格式不支持拖动进度条".to_string(),
                                    ));
                                }
                            }
                            self.update_mpris_state();
                        }
                    } else {
                        tracing::warn!("Cannot seek: no valid file path for song");
                    }
                }
            }
            self.ui.is_seeking = false;
        }
        Task::none()
    }

    /// Update audio fade state (for smooth pause/resume transitions)
    /// This should be called regularly from any high-frequency tick handler
    pub fn update_audio_fade(&self) {
        if let Some(player) = &self.core.audio {
            player.update_fade();
        }
    }

    /// Handle playback tick for auto-save and auto-next
    fn handle_playback_tick(&mut self) -> Task<Message> {
        self.update_audio_fade();

        self.update_mpris_state();

        let lyrics_scroll_task = if self.ui.lyrics.is_open {
            self.update_lyrics_animations()
        } else {
            Task::none()
        };

        self.check_lyrics_page_close();

        // Auto-save position every 5 seconds
        self.ui.save_position_counter += 1;
        if self.ui.save_position_counter >= 50 {
            self.ui.save_position_counter = 0;
            if let (Some(player), Some(db), Some(song)) =
                (&self.core.audio, &self.core.db, &self.library.current_song)
            {
                if player.is_playing() {
                    let info = player.get_info();
                    let position_secs = info.position.as_secs_f64();
                    let db = db.clone();
                    let song_id = song.id;
                    let queue_pos = self.library.queue_index.unwrap_or(0) as i64;
                    tokio::spawn(async move {
                        let _ = db
                            .update_playback_position(Some(song_id), queue_pos, position_secs)
                            .await;
                    });
                }
            }
        }

        // Check if song finished
        // Don't trigger next song if we're waiting for a song to be resolved
        if let Some(player) = &self.core.audio {
            let is_resolving = self.library.pending_resolution_idx.is_some();
            let info = player.get_info();

            // Check for song completion:
            // 1. Sink is empty (normal completion)
            // 2. Status changed to Stopped (detected in get_info)
            // 3. Position reached duration (for formats where empty() doesn't work)
            let is_finished =
                player.is_finished() || info.status == crate::audio::PlaybackStatus::Stopped;

            if is_finished && self.library.current_song.is_some() && !is_resolving {
                tracing::info!("Song finished, triggering next song");
                return self.handle_song_finished();
            }
        }

        lyrics_scroll_task
    }
}
