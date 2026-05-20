//! Rustle - A modern music streaming desktop application
//! Built with iced for a sleek, dark mode UI

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod app;
mod audio;
mod cache;
mod database;
mod download;
mod features;
mod i18n;
mod metadata;
mod platform;
mod protocol;
mod ui;
mod utils;

fn main() -> iced::Result {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    // Handle rustle:// protocol — forward to existing instance if running
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "--protocol-handler" {
        let uri = args[2].clone();
        match protocol::ipc::forward_uri_to_primary(&uri) {
            Ok(()) => {
                tracing::info!("URI forwarded to primary instance, exiting");
                std::process::exit(0);
            }
            Err(e) => {
                tracing::warn!("No primary instance found ({}), starting new one", e);
                protocol::ipc::set_pending_startup_uri(uri);
            }
        }
    }

    platform::init();

    // Run the application as a daemon (keeps running when windows are closed)
    // This allows the app to run in the background with system tray
    iced::daemon(app::App::new, app::App::update, app::App::view)
        .title(app::App::title)
        .theme(app::App::theme)
        .subscription(app::App::subscription)
        .antialiasing(true)
        .run()
}
