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
mod image;
mod metadata;
mod platform;
mod protocol;
mod ui;
mod utils;

fn main() -> iced::Result {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().collect();

    // Handle rustle:// protocol URL from CLI.
    // The desktop file passes the URL directly as the first argument via %u.
    if args.len() >= 2 && (args[1].starts_with("rustle://") || args[1].starts_with("rustle-dev://"))
    {
        let uri = args[1].clone();
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
    } else {
        // Normal launch: check if another instance is already running.
        // If so, focus its window and exit — single-instance enforcement.
        match protocol::ipc::forward_focus_to_primary() {
            Ok(()) => {
                tracing::info!("Focused existing instance, exiting");
                std::process::exit(0);
            }
            Err(_) => {
                // No existing instance, proceed with normal startup
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
