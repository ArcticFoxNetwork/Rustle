// src/app/update/playback.rs
//! Playback control message handlers

use iced::Task;

use crate::app::helpers::update_tray_state_full;
use crate::app::message::Message;
use crate::app::state::App;
use crate::audio::AudioEvent;

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
                self.ui.seek_preview_position = Some(*position);
                Some(Task::none())
            }

            Message::SeekRelease => Some(self.apply_seek()),

            Message::SetVolume(volume) => {
                if let Some(player) = &self.core.audio {
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

            // Streaming playback messages
            Message::StreamingEvent(song_id, event) => {
                Some(self.handle_streaming_event(*song_id, event.clone()))
            }

            Message::AudioEvent(event) => Some(self.handle_audio_event(event.clone())),

            _ => None,
        }
    }

    /// Toggle playback state
    fn toggle_playback(&mut self) -> Task<Message> {
        use crate::audio::PlaybackStatus;

        // Get current status from shared state
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
                if let Some(idx) = self.library.queue_index {
                    return self.play_song_at_index(idx);
                }

                // Fallback: try to play from current_song directly (for local songs)
                if let Some(song) = self.library.current_song.as_ref() {
                    let is_ncm = song.id < 0
                        || song.file_path.is_empty()
                        || song.file_path.starts_with("ncm://");
                    if is_ncm {
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
                    if let Some(player) = &self.core.audio {
                        player.play_with_fade(path, fade_in);
                        if normalize {
                            player.set_track_gain(1.0);
                        }
                        if let Some(pos) = playback_pos {
                            let seek_pos = std::time::Duration::from_secs_f64(pos);
                            player.seek(seek_pos);
                        }
                    }
                    self.update_tray_and_mpris(true, Some(title), Some(artist));
                }
            }
            PlaybackStatus::Playing => {
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

                let fade = self.core.settings.playback.fade_in_out;
                if let Some(player) = &self.core.audio {
                    player.pause_with_fade(fade);
                }
                self.update_tray_and_mpris_current(false);
            }
            PlaybackStatus::Paused => {
                let fade = self.core.settings.playback.fade_in_out;
                if let Some(player) = &self.core.audio {
                    player.resume_with_fade(fade);
                }
                self.update_tray_and_mpris_current(true);
            }
            PlaybackStatus::Buffering { .. } => {
                let fade = self.core.settings.playback.fade_in_out;
                if let Some(player) = &self.core.audio {
                    player.pause_with_fade(fade);
                }
                self.update_tray_and_mpris_current(false);
            }
        }

        Task::none()
    }

    fn update_tray_and_mpris(
        &mut self,
        is_playing: bool,
        title: Option<String>,
        artist: Option<String>,
    ) {
        update_tray_state_full(is_playing, title, artist, self.core.settings.play_mode);
        self.update_mpris_state();
    }

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

    fn apply_seek(&mut self) -> Task<Message> {
        if let Some(preview_pos) = self.ui.seek_preview_position.take() {
            if let Some(player) = &self.core.audio {
                let info = player.get_info();
                if info.duration.as_secs_f32() > 0.0 {
                    let seek_pos = std::time::Duration::from_secs_f32(
                        preview_pos * info.duration.as_secs_f32(),
                    );
                    player.seek(seek_pos);
                    self.update_mpris_state();
                } else if let Some(song) = &self.library.current_song {
                    let path = std::path::PathBuf::from(&song.file_path);
                    if !song.file_path.is_empty() && path.exists() {
                        let duration = song.duration_secs as f32;
                        player.play(path);
                        let seek_pos = std::time::Duration::from_secs_f32(preview_pos * duration);
                        player.seek(seek_pos);
                        self.update_mpris_state();
                    }
                }
            }
        }
        Task::none()
    }

    pub fn update_audio_tick(&self) {
        if let Some(player) = &self.core.audio {
            player.tick();
        }
    }

    fn handle_playback_tick(&mut self) -> Task<Message> {
        self.update_audio_tick();
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

        lyrics_scroll_task
    }

    pub fn handle_audio_event(&mut self, event: AudioEvent) -> Task<Message> {
        match event {
            AudioEvent::Started { path } => {
                tracing::debug!("AudioEvent::Started: {:?}", path);
            }
            AudioEvent::Paused { position } => {
                tracing::debug!("AudioEvent::Paused at {:?}", position);
            }
            AudioEvent::Resumed => {
                tracing::debug!("AudioEvent::Resumed");
            }
            AudioEvent::Stopped => {
                tracing::debug!("AudioEvent::Stopped");
            }
            AudioEvent::SeekComplete { position } => {
                tracing::debug!("AudioEvent::SeekComplete at {:?}", position);
            }
            AudioEvent::SeekFailed { error } => {
                tracing::warn!("Seek failed: {}", error);
                if error.contains("not supported") {
                    return Task::done(Message::ShowToast("该格式不支持拖动进度条".to_string()));
                }
                if error.contains("end of stream") || error.contains("streaming") {
                    let progress = self
                        .library
                        .streaming_buffer
                        .as_ref()
                        .map(|b| (b.progress() * 100.0) as u32)
                        .unwrap_or(0);
                    return Task::done(Message::ShowToast(format!(
                        "正在缓冲中 ({}%)，请稍候再拖动进度",
                        progress
                    )));
                }
            }
            AudioEvent::SeekStarted { target_position } => {
                tracing::debug!("AudioEvent::SeekStarted: target={:?}", target_position);
            }
            AudioEvent::StateChanged {
                old_status,
                new_status,
            } => {
                tracing::debug!(
                    "AudioEvent::StateChanged: {:?} -> {:?}",
                    old_status,
                    new_status
                );
            }
            AudioEvent::BufferProgress {
                downloaded,
                total,
                progress,
            } => {
                tracing::trace!(
                    "AudioEvent::BufferProgress: {}/{} ({:.1}%)",
                    downloaded,
                    total,
                    progress * 100.0
                );
            }
            AudioEvent::BufferingStarted { position } => {
                tracing::info!("AudioEvent::BufferingStarted at {:?}", position);
            }
            AudioEvent::BufferingEnded => {
                tracing::info!("AudioEvent::BufferingEnded");
            }
            AudioEvent::PreloadReady {
                request_id,
                duration,
                path,
            } => {
                tracing::debug!(
                    "AudioEvent::PreloadReady: request_id={}, path={:?}",
                    request_id,
                    path
                );
                self.handle_audio_preload_ready(request_id, duration, path);
            }
            AudioEvent::PreloadFailed { request_id, error } => {
                tracing::warn!("Preload failed: request_id={}, error={}", request_id, error);
            }
            AudioEvent::DeviceSwitched { restore_state } => {
                tracing::info!("Audio device switched: {:?}", restore_state);
            }
            AudioEvent::DeviceSwitchFailed { error } => {
                tracing::error!("Device switch failed: {}", error);
                return Task::done(Message::ShowErrorToast(format!(
                    "切换音频设备失败: {}",
                    error
                )));
            }
            AudioEvent::Finished => {
                tracing::info!("Song finished (AudioEvent::Finished)");
                if self.library.pending_resolution_idx.is_none()
                    && self.library.current_song.is_some()
                {
                    return self.handle_song_finished();
                }
            }
            AudioEvent::Error { message } => {
                tracing::error!("Audio error: {}", message);
                return Task::done(Message::ShowErrorToast(format!("播放错误: {}", message)));
            }
        }
        Task::none()
    }

    fn handle_streaming_event(
        &mut self,
        song_id: i64,
        event: crate::audio::streaming::StreamingEvent,
    ) -> Task<Message> {
        use crate::audio::streaming::StreamingEvent;

        let is_current = self
            .library
            .current_song
            .as_ref()
            .map(|s| s.id == song_id)
            .unwrap_or(false);

        if !is_current {
            return Task::none();
        }

        match event {
            StreamingEvent::Playable => {
                tracing::info!("Streaming: song {} is now playable", song_id);
            }
            StreamingEvent::Progress(downloaded, total) => {
                tracing::trace!("Streaming progress: {}/{} bytes", downloaded, total);
            }
            StreamingEvent::Complete => {
                tracing::info!("Streaming: song {} download complete", song_id);
            }
            StreamingEvent::Error(err) => {
                tracing::error!("Streaming error for song {}: {}", song_id, err);
                self.library.streaming_buffer = None;
                return Task::done(Message::ShowErrorToast(format!("下载失败: {}", err)));
            }
        }
        Task::none()
    }
}
