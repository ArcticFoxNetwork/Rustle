// src/app/update/player_controller.rs
//! Unified player controller for all playback operations
//!
//! Uses QueueNavigator as Single Source of Truth for index calculations.

use std::path::PathBuf;

use iced::Task;

use crate::app::helpers::update_tray_state_full;
use crate::app::message::Message;
use crate::app::state::App;
use crate::database::DbSong;
use crate::features::PlayMode;

use super::queue_navigator::QueueNavigator;

/// Result of attempting to play a song
pub enum PlayResult {
    Playing,
    NeedsResolution,
    Failed(String),
}

impl App {
    /// Central method to play a song at a specific queue index
    pub fn play_song_at_index(&mut self, idx: usize) -> Task<Message> {
        if idx >= self.library.queue.len() {
            tracing::warn!("Invalid queue index: {}", idx);
            return Task::none();
        }

        self.library.queue_index = Some(idx);
        let song = self.library.queue[idx].clone();

        if super::song_resolver::needs_resolution(&song) {
            tracing::info!("Song {} needs resolution", song.title);
            return self.resolve_and_play(idx, song);
        }

        match self.try_play_song(&song) {
            PlayResult::Playing => self.on_song_started(idx, song),
            PlayResult::NeedsResolution => self.resolve_and_play(idx, song),
            PlayResult::Failed(err) => {
                tracing::error!("Failed to play {}: {}", song.title, err);
                self.skip_to_next_playable(idx)
            }
        }
    }

    fn try_play_song(&mut self, song: &DbSong) -> PlayResult {
        let path = PathBuf::from(&song.file_path);
        if song.file_path.is_empty() || !path.exists() {
            if song.file_path.starts_with("ncm://") || song.id < 0 {
                return PlayResult::NeedsResolution;
            }
            return PlayResult::Failed(format!("File not found: {}", song.file_path));
        }

        if let Some(player) = &mut self.core.audio {
            match player.play(path) {
                Ok(_) => PlayResult::Playing,
                Err(e) => PlayResult::Failed(e),
            }
        } else {
            PlayResult::Failed("No audio player".to_string())
        }
    }

    fn on_song_started(&mut self, idx: usize, song: DbSong) -> Task<Message> {
        tracing::info!("Playing: {} - {}", song.title, song.artist);

        // Ensure cover path is local (not remote URL)
        let (song, needs_cover_download) = self.ensure_local_cover_path_with_download(idx, song);

        self.library.current_song = Some(song.clone());

        if let Some(db) = &self.core.db {
            let db = db.clone();
            let song_id = song.id;
            tokio::spawn(async move {
                let _ = db.record_play(song_id, 0, false).await;
            });
        }

        update_tray_state_full(
            true,
            Some(song.title.clone()),
            Some(song.artist.clone()),
            self.core.settings.play_mode,
        );

        // Update tray with favorite status for NCM songs
        if song.id < 0 {
            let ncm_id = (-song.id) as u64;
            let is_favorited = self
                .core
                .user_info
                .as_ref()
                .map(|u| u.like_songs.contains(&ncm_id))
                .unwrap_or(false);
            crate::app::helpers::update_tray_state_with_favorite(
                true,
                Some(song.title.clone()),
                Some(song.artist.clone()),
                self.core.settings.play_mode,
                Some(ncm_id),
                is_favorited,
            );
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

        // Pre-calculate shuffle indices for consistent preloading
        self.cache_shuffle_indices();

        self.update_mpris_state();

        // ============ 统一的歌曲切换副作用 ============
        // 无论歌词页面是否打开，都执行相同的逻辑

        // 1. 预加载相邻曲目（音频）
        let preload_task = self.preload_adjacent_tracks_with_ncm();

        // 2. 下载封面（如果需要）
        let cover_task = if let Some((ncm_id, cover_url)) = needs_cover_download {
            self.download_current_song_cover(song.id, ncm_id, cover_url)
        } else {
            Task::none()
        };

        // 3. 歌词页面相关更新
        let lyrics_task = if self.ui.lyrics.is_open {
            // 歌词页面已打开：加载歌词 + 更新背景
            self.load_lyrics_for_current_song(&song)
        } else {
            // 歌词页面未打开：只预加载歌词（后台）
            self.preload_lyrics_for_song(&song)
        };

        Task::batch([preload_task, cover_task, lyrics_task])
    }

    /// 为当前歌曲加载歌词和背景（歌词页面打开时调用）
    fn load_lyrics_for_current_song(&mut self, song: &DbSong) -> Task<Message> {
        // 使用统一的异步加载方法
        self.load_lyrics_async(song)
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
        for ext in &["jpg", "png", "gif", "webp", "bmp"] {
            let local_path = cover_cache_dir.join(format!("{}.{}", stem, ext));
            if local_path.exists() {
                let local_path_str = local_path.to_string_lossy().to_string();
                tracing::info!("Found local cover at: {}", local_path_str);

                // Update song's cover_path if it's different (was URL or different path)
                if song.cover_path.as_ref() != Some(&local_path_str) {
                    song.cover_path = Some(local_path_str.clone());

                    // Also update in queue
                    if let Some(queue_song) = self.library.queue.get_mut(idx) {
                        queue_song.cover_path = Some(local_path_str);
                    }
                }
                return (song, None);
            }
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
        let queue_len = self.library.queue.len();

        if self.core.settings.play_mode == PlayMode::Shuffle {
            self.library.shuffle_cache.regenerate(queue_len);
        } else {
            self.library.shuffle_cache.clear();
        }
    }

    /// Clear cached shuffle indices (call when queue or play mode changes)
    pub fn clear_shuffle_cache(&mut self) {
        self.library.shuffle_cache.clear();
        self.library.preload_manager.reset();
    }

    fn resolve_and_play(&mut self, idx: usize, song: DbSong) -> Task<Message> {
        // Mark this index as the one we're waiting for
        // Any other resolution results will only update song info, not trigger playback
        self.library.pending_resolution_idx = Some(idx);

        if let Some(client) = &self.core.ncm_client {
            let client = std::sync::Arc::new(client.clone());
            Task::perform(
                async move {
                    super::song_resolver::resolve_song(client, &song)
                        .await
                        .map(|resolved| (idx, resolved))
                },
                |result| {
                    if let Some((idx, resolved)) = result {
                        Message::SongResolved(idx, resolved.file_path, resolved.cover_path)
                    } else {
                        Message::SongResolveFailed
                    }
                },
            )
        } else {
            self.library.pending_resolution_idx = None;
            Task::done(Message::ShowToast("请先登录".to_string()))
        }
    }

    pub fn handle_song_resolved(
        &mut self,
        idx: usize,
        file_path: String,
        cover_path: Option<String>,
    ) -> Task<Message> {
        tracing::info!("Song at index {} resolved to {}", idx, file_path);

        // Always update the song info in queue (for caching purposes)
        if let Some(song) = self.library.queue.get_mut(idx) {
            song.file_path = file_path.clone();
            if cover_path.is_some() {
                song.cover_path = cover_path.clone();
            }

            if let Some(db) = &self.core.db {
                let db = db.clone();
                let song_clone = song.clone();
                tokio::spawn(async move {
                    let _ = db.upsert_ncm_song(&song_clone).await;
                });
            }
        }

        // Only trigger playback if this is the song we're actually waiting for
        let should_play = self.library.pending_resolution_idx == Some(idx);
        if !should_play {
            tracing::debug!(
                "Ignoring resolved song at index {} (pending: {:?})",
                idx,
                self.library.pending_resolution_idx
            );
            return Task::none();
        }

        // Check if we should restore playback position (for app restart scenario)
        let restore_position = self
            .library
            .playback_state
            .as_ref()
            .filter(|s| s.position_secs > 0.0 && s.queue_position == idx as i64)
            .map(|s| s.position_secs);

        // Clear pending state
        self.library.pending_resolution_idx = None;

        if let Some(song) = self.library.queue.get(idx).cloned() {
            match self.try_play_song(&song) {
                PlayResult::Playing => {
                    // Restore position if available (app restart scenario)
                    if let Some(pos) = restore_position {
                        if let Some(player) = &mut self.core.audio {
                            let seek_pos = std::time::Duration::from_secs_f64(pos);
                            let _ = player.seek(seek_pos);
                            tracing::info!("Restored playback position to {:?}", seek_pos);
                        }
                        // Clear the saved position after restoring
                        if let Some(state) = &mut self.library.playback_state {
                            state.position_secs = 0.0;
                        }
                    }
                    self.on_song_started(idx, song)
                }
                _ => self.skip_to_next_playable(idx),
            }
        } else {
            Task::none()
        }
    }

    pub fn calculate_next_index(&self) -> Option<usize> {
        let nav = QueueNavigator::new(
            self.library.queue.len(),
            self.library.queue_index,
            self.core.settings.play_mode,
            &self.library.shuffle_cache,
        );
        nav.next_index()
    }

    pub fn calculate_prev_index(&self) -> Option<usize> {
        let nav = QueueNavigator::new(
            self.library.queue.len(),
            self.library.queue_index,
            self.core.settings.play_mode,
            &self.library.shuffle_cache,
        );
        nav.prev_index()
    }

    pub fn play_next_song(&mut self) -> Task<Message> {
        let Some(next_idx) = self.calculate_next_index() else {
            return Task::none();
        };

        if self.can_use_preloaded_next(next_idx) {
            if let Some(player) = &mut self.core.audio {
                if player.switch_to_next().is_some() {
                    tracing::info!("Switched to preloaded next (index {})", next_idx);
                    self.library.queue_index = Some(next_idx);
                    if let Some(song) = self.library.queue.get(next_idx).cloned() {
                        return self.on_song_started(next_idx, song);
                    }
                    return Task::none();
                }
            }
        }

        self.play_song_at_index(next_idx)
    }

    pub fn play_prev_song(&mut self) -> Task<Message> {
        let Some(prev_idx) = self.calculate_prev_index() else {
            return Task::none();
        };

        if self.can_use_preloaded_prev(prev_idx) {
            if let Some(player) = &mut self.core.audio {
                if player.switch_to_prev().is_some() {
                    tracing::info!("Switched to preloaded prev (index {})", prev_idx);
                    self.library.queue_index = Some(prev_idx);
                    if let Some(song) = self.library.queue.get(prev_idx).cloned() {
                        return self.on_song_started(prev_idx, song);
                    }
                    return Task::none();
                }
            }
        }

        self.play_song_at_index(prev_idx)
    }

    pub fn skip_to_next_playable(&mut self, failed_idx: usize) -> Task<Message> {
        let queue_len = self.library.queue.len();
        if queue_len == 0 {
            return Task::none();
        }

        let next_idx = match self.core.settings.play_mode {
            PlayMode::Shuffle => {
                use rand::Rng;
                rand::rng().random_range(0..queue_len)
            }
            PlayMode::LoopOne | PlayMode::LoopAll => (failed_idx + 1) % queue_len,
            PlayMode::Sequential => {
                let candidate = failed_idx + 1;
                if candidate >= queue_len {
                    return Task::none();
                }
                candidate
            }
        };

        let song = &self.library.queue[next_idx];
        if super::song_resolver::needs_resolution(song) || PathBuf::from(&song.file_path).exists() {
            return self.play_song_at_index(next_idx);
        }

        tracing::warn!("Skipping unavailable song: {}", song.title);
        Task::none()
    }

    pub fn handle_song_finished(&mut self) -> Task<Message> {
        tracing::info!(
            "handle_song_finished called, play_mode: {:?}",
            self.core.settings.play_mode
        );

        if let (Some(db), Some(song)) = (&self.core.db, &self.library.current_song) {
            let db = db.clone();
            let song_id = song.id;
            let duration_secs = song.duration_secs;
            tokio::spawn(async move {
                let _ = db.record_play(song_id, duration_secs, true).await;
            });
        }

        // 清除播放完成状态，防止重复触发
        if let Some(player) = &mut self.core.audio {
            player.stop();
        }

        match self.core.settings.play_mode {
            PlayMode::LoopOne => {
                if let Some(idx) = self.library.queue_index {
                    tracing::info!("LoopOne: replaying song at index {}", idx);
                    return self.play_song_at_index(idx);
                }
            }
            PlayMode::LoopAll | PlayMode::Shuffle => {
                if let Some(next_idx) = self.calculate_next_index() {
                    tracing::info!("LoopAll/Shuffle: playing next song at index {}", next_idx);
                    return self.play_song_at_index(next_idx);
                } else {
                    // This shouldn't happen for LoopAll/Shuffle, but handle it gracefully
                    tracing::warn!("No next index calculated for LoopAll/Shuffle mode");
                    if !self.library.queue.is_empty() {
                        return self.play_song_at_index(0);
                    }
                }
            }
            PlayMode::Sequential => {
                if let Some(next_idx) = self.calculate_next_index() {
                    tracing::info!("Sequential: playing next song at index {}", next_idx);
                    return self.play_song_at_index(next_idx);
                } else {
                    tracing::info!("Sequential: queue finished");
                    self.handle_queue_finished();
                }
            }
        }

        Task::none()
    }

    fn handle_queue_finished(&mut self) {
        tracing::info!("Queue finished");
        if self.library.queue.is_empty() {
            return;
        }

        self.library.queue_index = Some(0);
        let first_song = self.library.queue[0].clone();
        self.library.current_song = Some(first_song.clone());

        if let Some(player) = &mut self.core.audio {
            player.stop();
        }

        if let Some(db) = &self.core.db {
            let db = db.clone();
            let song_id = first_song.id;
            tokio::spawn(async move {
                let _ = db.update_playback_position(Some(song_id), 0, 0.0).await;
            });
        }

        if let Some(state) = &mut self.library.playback_state {
            state.position_secs = 0.0;
        }

        update_tray_state_full(
            false,
            Some(first_song.title.clone()),
            Some(first_song.artist.clone()),
            self.core.settings.play_mode,
        );
    }

    /// Preload lyrics for a song (triggers online fetch for NCM songs)
    pub fn preload_lyrics_for_song(&mut self, song: &DbSong) -> Task<Message> {
        // Only preload for NCM songs (negative ID)
        if song.id >= 0 {
            return Task::none();
        }

        let ncm_id = (-song.id) as u64;

        // Check if already cached
        if crate::features::lyrics::is_lyrics_cached(ncm_id) {
            return Task::none();
        }

        // Trigger preload
        Task::done(Message::PreloadLyrics(
            song.id,
            ncm_id,
            song.title.clone(),
            song.artist.clone(),
            String::new(), // album not always available
        ))
    }
}
