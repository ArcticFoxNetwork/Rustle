//! Message update handlers - thin dispatcher delegating to submodules

mod database;
mod discover;
mod import;
mod keyboard;
mod lyrics;
mod mpris;
mod navigation;
mod ncm;
pub mod page_loader;
mod playback;
mod player_controller;
mod playlist;
mod preload;
pub mod preload_manager;
mod queue;
pub mod queue_navigator;
mod settings;
pub mod song_resolver;
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
        if let Some(task) = self.handle_database(&message) {
            return task;
        }
        if let Some(task) = self.handle_import(&message) {
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
        if let Some(task) = self.handle_keyboard(&message) {
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
        if let Some(task) = self.handle_preload(&message) {
            return task;
        }

        // Default: no task
        Task::none()
    }
}
