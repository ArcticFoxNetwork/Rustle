//! Preload state machine - manages track preloading with proper state tracking
//!
//! This module provides:
//! - State tracking for preload operations (prevents duplicate requests)
//! - Retry logic for failed downloads

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;

use crate::api::NcmClient;
use crate::app::message::Message;
use crate::database::DbSong;

/// Maximum retry attempts for failed downloads
const MAX_RETRIES: u8 = 2;

/// Preload state for a single track
#[derive(Debug, Clone, Default)]
pub enum PreloadState {
    /// No preload in progress
    #[default]
    Idle,
    /// Preload request pending (waiting for download to complete)
    Pending { idx: usize },
    /// Audio ready, preloaded into player
    Ready { idx: usize, path: PathBuf },
    /// Download failed
    Failed { idx: usize, retry_count: u8 },
}

impl PreloadState {
    /// Get the index this state refers to, if any
    pub fn index(&self) -> Option<usize> {
        match self {
            Self::Idle => None,
            Self::Pending { idx } => Some(*idx),
            Self::Ready { idx, .. } => Some(*idx),
            Self::Failed { idx, .. } => Some(*idx),
        }
    }

    /// Check if this state is for a specific index
    pub fn is_for_index(&self, target_idx: usize) -> bool {
        self.index() == Some(target_idx)
    }
}

/// Manages preloading for next and previous tracks
#[derive(Debug, Default)]
pub struct PreloadManager {
    /// State for next track preload
    pub next_track: PreloadState,
    /// State for previous track preload
    pub prev_track: PreloadState,
}

impl PreloadManager {
    /// Reset all preload states (call when queue changes significantly)
    pub fn reset(&mut self) {
        self.next_track = PreloadState::Idle;
        self.prev_track = PreloadState::Idle;
    }

    /// Check if we should start preloading for an index
    pub fn should_preload_next(&self, idx: usize) -> bool {
        match &self.next_track {
            PreloadState::Idle => true,
            PreloadState::Failed { retry_count, .. } if *retry_count < MAX_RETRIES => true,
            state => !state.is_for_index(idx),
        }
    }

    /// Check if we should start preloading prev track
    pub fn should_preload_prev(&self, idx: usize) -> bool {
        match &self.prev_track {
            PreloadState::Idle => true,
            PreloadState::Failed { retry_count, .. } if *retry_count < MAX_RETRIES => true,
            state => !state.is_for_index(idx),
        }
    }

    /// Mark next track as pending download
    pub fn mark_next_pending(&mut self, idx: usize) {
        self.next_track = PreloadState::Pending { idx };
    }

    /// Mark prev track as pending download
    pub fn mark_prev_pending(&mut self, idx: usize) {
        self.prev_track = PreloadState::Pending { idx };
    }

    /// Mark next track as ready
    pub fn mark_next_ready(&mut self, idx: usize, path: PathBuf) {
        self.next_track = PreloadState::Ready { idx, path };
    }

    /// Mark prev track as ready
    pub fn mark_prev_ready(&mut self, idx: usize, path: PathBuf) {
        self.prev_track = PreloadState::Ready { idx, path };
    }

    /// Mark next track as failed
    pub fn mark_next_failed(&mut self, idx: usize) {
        let retry_count = match &self.next_track {
            PreloadState::Failed { retry_count, .. } => retry_count + 1,
            _ => 0,
        };
        self.next_track = PreloadState::Failed { idx, retry_count };
    }

    /// Mark prev track as failed
    pub fn mark_prev_failed(&mut self, idx: usize) {
        let retry_count = match &self.prev_track {
            PreloadState::Failed { retry_count, .. } => retry_count + 1,
            _ => 0,
        };
        self.prev_track = PreloadState::Failed { idx, retry_count };
    }

    /// Check if next track is ready for the given index
    pub fn is_next_ready_for(&self, idx: usize) -> Option<&PathBuf> {
        match &self.next_track {
            PreloadState::Ready {
                idx: ready_idx,
                path,
            } if *ready_idx == idx => Some(path),
            _ => None,
        }
    }

    /// Check if prev track is ready for the given index
    pub fn is_prev_ready_for(&self, idx: usize) -> Option<&PathBuf> {
        match &self.prev_track {
            PreloadState::Ready {
                idx: ready_idx,
                path,
            } if *ready_idx == idx => Some(path),
            _ => None,
        }
    }

    /// Invalidate preload if index no longer matches expected
    pub fn invalidate_if_stale(
        &mut self,
        expected_next: Option<usize>,
        expected_prev: Option<usize>,
    ) {
        if let Some(next_idx) = expected_next {
            if !self.next_track.is_for_index(next_idx) {
                self.next_track = PreloadState::Idle;
            }
        } else {
            self.next_track = PreloadState::Idle;
        }

        if let Some(prev_idx) = expected_prev {
            if !self.prev_track.is_for_index(prev_idx) {
                self.prev_track = PreloadState::Idle;
            }
        } else {
            self.prev_track = PreloadState::Idle;
        }
    }
}

/// Async audio download
///
/// The music quality is determined by the client's internal setting.
pub async fn download_audio_only(client: Arc<NcmClient>, song: &DbSong) -> Result<PathBuf, String> {
    let ncm_id = super::song_resolver::get_ncm_id(song);

    let song_cache_dir = crate::utils::songs_cache_dir();
    std::fs::create_dir_all(&song_cache_dir)
        .map_err(|e| format!("Failed to create cache dir: {}", e))?;

    let song_file_path = song_cache_dir.join(format!("{}.mp3", ncm_id));

    // Check if already cached
    if song_file_path.exists() {
        tracing::debug!("Song {} found in cache", ncm_id);
        return Ok(song_file_path);
    }

    // Download song
    tracing::info!("Downloading audio for song {}", ncm_id);

    let urls = client
        .songs_url(&[ncm_id])
        .await
        .map_err(|e| format!("Failed to get song URL: {}", e))?;

    let song_url = urls.first().ok_or_else(|| "No URL returned".to_string())?;

    if song_url.url.is_empty() {
        return Err("Empty URL returned".to_string());
    }

    client
        .client
        .download_file(&song_url.url, song_file_path.clone())
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    Ok(song_file_path)
}

/// Background cover download (non-blocking)
pub fn spawn_cover_download(client: Arc<NcmClient>, song: DbSong) {
    tokio::spawn(async move {
        let ncm_id = super::song_resolver::get_ncm_id(&song);
        let cover_cache_dir = crate::utils::covers_cache_dir();
        let _ = std::fs::create_dir_all(&cover_cache_dir);

        // Check if already cached
        let stem = format!("cover_{}", ncm_id);
        for ext in &["jpg", "png", "gif", "webp", "bmp"] {
            let existing = cover_cache_dir.join(format!("{}.{}", stem, ext));
            if existing.exists() {
                return;
            }
        }

        // Download cover in background
        if let Some(url) = &song.cover_path {
            if url.starts_with("http") {
                let _ = crate::utils::download_cover(&client, ncm_id, url).await;
            }
        }
    });
}

/// Create preload task for NCM song
///
/// The music quality is determined by the client's internal setting.
pub fn create_preload_task(
    client: Arc<NcmClient>,
    idx: usize,
    song: DbSong,
    is_next: bool,
) -> Task<Message> {
    // Spawn background cover download (non-blocking)
    spawn_cover_download(client.clone(), song.clone());

    // Download audio and return result
    Task::perform(
        async move {
            match download_audio_only(client, &song).await {
                Ok(path) => Some((idx, path.to_string_lossy().to_string(), is_next)),
                Err(e) => {
                    tracing::error!("Preload failed for idx {}: {}", idx, e);
                    None
                }
            }
        },
        move |result| {
            if let Some((idx, path, is_next)) = result {
                Message::PreloadAudioReady(idx, path, is_next)
            } else {
                Message::PreloadAudioFailed(idx, is_next)
            }
        },
    )
}
