// src/app/update/import.rs
//! Import message handlers

use iced::Task;
use std::sync::Arc;

use crate::app::helpers::{create_playlist_from_import, load_playlists, load_songs};
use crate::app::message::Message;
use crate::app::state::App;
use crate::features::import::{
    ScanConfig, ScanHandle, ScanProgress, ScanState, progress_channel, scan_and_import,
};
use crate::ui::components::ImportingPlaylist;
use crate::ui::widgets::Toast;

impl App {
    /// Handle import-related messages
    pub fn handle_import(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::FolderSelected(path) => {
                self.ui.dialogs.import_open = false;
                if let Some(path) = path {
                    return Some(Task::done(Message::StartScan(path.clone())));
                }
                Some(Task::none())
            }

            Message::StartScan(path) => Some(self.start_scan(path.clone())),

            Message::ScanProgressUpdate(progress) => {
                Some(self.process_scan_progress(progress.clone()))
            }

            Message::AddWatchedFolder(path) => {
                if !self.library.watched_folders.contains(path) {
                    self.library.watched_folders.push(path.clone());
                    if let Some(watcher) = &mut self.library.folder_watcher {
                        if let Err(e) = watcher.watch(path) {
                            tracing::error!("Failed to watch folder: {}", e);
                        }
                    }
                }
                Some(Task::none())
            }

            Message::RemoveWatchedFolder(path) => {
                self.library.watched_folders.retain(|p| p != path);
                if let Some(watcher) = &mut self.library.folder_watcher {
                    if let Err(e) = watcher.unwatch(path) {
                        tracing::error!("Failed to unwatch folder: {}", e);
                    }
                }
                Some(Task::none())
            }

            Message::WatcherEvent(event) => {
                use crate::features::import::WatchEvent;
                match event {
                    WatchEvent::FileCreated(path) => {
                        tracing::info!("New file detected: {:?}", path);
                        // Could auto-import here if desired
                    }
                    WatchEvent::FileDeleted(path) => {
                        tracing::info!("File deleted: {:?}", path);
                        // Update database to mark song as unavailable or remove it
                        if let Some(db) = &self.core.db {
                            let db = db.clone();
                            let path_str = path.to_string_lossy().to_string();
                            tokio::spawn(async move {
                                if let Err(e) = db.delete_song_by_path(&path_str).await {
                                    tracing::error!("Failed to remove deleted song from db: {}", e);
                                }
                            });
                        }
                    }
                    WatchEvent::FileModified(path) => {
                        tracing::debug!("File modified: {:?}", path);
                        // Could re-scan metadata here if desired
                    }
                    WatchEvent::FileRenamed(old, new) => {
                        tracing::info!("File renamed: {:?} -> {:?}", old, new);
                        // Update file path in database
                        if let Some(db) = &self.core.db {
                            let db = db.clone();
                            let old_path = old.to_string_lossy().to_string();
                            let new_path = new.to_string_lossy().to_string();
                            tokio::spawn(async move {
                                if let Err(e) = db.update_song_path(&old_path, &new_path).await {
                                    tracing::error!("Failed to update renamed song path: {}", e);
                                }
                            });
                        }

                        // Also update in-memory state if the renamed file is currently playing
                        if let Some(song) = &mut self.library.current_song {
                            if song.file_path == old.to_string_lossy() {
                                song.file_path = new.to_string_lossy().to_string();
                                tracing::info!("Updated current song path to: {}", song.file_path);
                            }
                        }

                        // Update songs in db_songs cache
                        for song in &mut self.library.db_songs {
                            if song.file_path == old.to_string_lossy() {
                                song.file_path = new.to_string_lossy().to_string();
                            }
                        }

                        // Update songs in queue
                        for song in &mut self.library.queue {
                            if song.file_path == old.to_string_lossy() {
                                song.file_path = new.to_string_lossy().to_string();
                            }
                        }
                    }
                    WatchEvent::Error(e) => tracing::error!("Watcher error: {}", e),
                }
                Some(Task::none())
            }

            Message::CoverCacheReady(cache) => {
                tracing::info!("Cover cache initialized");
                self.core.cover_cache = Some(cache.clone());
                Some(Task::none())
            }

            Message::HideToast => {
                self.ui.toast_visible = false;
                if let Some(playlist) = &self.ui.importing_playlist {
                    if playlist.completed {
                        return Some(Task::perform(
                            async {
                                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            },
                            |_| Message::ClearImportingPlaylist,
                        ));
                    }
                }
                Some(Task::none())
            }

            Message::ClearImportingPlaylist => {
                self.ui.importing_playlist = None;
                Some(Task::none())
            }

            _ => None,
        }
    }

    /// Start a folder scan for importing music
    fn start_scan(&mut self, path: std::path::PathBuf) -> Task<Message> {
        if let (Some(db), Some(cache)) = (&self.core.db, &self.core.cover_cache) {
            let folder_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("导入的歌单")
                .to_string();
            self.ui.importing_playlist = Some(ImportingPlaylist::new(folder_name));

            let db = db.clone();
            let cache = cache.clone();
            let state = Arc::new(ScanState::new());
            let handle = ScanHandle::new(state.clone());

            self.library.scan_state = Some(state.clone());
            self.library.scan_handle = Some(handle);

            let (tx, mut rx) = progress_channel();
            let config = ScanConfig::default();
            let path_clone = path.clone();

            tokio::spawn(async move {
                if let Err(e) = scan_and_import(db, path_clone, config, cache, state, tx).await {
                    tracing::error!("Scan error: {}", e);
                }
            });

            return Task::run(
                async_stream::stream! {
                    while let Some(progress) = rx.recv().await {
                        yield progress;
                    }
                },
                Message::ScanProgressUpdate,
            );
        }
        Task::none()
    }

    /// Process scan progress updates
    fn process_scan_progress(&mut self, progress: ScanProgress) -> Task<Message> {
        match &progress {
            ScanProgress::Started { total_files } => {
                tracing::info!("Scan started: {} files", total_files);
                if let Some(playlist) = &mut self.ui.importing_playlist {
                    playlist.total = *total_files;
                }
            }
            ScanProgress::Imported {
                current,
                total,
                title,
                artist,
                cover_path,
            } => {
                tracing::debug!("Imported ({}/{}): {} - {}", current, total, artist, title);
                if let Some(playlist) = &mut self.ui.importing_playlist {
                    playlist.update_progress(*current, *total);
                    if let Some(cover) = cover_path {
                        playlist.set_cover(cover.clone());
                    }
                }
            }
            ScanProgress::Skipped { current, total, .. } => {
                if let Some(playlist) = &mut self.ui.importing_playlist {
                    playlist.update_progress(*current, *total);
                }
            }
            ScanProgress::Completed {
                imported,
                skipped,
                errors,
                duration_secs,
            } => {
                tracing::info!(
                    "Scan completed: {} imported, {} skipped, {} errors in {:.2}s",
                    imported,
                    skipped,
                    errors,
                    duration_secs
                );

                // Extract scanned paths before clearing state
                let scanned_paths = self
                    .library
                    .scan_state
                    .as_ref()
                    .and_then(|state| state.get_scanned_paths())
                    .unwrap_or_default();

                self.library.scan_state = None;
                self.library.scan_handle = None;

                let is_success = *imported > 0 || *skipped > 0;

                let total_processed = *imported + *skipped + *errors;
                let toast = if total_processed == 0 {
                    self.ui.importing_playlist = None;
                    Toast::error("导入失败：未找到任何音频文件")
                } else if *errors == 0 {
                    Toast::success(format!("导入完成！成功导入 {} 首歌曲", imported))
                } else {
                    Toast::warning(format!("导入完成：{} 首成功，{} 首失败", imported, errors))
                };
                self.ui.toast = Some(toast);
                self.ui.toast_visible = true;

                if is_success {
                    if let (Some(db), Some(playlist)) = (&self.core.db, &self.ui.importing_playlist)
                    {
                        let db = db.clone();
                        let name = playlist.name.clone();
                        let cover_path = playlist.cover_path.clone();

                        if let Some(p) = &mut self.ui.importing_playlist {
                            p.complete();
                        }

                        let db_for_reload = db.clone();
                        return Task::batch([
                            Task::perform(
                                async move {
                                    let result = create_playlist_from_import(
                                        db.clone(),
                                        name,
                                        cover_path,
                                        scanned_paths,
                                    )
                                    .await;
                                    let playlists = load_playlists(db).await;
                                    (result, playlists)
                                },
                                |(result, playlists)| {
                                    if let Err(e) = result {
                                        tracing::error!("Failed to create playlist: {}", e);
                                    }
                                    Message::PlaylistsLoaded(playlists)
                                },
                            ),
                            Task::perform(load_songs(db_for_reload), Message::SongsLoaded),
                            Task::perform(
                                async {
                                    tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                                },
                                |_| Message::HideToast,
                            ),
                        ]);
                    }
                } else {
                    return Task::perform(
                        async {
                            tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                        },
                        |_| Message::HideToast,
                    );
                }
            }
            ScanProgress::Cancelled => {
                tracing::info!("Scan cancelled");
                self.library.scan_state = None;
                self.library.scan_handle = None;
                self.ui.importing_playlist = None;

                self.ui.toast = Some(Toast::error("导入已取消".to_string()));
                self.ui.toast_visible = true;
                return Task::perform(
                    async {
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    },
                    |_| Message::HideToast,
                );
            }
            ScanProgress::Error(e) => {
                tracing::error!("Scan error: {}", e);
                self.ui.toast = Some(Toast::error(format!("导入失败：{}", e)));
                self.ui.toast_visible = true;
            }
            _ => {}
        }
        self.library.scan_progress = Some(progress);
        Task::none()
    }
}
