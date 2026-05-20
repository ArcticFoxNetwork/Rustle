//! Message update handlers - thin dispatcher delegating to submodules

mod context_menu;
mod database;
mod discord;
mod discover;
mod import;
mod keyboard;
mod layout;
mod lyrics;
mod mpris;
mod navigation;
mod ncm;
mod overlay;
pub mod page_loader;
mod playback;
mod player_controller;
mod playlist;
mod preload;
pub mod preload_manager;
mod protocol;
mod queue;
pub mod queue_navigator;
mod router;
mod search;
mod settings;
pub mod song_resolver;
mod toast;
mod tray;
mod window;

use iced::Task;

use super::{App, Message};

impl App {
    /// Handle messages by delegating to appropriate submodule handlers
    pub fn update(&mut self, message: Message) -> Task<Message> {
        // Try each handler in order until one handles the message
        if let Some(task) = self.handle_navigation(&message) {
            return task;
        }
        if let Some(task) = self.handle_context_menu(&message) {
            return task;
        }
        if let Some(task) = self.handle_overlay(&message) {
            return task;
        }
        if let Some(task) = self.handle_database(&message) {
            return task;
        }
        if let Some(task) = self.handle_import(&message) {
            return task;
        }
        if let Some(task) = self.handle_toast(&message) {
            return task;
        }
        if let Some(task) = self.handle_download(&message) {
            return task;
        }
        if let Some(task) = self.handle_playback(&message) {
            return task;
        }
        if let Some(task) = self.handle_playlist(&message) {
            return task;
        }
        if let Some(task) = self.handle_queue(&message) {
            return task;
        }
        if let Some(task) = self.handle_settings(&message) {
            return task;
        }
        if let Some(task) = self.handle_window(&message) {
            return task;
        }
        if let Some(task) = self.handle_tray(&message) {
            return task;
        }
        if let Some(task) = self.handle_mpris(&message) {
            return task;
        }
        if let Some(task) = self.handle_discord(&message) {
            return task;
        }
        if let Some(task) = self.handle_keyboard(&message) {
            return task;
        }
        if let Some(task) = self.handle_layout(&message) {
            return task;
        }
        if let Some(task) = self.handle_lyrics(&message) {
            return task;
        }
        if let Some(task) = self.handle_ncm(&message) {
            return task;
        }
        if let Some(task) = self.handle_discover(&message) {
            return task;
        }
        if let Some(task) = self.handle_search(&message) {
            return task;
        }
        if let Some(task) = self.handle_preload(&message) {
            return task;
        }
        if let Some(task) = self.handle_protocol(&message) {
            return task;
        }

        // Process pending cover downloads
        if let Some(task) = self.process_pending_covers() {
            return task;
        }

        // Default: no task
        Task::none()
    }

    fn process_pending_covers(&mut self) -> Option<Task<Message>> {
        let pending = crate::utils::drain_pending_covers();
        if pending.is_empty() {
            return None;
        }
        let client = self.core.ncm_client.as_ref()?.clone();
        let tasks: Vec<Task<Message>> = pending
            .into_iter()
            .flat_map(|(id, is_playlist)| {
                let c = client.clone();
                if is_playlist {
                    Some(Task::perform(
                        async move {
                            if let Ok(detail) = c.client.song_list_detail(id).await {
                                if !detail.cover_img_url.is_empty() {
                                    crate::utils::download_playlist_cover(
                                        &c, id, &detail.cover_img_url,
                                    ).await;
                                }
                            }
                        },
                        |_| Message::Noop,
                    ))
                } else {
                    Some(Task::perform(
                        async move {
                            if let Ok(songs) = c.song_detail(&[id]).await {
                                if let Some(s) = songs.first() {
                                    if !s.pic_url.is_empty() {
                                        crate::utils::download_cover(&c, id, &s.pic_url).await;
                                    }
                                }
                            }
                        },
                        |_| Message::Noop,
                    ))
                }
            })
            .collect();
        if tasks.is_empty() { None } else { Some(Task::batch(tasks)) }
    }
}
