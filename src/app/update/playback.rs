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
            Message::StartPlayerEventListener => {
                // Take the receiver and start listening (only happens once)
                if let Some(rx) = self.core.player_event_rx.take() {
                    tracing::info!("Starting player event listener");
                    return Some(Task::run(
                        async_stream::stream! {
                            loop {
                                let event = rx.lock().await.recv().await;
                                if let Some(event) = event {
                                    yield event;
                                } else {
                                    tracing::info!("Player event channel closed");
                                    break;
                                }
                            }
                        },
                        Message::PlayerEvent,
                    ));
                }
                Some(Task::none())
            }

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

            // Streaming playback messages
            Message::StreamingEvent(song_id, event) => {
                Some(self.handle_streaming_event(*song_id, event.clone()))
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
                    let seek_pos = std::time::Duration::from_secs_f32(
                        self.ui.seek_preview_position * info.duration.as_secs_f32(),
                    );

                    // For streaming playback, check if we're seeking to unbuffered position
                    let is_streaming_seek = self.library.streaming_buffer.as_ref()
                        .map(|b| !b.is_complete())
                        .unwrap_or(false);

                    // Normal seek
                    match player.seek(seek_pos) {
                        Ok(_) => {
                            if !player.is_playing() {
                                player.update_paused_position(seek_pos);
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Seek failed: {}", e);
                            // For streaming playback, show a more helpful message
                            if is_streaming_seek && (e.contains("end of stream") || e.contains("No current path")) {
                                self.ui.is_seeking = false;
                                let progress = self.library.streaming_buffer.as_ref()
                                    .map(|b| (b.progress() * 100.0) as u32)
                                    .unwrap_or(0);
                                return Task::done(Message::ShowToast(
                                    format!("正在缓冲中 ({}%)，请稍候再拖动进度", progress),
                                ));
                            }
                            if e.contains("not supported") {
                                self.ui.is_seeking = false;
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

    /// Handle playback tick for auto-save, streaming buffer check, and finish detection
    fn handle_playback_tick(&mut self) -> Task<Message> {
        self.update_audio_fade();

        self.update_mpris_state();

        let lyrics_scroll_task = if self.ui.lyrics.is_open {
            self.update_lyrics_animations()
        } else {
            Task::none()
        };

        self.check_lyrics_page_close();

        // Check for buffering needs during streaming playback FIRST
        // This must happen before is_finished() check because sink.empty() can be true
        // when streaming data runs out but download is not complete
        if let Some(buffer) = &self.library.streaming_buffer {
            if !buffer.is_complete() {
                // Get playback info
                let (position_secs, duration_secs) = self.core.audio
                    .as_ref()
                    .map(|p| {
                        let info = p.get_info();
                        (info.position.as_secs_f64(), info.duration.as_secs_f64())
                    })
                    .unwrap_or((0.0, 0.0));
                
                let total_size = buffer.total_size();
                
                // Estimate current byte position
                let byte_pos = crate::audio::streaming::estimate_byte_position(
                    position_secs,
                    total_size,
                    duration_secs,
                );
                
                const BUFFER_AHEAD_BYTES: u64 = 128 * 1024; // 128KB
                let has_enough_buffer = buffer.has_buffer_ahead(byte_pos, BUFFER_AHEAD_BYTES);
                
                // Sync buffering state directly (no Message needed)
                // Note: We don't check is_playing here because when seeking to unbuffered position,
                // the decoder thread blocks on read_at() and playback appears stopped
                if !self.library.is_buffering && !has_enough_buffer {
                    // Start buffering: set state (playback may already be blocked)
                    self.library.is_buffering = true;
                    tracing::info!(
                        "Buffering started at position {:.1}s (byte {}, downloaded {})",
                        position_secs,
                        byte_pos,
                        buffer.downloaded()
                    );
                } else if self.library.is_buffering && has_enough_buffer {
                    // Buffer ready: resume playback
                    self.library.is_buffering = false;
                    if let Some(player) = &mut self.core.audio {
                        // Only resume if not already playing
                        if !player.is_playing() {
                            player.resume();
                            tracing::info!(
                                "Buffer ready, resuming playback at {:.1}s",
                                position_secs
                            );
                        }
                    }
                }
            } else if self.library.is_buffering {
                // Download complete, clear buffering state and ensure playback
                self.library.is_buffering = false;
                if let Some(player) = &mut self.core.audio {
                    if !player.is_playing() {
                        player.resume();
                        tracing::info!("Download complete, resuming playback");
                    }
                }
            }
        }

        // Check if song finished playing
        // For streaming songs, only consider finished if download is complete
        if let Some(player) = &self.core.audio {
            let is_streaming_incomplete = self
                .library
                .streaming_buffer
                .as_ref()
                .map(|b| !b.is_complete())
                .unwrap_or(false);

            // If streaming is incomplete, don't use is_finished() which relies on sink.empty()
            // Instead, wait for download to complete
            if !is_streaming_incomplete && player.is_finished() {
                // Don't trigger if we're waiting for a song to be resolved
                if self.library.pending_resolution_idx.is_none() && self.library.current_song.is_some() {
                    tracing::info!("Song finished (detected in PlaybackTick), triggering next song");
                    return self.handle_song_finished();
                }
            }
        }

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

    /// Handle player events (event-driven architecture)
    /// Note: Streaming events are handled separately via Message::StreamingEvent
    /// Note: Finish detection is done via polling, not events
    pub fn handle_player_event(&mut self, event: crate::audio::PlayerEvent) -> Task<Message> {
        use crate::audio::PlayerEvent;

        match event {
            PlayerEvent::Started { path } => {
                tracing::debug!("PlayerEvent::Started: {:?}", path);
                Task::none()
            }
            PlayerEvent::StreamingStarted => {
                tracing::debug!("PlayerEvent::StreamingStarted");
                Task::none()
            }
        }
    }

    /// Handle streaming download events
    fn handle_streaming_event(
        &mut self,
        song_id: i64,
        event: crate::audio::streaming::StreamingEvent,
    ) -> Task<Message> {
        use crate::audio::streaming::StreamingEvent;

        // Only handle events for the current song
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
                tracing::trace!(
                    "Streaming progress: {}/{} bytes ({:.1}%)",
                    downloaded,
                    total,
                    if total > 0 {
                        downloaded as f64 / total as f64 * 100.0
                    } else {
                        0.0
                    }
                );
            }
            StreamingEvent::Complete => {
                tracing::info!("Streaming: song {} download complete", song_id);
                self.library.is_buffering = false;
            }
            StreamingEvent::Error(err) => {
                tracing::error!("Streaming error for song {}: {}", song_id, err);
                self.library.streaming_buffer = None;
                self.library.is_buffering = false;
                return Task::done(Message::ShowErrorToast(format!("下载失败: {}", err)));
            }
        }

        Task::none()
    }
}
