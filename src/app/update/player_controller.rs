// src/app/update/player_controller.rs
//! Unified player controller for all playback operations
//!
//! Uses QueueNavigator as Single Source of Truth for index calculations.

use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use iced::Task;

use crate::app::message::Message;
use crate::app::state::{App, PendingPlaybackKind, PendingPlaybackRequest};
use crate::database::DbSong;
use crate::features::PlayMode;
use crate::i18n::Key;

use super::audio_preload_manager::PreloadDirection;
use super::preload_coordinator::WindowChange;
use super::queue_navigator::QueueNavigator;
use super::song_resolver::ResolvedSong;

#[derive(Clone, Copy)]
pub(super) enum TrackGainMode {
    MetadataOnly,
    AnalyzeIfMissing,
}

pub(super) enum PlaybackSource {
    AudioPath {
        path: PathBuf,
        gain_mode: TrackGainMode,
        start_position: Option<std::time::Duration>,
    },
    StreamingBuffer {
        buffer: crate::audio::SharedBuffer,
        duration_secs: Option<u64>,
        file_path: String,
    },
    Preloaded {
        request_id: u64,
        path: PathBuf,
        buffer: Option<crate::audio::SharedBuffer>,
    },
}

/// Maximum consecutive failures before stopping playback
const MAX_CONSECUTIVE_FAILURES: u8 = 3;

impl App {
    fn resolve_artist_id_for_song(&self, song: &DbSong) -> Option<u64> {
        let ncm_id = if song.id < 0 {
            Some((-song.id) as u64)
        } else {
            None
        };

        self.ui
            .home
            .current_ncm_playlist_songs
            .iter()
            .chain(self.ui.home.trending_songs.iter())
            .chain(self.ui.search.songs.iter())
            .find(|candidate| {
                if let Some(id) = ncm_id {
                    candidate.id == id
                } else {
                    candidate.name == song.title
                        && candidate.singer == song.artist
                        && candidate.album == song.album
                }
            })
            .and_then(|song| (song.singer_id != 0).then_some(song.singer_id))
    }

    pub(super) fn effective_queue_play_mode(&self) -> PlayMode {
        if self.is_fm_mode() {
            PlayMode::Sequential
        } else {
            self.core.settings.play_mode
        }
    }

    pub(super) fn replace_active_streaming_buffer(
        &mut self,
        next_buffer: Option<crate::audio::SharedBuffer>,
    ) {
        if let Some(old_buffer) = self.playback.active_streaming_buffer.take() {
            old_buffer.cancel();
            tracing::debug!("Cancelled previous streaming buffer");
        }

        self.playback.active_streaming_buffer = next_buffer;
    }

    fn cache_track_gain_in_memory(&mut self, song: &DbSong, normalization_gain: f64) {
        let update_song = |candidate: &mut DbSong| {
            candidate.normalization_gain = Some(normalization_gain);
        };

        for candidate in &mut self.playback.queue {
            if candidate.id == song.id || candidate.file_path == song.file_path {
                update_song(candidate);
            }
        }

        for candidate in &mut self.library.db_songs {
            if candidate.id == song.id || candidate.file_path == song.file_path {
                update_song(candidate);
            }
        }

        for candidate in &mut self.library.recently_played {
            if candidate.id == song.id || candidate.file_path == song.file_path {
                update_song(candidate);
            }
        }

        if let Some(current_song) = &mut self.playback.current_song {
            if current_song.id == song.id || current_song.file_path == song.file_path {
                update_song(current_song);
            }
        }
    }

    fn fade_in_enabled(&self) -> bool {
        self.core.settings.playback.fade_in_out
    }

    fn resolve_queue_index_for_song(&self, song: &DbSong) -> Option<usize> {
        self.playback
            .current_index
            .filter(|&idx| {
                self.playback
                    .queue
                    .get(idx)
                    .map(|candidate| {
                        candidate.id == song.id || candidate.file_path == song.file_path
                    })
                    .unwrap_or(false)
            })
            .or_else(|| {
                self.playback.queue.iter().position(|candidate| {
                    candidate.id == song.id || candidate.file_path == song.file_path
                })
            })
    }

    fn queue_pending_playback_request(
        &mut self,
        request_id: u64,
        queue_index: Option<usize>,
        song: DbSong,
        kind: PendingPlaybackKind,
    ) {
        if let Some(idx) = queue_index {
            self.playback.current_index = Some(idx);
        }

        self.playback.pending_playback_request = Some(PendingPlaybackRequest {
            request_id,
            queue_index,
            song,
            kind,
        });
    }

    fn take_pending_playback_request(&mut self, request_id: u64) -> Option<PendingPlaybackRequest> {
        if self
            .playback
            .pending_playback_request
            .as_ref()
            .is_some_and(|pending| pending.request_id == request_id)
        {
            self.playback.pending_playback_request.take()
        } else {
            None
        }
    }

    fn restore_current_playback_position(&mut self, position_secs: f64) {
        let seek_pos = std::time::Duration::from_secs_f64(position_secs);
        if self.playback_output_available() {
            self.seek_audio_output(seek_pos);
            tracing::info!("Restored playback position to {:?}", seek_pos);
        }
        if let Some(state) = &mut self.playback.saved_state {
            state.position_secs = 0.0;
        }
    }

    pub(super) fn apply_resolved_song_to_queue(
        &mut self,
        idx: usize,
        resolved: &ResolvedSong,
    ) -> Option<DbSong> {
        let song = self.playback.queue.get_mut(idx)?;
        song.file_path = resolved.file_path.clone();
        if let Some(cover_path) = &resolved.cover_path {
            song.cover_path = Some(cover_path.clone());
        }

        if let Some(db) = &self.core.db {
            let db = db.clone();
            let song_clone = song.clone();
            tokio::spawn(async move {
                let _ = db.upsert_ncm_song(&song_clone).await;
            });
        }

        Some(song.clone())
    }

    fn playback_source_from_resolved_song(
        song: &DbSong,
        resolved: &ResolvedSong,
        restore_position: Option<f64>,
    ) -> Result<PlaybackSource, String> {
        if let Some(buffer) = resolved.shared_buffer.clone() {
            return Ok(PlaybackSource::StreamingBuffer {
                buffer,
                duration_secs: resolved.duration_secs,
                file_path: resolved.file_path.clone(),
            });
        }

        if let Some(position) = restore_position {
            Self::audio_path_source_for_song_at_position(
                song,
                std::time::Duration::from_secs_f64(position),
            )
        } else {
            Self::audio_path_source_for_song(song)
        }
    }

    pub(super) fn resolve_track_gain_for_song(
        &mut self,
        song: &DbSong,
        gain_mode: TrackGainMode,
    ) -> f32 {
        if !self.core.settings.playback.volume_normalization {
            return 1.0;
        }

        if let Some(cached_gain) = song.normalization_gain {
            return cached_gain as f32;
        }

        let path = Path::new(&song.file_path);
        if !path.exists() {
            return 1.0;
        }

        // Try metadata tags first (fast, synchronous)
        if let Some(tagged_gain) = crate::features::extract_track_gain(path) {
            let gain = tagged_gain as f64;
            self.cache_track_gain_in_memory(song, gain);
            if let Some(db) = &self.core.db {
                let db = db.clone();
                let song_id = song.id;
                let file_path = song.file_path.clone();
                tokio::spawn(async move {
                    let _ = db.update_song_normalization(song_id, &file_path, gain).await;
                });
            }
            return tagged_gain;
        }

        // No tags available — launch waveform analysis in background if mode allows it.
        // Play immediately with unity gain to avoid blocking the UI thread.
        // Result is saved to DB for the next session; no in-memory update needed
        // because the gain won't apply until the next play or restart anyway.
        if matches!(gain_mode, TrackGainMode::AnalyzeIfMissing) {
            let path_buf = path.to_path_buf();
            let db = self.core.db.clone();
            let song_id = song.id;
            let file_path = song.file_path.clone();

            tokio::spawn(async move {
                let result = tokio::task::spawn_blocking(move || {
                    crate::features::resolve_track_gain(&path_buf)
                })
                .await;

                if let Ok(Some(gain)) = result {
                    if let Some(db) = db {
                        let _ = db
                            .update_song_normalization(song_id, &file_path, gain as f64)
                            .await;
                    }
                }
            });
        }

        1.0
    }

    pub(super) fn play_audio_path_for_song(
        &mut self,
        song: &DbSong,
        path: PathBuf,
        gain_mode: TrackGainMode,
    ) -> Result<u64, String> {
        let track_gain = self.resolve_track_gain_for_song(song, gain_mode);
        let fade_in = self.fade_in_enabled();
        self.play_audio_file(path, fade_in, track_gain)
    }

    pub(super) fn play_audio_path_at_position_for_song(
        &mut self,
        song: &DbSong,
        path: PathBuf,
        gain_mode: TrackGainMode,
        position: std::time::Duration,
    ) -> Result<u64, String> {
        let track_gain = self.resolve_track_gain_for_song(song, gain_mode);
        let fade_in = self.fade_in_enabled();
        self.play_audio_file_at_position(path, position, fade_in, track_gain)
    }

    pub(super) fn play_streaming_buffer_for_song(
        &mut self,
        song: &DbSong,
        buffer: crate::audio::SharedBuffer,
        duration_secs: Option<u64>,
        file_path: &str,
    ) -> Result<u64, String> {
        let track_gain = self.resolve_track_gain_for_song(song, TrackGainMode::MetadataOnly);
        let fade_in = self.fade_in_enabled();
        let duration =
            std::time::Duration::from_secs(duration_secs.unwrap_or(song.duration_secs as u64));
        let cache_path = (!file_path.is_empty()).then(|| PathBuf::from(file_path));
        self.play_streaming_audio(buffer, duration, cache_path, fade_in, track_gain)
    }

    pub(super) fn play_preloaded_request(
        &self,
        request_id: u64,
        path: PathBuf,
    ) -> Result<u64, String> {
        self.play_preloaded_audio(request_id, path, self.fade_in_enabled())
    }

    pub(super) fn audio_path_source_for_song(song: &DbSong) -> Result<PlaybackSource, String> {
        let path = PathBuf::from(&song.file_path);
        if song.file_path.is_empty() || !path.exists() {
            return Err(format!("File not found: {}", song.file_path));
        }

        Ok(PlaybackSource::AudioPath {
            path,
            gain_mode: TrackGainMode::AnalyzeIfMissing,
            start_position: None,
        })
    }

    pub(super) fn audio_path_source_for_song_at_position(
        song: &DbSong,
        position: std::time::Duration,
    ) -> Result<PlaybackSource, String> {
        let path = PathBuf::from(&song.file_path);
        if song.file_path.is_empty() || !path.exists() {
            return Err(format!("File not found: {}", song.file_path));
        }

        Ok(PlaybackSource::AudioPath {
            path,
            gain_mode: TrackGainMode::AnalyzeIfMissing,
            start_position: Some(position),
        })
    }

    pub(super) fn start_audio_source_for_song(
        &mut self,
        song: &DbSong,
        source: PlaybackSource,
    ) -> Result<u64, String> {
        match source {
            PlaybackSource::AudioPath {
                path,
                gain_mode,
                start_position,
            } => {
                self.replace_active_streaming_buffer(None);
                if let Some(position) = start_position {
                    self.play_audio_path_at_position_for_song(song, path, gain_mode, position)
                } else {
                    self.play_audio_path_for_song(song, path, gain_mode)
                }
            }
            PlaybackSource::StreamingBuffer {
                buffer,
                duration_secs,
                file_path,
            } => {
                let active_buffer = buffer.clone();
                self.replace_active_streaming_buffer(Some(active_buffer));
                self.play_streaming_buffer_for_song(song, buffer, duration_secs, &file_path)
            }
            PlaybackSource::Preloaded {
                request_id,
                path,
                buffer,
            } => {
                self.replace_active_streaming_buffer(buffer);
                self.play_preloaded_request(request_id, path)
            }
        }
    }

    pub(super) fn load_audio_path_paused_for_song(
        &mut self,
        song: &DbSong,
        path: PathBuf,
        position: std::time::Duration,
    ) -> Result<(), String> {
        let track_gain = self.resolve_track_gain_for_song(song, TrackGainMode::AnalyzeIfMissing);
        let request_id = self.load_audio_file_paused(path, position, track_gain)?;
        let queue_index = self.resolve_queue_index_for_song(song);
        self.queue_pending_playback_request(
            request_id,
            queue_index,
            song.clone(),
            PendingPlaybackKind::LoadPausedTrack,
        );
        self.replace_active_streaming_buffer(None);
        Ok(())
    }

    pub(super) fn load_streaming_buffer_paused_for_song(
        &mut self,
        song: &DbSong,
        buffer: crate::audio::SharedBuffer,
        duration_secs: Option<u64>,
        file_path: &str,
        position: std::time::Duration,
    ) -> Result<(), String> {
        let track_gain = self.resolve_track_gain_for_song(song, TrackGainMode::MetadataOnly);
        let duration =
            std::time::Duration::from_secs(duration_secs.unwrap_or(song.duration_secs as u64));
        let cache_path = (!file_path.is_empty()).then(|| PathBuf::from(file_path));
        let request_id = self.load_streaming_audio_paused(
            buffer.clone(),
            duration,
            cache_path,
            position,
            track_gain,
        )?;
        let queue_index = self.resolve_queue_index_for_song(song);
        self.queue_pending_playback_request(
            request_id,
            queue_index,
            song.clone(),
            PendingPlaybackKind::LoadPausedTrack,
        );
        self.replace_active_streaming_buffer(Some(buffer));
        Ok(())
    }

    pub(super) fn restart_audio_path_for_song_at_position(
        &mut self,
        song: &DbSong,
        position: std::time::Duration,
    ) -> Result<(), String> {
        let source = Self::audio_path_source_for_song_at_position(song, position)?;
        let request_id = self.start_audio_source_for_song(song, source)?;
        let queue_index = self.resolve_queue_index_for_song(song);
        self.queue_pending_playback_request(
            request_id,
            queue_index,
            song.clone(),
            PendingPlaybackKind::RestartCurrentTrack,
        );
        Ok(())
    }

    pub(super) fn start_queue_song_from_source(
        &mut self,
        idx: usize,
        song: DbSong,
        source: PlaybackSource,
    ) -> Result<Task<Message>, String> {
        let request_id = self.start_audio_source_for_song(&song, source)?;
        self.queue_pending_playback_request(
            request_id,
            Some(idx),
            song,
            PendingPlaybackKind::StartPlayingTrack,
        );
        Ok(Task::none())
    }

    /// Central method to play a song at a specific queue index
    pub fn play_song_at_index(&mut self, idx: usize) -> Task<Message> {
        if idx >= self.playback.queue.len() {
            tracing::warn!("Invalid queue index: {}", idx);
            return Task::none();
        }

        let song = self.playback.queue[idx].clone();

        if super::song_resolver::needs_resolution(&song) {
            self.playback.current_index = Some(idx);
            tracing::info!("Song {} needs resolution", song.title);
            return self.resolve_and_play(idx, song);
        }

        let source = match Self::audio_path_source_for_song(&song) {
            Ok(source) => source,
            Err(err) => {
                tracing::error!("Failed to play {}: {}", song.title, err);
                return self.handle_playback_failure(idx, &err);
            }
        };

        match self.start_queue_song_from_source(idx, song.clone(), source) {
            Ok(task) => task,
            Err(err) => {
                tracing::error!("Failed to play {}: {}", song.title, err);
                self.handle_playback_failure(idx, &err)
            }
        }
    }

    /// Handle playback failure with consecutive failure detection
    /// Design: Skip failed songs and continue to next, show toast after MAX_CONSECUTIVE_FAILURES
    pub fn handle_playback_failure(&mut self, failed_idx: usize, error: &str) -> Task<Message> {
        self.playback.consecutive_failures += 1;

        tracing::warn!(
            "Playback failure {} of {}: {}",
            self.playback.consecutive_failures,
            MAX_CONSECUTIVE_FAILURES,
            error
        );

        // Show warning after too many consecutive failures
        let toast_task = if self.playback.consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
            tracing::warn!(
                "Consecutive failures reached {}, showing warning and stopping retry",
                self.playback.consecutive_failures
            );

            Self::toast_error(format!(
                "连续 {} 首歌曲播放失败，已停止播放",
                MAX_CONSECUTIVE_FAILURES
            ))
        } else {
            Task::none()
        };

        // Only skip to next if we haven't exceeded max failures
        // This prevents infinite loop when all songs fail
        if self.playback.consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
            // Stop trying - reset counter for next user-initiated play
            self.playback.consecutive_failures = 0;
            return toast_task;
        }

        // Skip to next playable song
        Task::batch([toast_task, self.skip_to_next_playable(failed_idx)])
    }

    fn save_playback_position_snapshot(&self, song_id: i64, queue_pos: i64, position_secs: f64) {
        if let Some(db) = &self.core.db {
            let db = db.clone();
            tokio::spawn(async move {
                let _ = db
                    .update_playback_position(Some(song_id), queue_pos, position_secs)
                    .await;
            });
        }
    }

    fn commit_current_song_playback_state(
        &mut self,
        queue_index: Option<usize>,
        song: DbSong,
        is_playing: bool,
    ) {
        if let Some(idx) = queue_index {
            self.playback.current_index = Some(idx);
        }

        self.playback.current_artist_id = self.resolve_artist_id_for_song(&song);
        self.playback.current_song = Some(song);
        self.playback.consecutive_failures = 0;
        self.update_tray_and_mpris_current(is_playing);
    }

    fn on_song_loaded_paused(&mut self, idx: usize, song: DbSong) -> Task<Message> {
        tracing::info!("Loaded paused song: {} - {}", song.title, song.artist);

        let (song, needs_cover_download) = self.ensure_local_cover_path_with_download(idx, song);
        self.commit_current_song_playback_state(Some(idx), song.clone(), false);
        self.cache_shuffle_indices();
        self.refresh_preload_window();
        self.schedule_post_switch_side_effects(&song, needs_cover_download)
    }

    pub fn handle_audio_started_event(
        &mut self,
        request_id: u64,
        path: Option<PathBuf>,
    ) -> Task<Message> {
        tracing::debug!(
            "AudioEvent::Started: request_id={}, path={:?}",
            request_id,
            path
        );

        let Some(pending) = self.take_pending_playback_request(request_id) else {
            tracing::debug!(
                "Ignoring Started for stale/unknown request_id={}",
                request_id
            );
            return Task::none();
        };

        match pending.kind {
            PendingPlaybackKind::StartPlayingTrack => {
                if let Some(idx) = pending.queue_index {
                    self.on_song_started(idx, pending.song)
                } else {
                    self.commit_current_song_playback_state(None, pending.song, true);
                    Task::none()
                }
            }
            PendingPlaybackKind::RestartCurrentTrack => {
                self.commit_current_song_playback_state(pending.queue_index, pending.song, true);
                Task::none()
            }
            PendingPlaybackKind::LoadPausedTrack => {
                tracing::debug!("Ignoring Started for paused-load request_id={}", request_id);
                Task::none()
            }
        }
    }

    pub fn handle_audio_paused_event(
        &mut self,
        request_id: Option<u64>,
        position: std::time::Duration,
    ) -> Task<Message> {
        tracing::debug!(
            "AudioEvent::Paused: request_id={:?}, position={:?}",
            request_id,
            position
        );

        if let Some(request_id) = request_id {
            let Some(pending) = self.take_pending_playback_request(request_id) else {
                tracing::debug!(
                    "Ignoring Paused for stale/unknown request_id={}",
                    request_id
                );
                return Task::none();
            };

            return match pending.kind {
                PendingPlaybackKind::LoadPausedTrack => {
                    if let Some(idx) = pending.queue_index {
                        self.on_song_loaded_paused(idx, pending.song)
                    } else {
                        self.commit_current_song_playback_state(None, pending.song, false);
                        Task::none()
                    }
                }
                PendingPlaybackKind::RestartCurrentTrack => {
                    self.commit_current_song_playback_state(
                        pending.queue_index,
                        pending.song,
                        false,
                    );
                    Task::none()
                }
                PendingPlaybackKind::StartPlayingTrack => {
                    tracing::debug!(
                        "Ignoring Paused for playing-track request_id={}",
                        request_id
                    );
                    Task::none()
                }
            };
        }

        if let (Some(song), Some(queue_index)) =
            (&self.playback.current_song, self.playback.current_index)
        {
            self.save_playback_position_snapshot(
                song.id,
                queue_index as i64,
                position.as_secs_f64(),
            );
        }
        self.update_tray_and_mpris_current(false);
        Task::none()
    }

    pub fn handle_audio_resumed_event(&mut self) -> Task<Message> {
        tracing::debug!("AudioEvent::Resumed");
        self.update_tray_and_mpris_current(true);
        Task::none()
    }

    pub fn handle_audio_stopped_event(&mut self) -> Task<Message> {
        tracing::debug!("AudioEvent::Stopped");
        if self.playback.pending_playback_request.is_none() {
            self.update_tray_and_mpris_current(false);
        }
        Task::none()
    }

    pub fn handle_audio_error_event(
        &mut self,
        request_id: Option<u64>,
        message: String,
        error_kind: Option<crate::audio::PlaybackError>,
    ) -> Task<Message> {
        tracing::error!(
            "Audio error: request_id={:?}, message={}, kind={:?}",
            request_id,
            message,
            error_kind
        );

        let toast_message = match error_kind {
            Some(crate::audio::PlaybackError::UnsupportedFormat(_)) => {
                "不支持的音频格式".to_string()
            }
            Some(crate::audio::PlaybackError::FileNotFound(_)) => "文件不存在".to_string(),
            Some(crate::audio::PlaybackError::IoError(_)) => "文件读取出错".to_string(),
            _ => format!("播放错误: {}", message),
        };

        if let Some(request_id) = request_id {
            let Some(pending) = self.take_pending_playback_request(request_id) else {
                tracing::debug!("Ignoring Error for stale/unknown request_id={}", request_id);
                return Task::none();
            };

            return match pending.kind {
                PendingPlaybackKind::StartPlayingTrack => {
                    if let Some(idx) = pending.queue_index {
                        self.handle_playback_failure(idx, &message)
                    } else {
                        Self::toast_error(toast_message)
                    }
                }
                PendingPlaybackKind::LoadPausedTrack | PendingPlaybackKind::RestartCurrentTrack => {
                    Self::toast_error(toast_message)
                }
            };
        }

        Self::toast_error(toast_message)
    }

    pub fn pause_current_playback(&mut self) {
        self.refresh_playback_runtime();

        if let (Some(song), Some(queue_index)) =
            (&self.playback.current_song, self.playback.current_index)
        {
            let position = self.playback_info().position.as_secs_f64();
            self.save_playback_position_snapshot(song.id, queue_index as i64, position);
        }

        self.pause_audio_output_with_fade(self.fade_in_enabled());
    }

    pub fn resume_current_playback(&mut self) {
        self.resume_audio_output_with_fade(self.fade_in_enabled());
    }

    pub fn stop_audio_output(&mut self) {
        self.playback.pending_playback_request = None;
        self.stop_audio_backend();
    }

    pub fn stop_and_clear_current_playback(&mut self) {
        self.stop_audio_output();
        self.playback.current_song = None;
        self.playback.current_artist_id = None;
        self.playback.preload_coordinator.clear_window();
        let released = self.playback.audio_preload_manager.reset();
        self.release_preload_requests(released);
    }

    pub fn seek_to_position(&mut self, position: std::time::Duration) {
        self.seek_audio_output(position);
    }

    pub fn seek_by_offset(&mut self, offset: std::time::Duration, forward: bool) {
        if !self.playback_output_available() {
            return;
        }

        self.refresh_playback_runtime();
        let info = self.playback_info().clone();
        let new_pos = if forward {
            if info.duration.is_zero() {
                return;
            }
            (info.position + offset).min(info.duration)
        } else {
            info.position.saturating_sub(offset)
        };
        self.seek_audio_output(new_pos);
    }

    pub fn toggle_current_playback(&mut self) -> Task<Message> {
        use crate::audio::PlaybackStatus;

        if !self.playback_output_available() {
            tracing::warn!("toggle_playback: No audio player");
            return Task::none();
        }

        self.refresh_playback_runtime();
        let status = self.playback_status();

        tracing::info!("toggle_playback: current status = {:?}", status);

        match status {
            PlaybackStatus::Stopped => {
                if let Some(idx) = self.playback.current_index {
                    return self.play_song_at_index(idx);
                }

                if let Some(song) = self.playback.current_song.clone() {
                    let is_ncm = song.id < 0
                        || song.file_path.is_empty()
                        || song.file_path.starts_with("ncm://");
                    if is_ncm {
                        tracing::warn!("Cannot play NCM song without queue index");
                        return Task::none();
                    }

                    let playback_pos = self
                        .playback
                        .saved_state
                        .as_ref()
                        .filter(|s| s.position_secs > 0.0)
                        .map(|s| std::time::Duration::from_secs_f64(s.position_secs));
                    let has_saved_position = playback_pos.is_some();

                    let source = if let Some(position) = playback_pos {
                        Self::audio_path_source_for_song_at_position(&song, position)
                    } else {
                        Self::audio_path_source_for_song(&song)
                    };

                    if let Ok(source) = source {
                        if let Ok(request_id) = self.start_audio_source_for_song(&song, source) {
                            let queue_index = self.resolve_queue_index_for_song(&song);
                            self.queue_pending_playback_request(
                                request_id,
                                queue_index,
                                song,
                                PendingPlaybackKind::RestartCurrentTrack,
                            );
                            if has_saved_position {
                                if let Some(state) = &mut self.playback.saved_state {
                                    state.position_secs = 0.0;
                                }
                            }
                        }
                    }
                }
                Task::none()
            }
            PlaybackStatus::Playing | PlaybackStatus::Buffering { .. } => {
                self.pause_current_playback();
                Task::none()
            }
            PlaybackStatus::Paused => {
                self.resume_current_playback();
                Task::none()
            }
        }
    }

    /// Refresh coordinator preload window from current playback state.
    /// Uses refresh_window_with_indices to avoid QueueNavigator borrow conflicts.
    pub(super) fn refresh_preload_window(&mut self) -> WindowChange {
        let adjacent = {
            let nav = QueueNavigator::new(
                self.playback.queue.len(),
                self.playback.current_index,
                self.effective_queue_play_mode(),
                &self.playback.shuffle_cache,
            );
            (nav.adjacent_indices(), nav.current_index())
        };
        let (adjacent, current_index) = adjacent;
        let current_song_id = self.playback.current_song.as_ref().map(|s| s.id);
        let next_song_id = adjacent
            .next
            .and_then(|idx| self.playback.queue.get(idx))
            .map(|s| s.id);
        let prev_song_id = adjacent
            .prev
            .and_then(|idx| self.playback.queue.get(idx))
            .map(|s| s.id);
        let change = self
            .playback
            .preload_coordinator
            .refresh_window_with_indices(
                current_song_id,
                current_index,
                adjacent.next,
                next_song_id,
                adjacent.prev,
                prev_song_id,
            );
        // Sync render manager: keep only entries for songs in the current window
        let window = self.playback.preload_coordinator.window();
        let keep_ids: Vec<i64> = [
            window.current_song_id,
            window.next_song_id,
            window.prev_song_id,
        ]
        .into_iter()
        .flatten()
        .collect();
        self.playback.lyrics_render_manager.retain(&keep_ids);
        change
    }

    fn on_song_started(&mut self, idx: usize, song: DbSong) -> Task<Message> {
        tracing::info!("Playing: {} - {}", song.title, song.artist);

        // Ensure cover path is local (not remote URL)
        let (song, needs_cover_download) = self.ensure_local_cover_path_with_download(idx, song);
        self.commit_current_song_playback_state(Some(idx), song.clone(), true);

        if let Some(db) = &self.core.db {
            let db = db.clone();
            let song_id = song.id;
            tokio::spawn(async move {
                let _ = db.record_play(song_id, 0, false).await;
            });
        }

        if let Some(db) = &self.core.db {
            let db = db.clone();
            let song_id = song.id;
            let queue_pos = idx as i64;
            tokio::spawn(async move {
                let _ = db
                    .update_playback_position(Some(song_id), queue_pos, 0.0)
                    .await;
            });
        }

        // Refresh preload coordinator window
        self.cache_shuffle_indices();
        self.refresh_preload_window();
        self.schedule_post_switch_side_effects(&song, needs_cover_download)
    }

    /// 为当前歌曲加载歌词和背景（歌词页面打开时调用）
    fn load_lyrics_for_current_song(&mut self, song: &DbSong) -> Task<Message> {
        // 使用统一的异步加载方法
        self.load_lyrics_async(song)
    }

    /// Playback-side prefetch coordinator after a track switch completes.
    fn schedule_post_switch_side_effects(
        &mut self,
        song: &DbSong,
        needs_cover_download: Option<(u64, String)>,
    ) -> Task<Message> {
        let audio_task = self.preload_adjacent_tracks_with_ncm();
        let cover_task = if let Some((ncm_id, cover_url)) = needs_cover_download {
            self.download_current_song_cover(song.id, ncm_id, cover_url)
        } else {
            Task::none()
        };
        let lyrics_task = self.schedule_lyrics_prefetches(song);
        let bg_task = self.schedule_background_prep();

        Task::batch([audio_task, cover_task, lyrics_task, bg_task])
    }

    /// Schedule current-song lyrics display loading plus background cache warmup.
    fn schedule_lyrics_prefetches(&mut self, current_song: &DbSong) -> Task<Message> {
        let mut tasks = Vec::new();

        if self.ui.lyrics.is_open {
            // Render-ready source of truth: LyricsRenderManager
            let lyrics_ready = self.current_lyrics_shape_metrics().is_some_and(|(cw, fs)| {
                self.playback
                    .lyrics_render_manager
                    .is_render_ready(current_song.id, cw, fs)
            });

            if lyrics_ready {
                if self.install_current_lyrics_render_if_ready(current_song.id) {
                    tracing::debug!(
                        "Lyrics render-ready for current song {}, installed without prefetch",
                        current_song.id
                    );
                }
            } else {
                tasks.push(self.load_lyrics_for_current_song(current_song));
            }
        } else {
            tasks.push(self.warm_lyrics_cache_for_song(current_song));
        }

        // Use coordinator window for adjacent indices
        let window = self.playback.preload_coordinator.window();
        let mut scheduled_song_ids = HashSet::from([current_song.id]);

        for idx in [window.next_index, window.prev_index].into_iter().flatten() {
            let Some(candidate) = self.playback.queue.get(idx).cloned() else {
                continue;
            };

            if !scheduled_song_ids.insert(candidate.id) {
                continue;
            }

            tasks.push(self.warm_lyrics_cache_for_song(&candidate));
        }

        // When lyrics page is open, also trigger render prep for adjacent songs
        if self.ui.lyrics.is_open {
            tasks.push(self.schedule_adjacent_lyrics_render_prep());
        }

        Task::batch(tasks)
    }

    /// Ensure song has local cover path instead of remote URL
    /// If cover is cached locally, update the song's cover_path
    /// Returns (song, Option<(ncm_id, cover_url)>, needs_refetch) - the second value indicates if download is needed
    /// If needs_refetch is true, we need to fetch cover URL from API
    fn ensure_local_cover_path_with_download(
        &mut self,
        idx: usize,
        mut song: DbSong,
    ) -> (DbSong, Option<(u64, String)>) {
        // Only process NCM songs (negative ID)
        if song.id >= 0 {
            return (song, None);
        }

        let ncm_id = (-song.id) as u64;
        let cover_cache_dir = crate::utils::covers_cache_dir();
        let stem = format!("cover_{}", ncm_id);

        tracing::debug!(
            "ensure_local_cover_path_with_download: song_id={}, ncm_id={}, current_cover={:?}",
            song.id,
            ncm_id,
            song.cover_path
        );

        // Check if cover already exists locally
        if let Some(local_path) = crate::utils::find_cached_image(&cover_cache_dir, &stem) {
            let local_path_str = local_path.to_string_lossy().to_string();
            tracing::info!("Found local cover at: {}", local_path_str);

            // Update song's cover_path if it's different (was URL or different path)
            if song.cover_path.as_ref() != Some(&local_path_str) {
                song.cover_path = Some(local_path_str.clone());

                // Also update in queue
                if let Some(queue_song) = self.playback.queue.get_mut(idx) {
                    queue_song.cover_path = Some(local_path_str);
                }
            }
            return (song, None);
        }

        // Cover not found locally - check if we have a URL to download
        if let Some(url) = song
            .cover_path
            .clone()
            .filter(|url| url.starts_with("http"))
        {
            tracing::info!("Cover needs download for song_id={}", song.id);
            return (song, Some((ncm_id, url)));
        }

        // No http URL available - need to refetch from API
        // This happens when cache was cleared but cover_path in DB is local path
        tracing::info!(
            "Cover cache cleared, need to refetch URL from API for song_id={}, ncm_id={}",
            song.id,
            ncm_id
        );

        // Mark that we need to refetch - use a special marker
        // The download_current_song_cover will handle the API call
        (song, Some((ncm_id, String::new())))
    }

    /// Download cover for current playing song
    /// If cover_url is empty, fetch from API first
    fn download_current_song_cover(
        &self,
        song_id: i64,
        ncm_id: u64,
        cover_url: String,
    ) -> Task<Message> {
        if let Some(client) = &self.core.ncm_client {
            let client = client.clone();
            Task::perform(
                async move {
                    // If cover_url is empty, we need to fetch it from API first
                    let actual_url = if cover_url.is_empty() {
                        tracing::info!("Fetching cover URL from API for ncm_id={}", ncm_id);
                        // Try to get song detail to get cover URL
                        match client.song_detail(&[ncm_id]).await {
                            Ok(songs) if !songs.is_empty() => {
                                let url = songs[0].pic_url.clone();
                                if url.is_empty() {
                                    tracing::warn!(
                                        "API returned empty cover URL for ncm_id={}",
                                        ncm_id
                                    );
                                    return None;
                                }
                                tracing::info!("Got cover URL from API: {}", url);
                                url
                            }
                            Ok(_) => {
                                tracing::warn!("No song detail found for ncm_id={}", ncm_id);
                                return None;
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to fetch song detail for ncm_id={}: {}",
                                    ncm_id,
                                    e
                                );
                                return None;
                            }
                        }
                    } else {
                        cover_url
                    };

                    if let Some(path) =
                        crate::utils::download_cover(&client, ncm_id, &actual_url).await
                    {
                        Some((song_id, path.to_string_lossy().to_string()))
                    } else {
                        None
                    }
                },
                |result| {
                    if let Some((song_id, path)) = result {
                        Message::CurrentSongCoverReady(song_id, path)
                    } else {
                        Message::NoOp
                    }
                },
            )
        } else {
            Task::none()
        }
    }

    /// 预计算并缓存 shuffle 模式的 next/prev 索引
    /// 确保预加载和实际播放使用相同的索引
    pub fn cache_shuffle_indices(&mut self) {
        let queue_len = self.playback.queue.len();

        if self.core.settings.play_mode == PlayMode::Shuffle {
            self.playback.shuffle_cache.regenerate(queue_len);
        } else {
            self.playback.shuffle_cache.clear();
        }
    }

    /// Clear cached shuffle indices (call when queue or play mode changes)
    pub fn clear_shuffle_cache(&mut self) {
        self.playback.shuffle_cache.clear();
        let released_request_ids = self.playback.audio_preload_manager.reset();
        self.release_preload_requests(released_request_ids);
    }

    fn resolve_and_play(&mut self, idx: usize, song: DbSong) -> Task<Message> {
        // Mark this index as the one we're waiting for
        // Any other resolution results will only update song info, not trigger playback
        self.playback.pending_resolution_index = Some(idx);

        if let Some(client) = &self.core.ncm_client {
            let client = std::sync::Arc::new(client.clone());
            let song_id = song.id;

            // Create channel for streaming events
            let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(32);

            // Spawn the resolution task
            let resolve_task = Task::perform(
                async move {
                    super::song_resolver::resolve_song(client, &song, event_tx)
                        .await
                        .map(|resolved| (idx, resolved))
                },
                move |result| {
                    if let Some((idx, resolved)) = result {
                        Message::SongResolvedStreaming(
                            idx,
                            resolved.file_path,
                            resolved.cover_path,
                            resolved.shared_buffer,
                            resolved.duration_secs,
                        )
                    } else {
                        Message::SongResolveFailed
                    }
                },
            );

            // Spawn task to forward streaming events to app messages
            let event_task = Task::perform(
                async move {
                    let mut events = Vec::new();
                    while let Some(event) = event_rx.recv().await {
                        events.push((song_id, event));
                    }
                    events
                },
                |events| {
                    // Return first playable event if any
                    for (song_id, event) in events {
                        if matches!(event, crate::audio::streaming::StreamingEvent::Playable) {
                            return Message::StreamingEvent(song_id, event);
                        }
                    }
                    Message::NoOp
                },
            );

            Task::batch([resolve_task, event_task])
        } else {
            self.playback.pending_resolution_index = None;
            Self::toast_warning(self.core.locale.get(Key::NotLoggedIn).to_string())
        }
    }

    /// Handle song resolved with streaming support
    pub fn handle_song_resolved_streaming(
        &mut self,
        idx: usize,
        file_path: String,
        cover_path: Option<String>,
        shared_buffer: Option<crate::audio::SharedBuffer>,
        duration_secs: Option<u64>,
    ) -> Task<Message> {
        tracing::info!(
            "Song at index {} resolved to {} (buffer: {})",
            idx,
            file_path,
            shared_buffer.is_some()
        );

        let resolved = ResolvedSong {
            file_path,
            cover_path,
            shared_buffer,
            duration_secs,
        };
        let _ = self.apply_resolved_song_to_queue(idx, &resolved);

        // Only trigger playback if this is the song we're actually waiting for
        let should_play = self.playback.pending_resolution_index == Some(idx);
        if !should_play {
            tracing::debug!(
                "Ignoring resolved song at index {} (pending: {:?})",
                idx,
                self.playback.pending_resolution_index
            );
            return Task::none();
        }

        // Check if we should restore playback position (for app restart scenario)
        let restore_position = self
            .playback
            .saved_state
            .as_ref()
            .filter(|s| s.position_secs > 0.0 && s.queue_position == idx as i64)
            .map(|s| s.position_secs);

        // Clear pending state
        self.playback.pending_resolution_index = None;

        let Some(song) = self.playback.queue.get(idx).cloned() else {
            return Task::none();
        };

        let Ok(source) =
            Self::playback_source_from_resolved_song(&song, &resolved, restore_position)
        else {
            return self.skip_to_next_playable(idx);
        };

        match self.start_queue_song_from_source(idx, song, source) {
            Ok(task) => {
                if resolved.shared_buffer.is_some() {
                    tracing::info!("Playing from streaming buffer");
                }
                if let Some(pos) = restore_position {
                    self.restore_current_playback_position(pos);
                }
                task
            }
            Err(_) => self.skip_to_next_playable(idx),
        }
    }

    fn calculate_next_index(&self) -> Option<usize> {
        let nav = QueueNavigator::new(
            self.playback.queue.len(),
            self.playback.current_index,
            self.effective_queue_play_mode(),
            &self.playback.shuffle_cache,
        );
        nav.next_index()
    }

    fn calculate_prev_index(&self) -> Option<usize> {
        let nav = QueueNavigator::new(
            self.playback.queue.len(),
            self.playback.current_index,
            self.effective_queue_play_mode(),
            &self.playback.shuffle_cache,
        );
        nav.prev_index()
    }

    pub fn play_next_song(&mut self) -> Task<Message> {
        let next_idx = self.calculate_next_index();

        if next_idx.is_none() {
            if self.is_fm_mode() {
                tracing::info!("FM mode: no next song, fetching more songs");
                return self.fetch_more_fm_songs_and_play();
            }
            self.handle_queue_finished();
            return Task::none();
        }

        let next_idx = next_idx.unwrap();
        let fetch_task = if self.is_fm_mode() && self.should_fetch_more_fm() {
            tracing::info!(
                "FM mode: fetching more songs (current_idx={}, queue_len={})",
                self.playback.current_index.unwrap_or(0),
                self.playback.queue.len()
            );
            self.fetch_more_fm_songs()
        } else {
            Task::none()
        };

        // Try to use preloaded track from PreloadManager (zero-delay playback)
        if let Some(song) = self.playback.queue.get(next_idx).cloned() {
            if let Some(source) = self.take_preloaded_source(next_idx, PreloadDirection::Next) {
                tracing::info!("Playing preloaded next (index {}) - zero delay", next_idx);
                return match self.start_queue_song_from_source(next_idx, song, source) {
                    Ok(play_task) => Task::batch([fetch_task, play_task]),
                    Err(err) => {
                        Task::batch([fetch_task, self.handle_playback_failure(next_idx, &err)])
                    }
                };
            }
        }

        let play_task = self.play_song_at_index(next_idx);
        Task::batch([fetch_task, play_task])
    }

    pub fn play_prev_song(&mut self) -> Task<Message> {
        let Some(prev_idx) = self.calculate_prev_index() else {
            return Task::none();
        };

        // Try to use preloaded track from PreloadManager (zero-delay playback)
        if let Some(song) = self.playback.queue.get(prev_idx).cloned() {
            if let Some(source) = self.take_preloaded_source(prev_idx, PreloadDirection::Previous) {
                tracing::info!("Playing preloaded prev (index {}) - zero delay", prev_idx);
                return match self.start_queue_song_from_source(prev_idx, song, source) {
                    Ok(task) => task,
                    Err(err) => self.handle_playback_failure(prev_idx, &err),
                };
            }
        }

        self.play_song_at_index(prev_idx)
    }

    fn skip_to_next_playable(&mut self, failed_idx: usize) -> Task<Message> {
        // Use QueueNavigator's skip_to_next_playable for consistent behavior
        let next_idx = super::queue_navigator::skip_to_next_playable(
            self.playback.queue.len(),
            failed_idx,
            self.core.settings.play_mode,
            &self.playback.shuffle_cache,
        );

        let Some(next_idx) = next_idx else {
            return Task::none();
        };

        let song = &self.playback.queue[next_idx];
        if super::song_resolver::needs_resolution(song) || PathBuf::from(&song.file_path).exists() {
            return self.play_song_at_index(next_idx);
        }

        tracing::warn!("Skipping unavailable song: {}", song.title);
        Task::none()
    }

    pub fn handle_song_finished(&mut self) -> Task<Message> {
        tracing::info!(
            "handle_song_finished called, play_mode: {:?}, fm_mode: {}",
            self.core.settings.play_mode,
            self.is_fm_mode()
        );

        if let (Some(db), Some(song)) = (&self.core.db, &self.playback.current_song) {
            let db = db.clone();
            let song_id = song.id;
            let duration_secs = song.duration_secs;
            tokio::spawn(async move {
                let _ = db.record_play(song_id, duration_secs, true).await;
            });
        }

        // 清除播放完成状态，防止重复触发
        self.stop_audio_output();

        self.play_next_song()
    }

    fn handle_queue_finished(&mut self) {
        tracing::info!("Queue finished");
        if self.playback.queue.is_empty() {
            return;
        }

        let first_song = self.playback.queue[0].clone();
        self.commit_current_song_playback_state(Some(0), first_song.clone(), false);

        self.stop_audio_output();

        if let Some(db) = &self.core.db {
            let db = db.clone();
            let song_id = first_song.id;
            tokio::spawn(async move {
                let _ = db.update_playback_position(Some(song_id), 0, 0.0).await;
            });
        }

        if let Some(state) = &mut self.playback.saved_state {
            state.position_secs = 0.0;
        }
    }

    /// Warm remote lyrics cache for an NCM song without touching display state.
    pub fn warm_lyrics_cache_for_song(&mut self, song: &DbSong) -> Task<Message> {
        // Only preload for NCM songs (negative ID)
        if song.id >= 0 {
            return Task::none();
        }

        let ncm_id = (-song.id) as u64;

        if !self
            .playback
            .lyrics_preload_manager
            .should_schedule_warmup(song.id, ncm_id)
        {
            return Task::none();
        }

        self.playback
            .preload_coordinator
            .ensure_lyrics_slot(song.id);
        Task::done(Message::WarmLyricsCache(song.id, ncm_id))
    }
}
