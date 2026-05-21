//! Download manager types — queue, concurrency, progress tracking

pub mod handler;
pub mod task;

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

use crate::features::settings::MusicQuality;
use crate::metadata::SongMetadata;

/// Download status for a single task
#[derive(Debug, Clone)]
pub enum DownloadStatus {
    Pending,
    Active { progress: f32, speed: String },
    Completed(PathBuf),
    Failed(String),
}

/// A single download task — operational fields + unified metadata
#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub song_id: i64,
    pub ncm_id: u64,
    pub song_url: String,
    pub quality: MusicQuality,
    pub download_dir: PathBuf,
    pub file_size: u64,
    pub status: DownloadStatus,
    /// All descriptive metadata in one place — adding a field to SongMetadata
    /// automatically propagates through the entire download pipeline.
    pub metadata: SongMetadata,
}

/// Download queue manager
#[derive(Debug, Default)]
pub struct DownloadManager {
    pub pending: VecDeque<DownloadTask>,
    pub active: Vec<DownloadTask>,
    pub completed: Vec<DownloadTask>,
    pub failed: Vec<(DownloadTask, String)>,
    #[doc(hidden)]
    pub abort_handles: HashMap<i64, iced::task::Handle>,
}

impl DownloadManager {
    pub fn enqueue_song(
        &mut self,
        song_id: i64,
        ncm_id: u64,
        song_url: String,
        quality: MusicQuality,
        download_dir: PathBuf,
        metadata: SongMetadata,
    ) -> bool {
        // Block if already in pending or active.
        // For completed downloads, allow re-download if the file was deleted.
        let blocked = self
            .pending
            .iter()
            .map(|t| t.song_id)
            .chain(self.active.iter().map(|t| t.song_id))
            .chain(self.completed.iter().filter_map(|t| {
                match &t.status {
                    DownloadStatus::Completed(path) if path.exists() => Some(t.song_id),
                    DownloadStatus::Completed(_) => None, // file gone — allow re-download
                    _ => Some(t.song_id),
                }
            }))
            .any(|id| id == song_id);
        if blocked {
            return false;
        }
        self.pending.push_back(DownloadTask {
            song_id,
            ncm_id,
            song_url,
            quality,
            download_dir,
            file_size: 0,
            status: DownloadStatus::Pending,
            metadata,
        });
        true
    }

    pub fn enqueue_playlist(
        &mut self,
        songs: Vec<(i64, u64, String, SongMetadata)>,
        quality: MusicQuality,
        download_dir: PathBuf,
    ) -> usize {
        let mut count = 0;
        for (song_id, ncm_id, url, meta) in songs {
            if self.enqueue_song(song_id, ncm_id, url, quality, download_dir.clone(), meta) {
                count += 1;
            }
        }
        count
    }

    pub fn schedule(&mut self) -> Option<DownloadTask> {
        let active_running = self.active.len();
        if active_running >= 3 {
            return None;
        }
        self.pending.pop_front().map(|task| {
            let mut active_task = task;
            active_task.status = DownloadStatus::Active {
                progress: 0.0,
                speed: String::new(),
            };
            self.active.push(active_task.clone());
            active_task
        })
    }

    pub fn complete(&mut self, song_id: i64, path: PathBuf) {
        if let Some(pos) = self.active.iter().position(|t| t.song_id == song_id) {
            let mut task = self.active.remove(pos);
            task.status = DownloadStatus::Completed(path);
            self.completed.push(task);
        }
        self.abort_handles.remove(&song_id);
    }

    pub fn fail(&mut self, song_id: i64, error: String) {
        if let Some(pos) = self.active.iter().position(|t| t.song_id == song_id) {
            let task = self.active.remove(pos);
            self.failed.push((task, error));
        }
        self.abort_handles.remove(&song_id);
    }

    pub fn cancel(&mut self, song_id: i64) {
        if let Some(handle) = self.abort_handles.remove(&song_id) {
            handle.abort();
        }
        self.pending.retain(|t| t.song_id != song_id);
        self.active.retain(|t| t.song_id != song_id);
    }

    pub fn total_count(&self) -> usize {
        self.pending.len() + self.active.len() + self.completed.len()
    }

    pub fn restore_from_db(&mut self, songs: Vec<crate::database::DbSong>) {
        for song in songs {
            let path = std::path::PathBuf::from(&song.file_path);
            if !path.exists() {
                continue;
            }
            let ncm_id = if song.id < 0 { (-song.id) as u64 } else { 0 };
            self.completed.push(DownloadTask {
                song_id: song.id,
                ncm_id,
                song_url: String::new(),
                quality: MusicQuality::High,
                download_dir: path.parent().map(|p| p.to_path_buf()).unwrap_or_default(),
                file_size: song.file_size as u64,
                status: DownloadStatus::Completed(path),
                metadata: SongMetadata::from(&song),
            });
        }
    }
}
