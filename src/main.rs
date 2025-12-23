//! Rustle - A modern music streaming desktop application
//! Built with iced for a sleek, dark mode UI

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod app;
mod audio;
mod cache;
mod database;
mod features;
mod i18n;
mod ui;
mod utils;

fn main() -> iced::Result {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    // Run the application as a daemon (keeps running when windows are closed)
    // This allows the app to run in the background with system tray
    iced::daemon(app::App::new, app::App::update, app::App::view)
        .title(app::App::title)
        .theme(app::App::theme)
        .subscription(app::App::subscription)
        .antialiasing(true)
        .run()
}
