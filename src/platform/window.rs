//! Window behavior abstraction
//!
//! Provides unified window behavior functions across platforms.
//! Handles platform-specific differences in show/hide/minimize behavior.

use iced::Task;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// Show the window and bring it to front
pub fn show_window<Message: Send + 'static>() -> Task<Message> {
    #[cfg(target_os = "windows")]
    {
        windows::show_window()
    }
    #[cfg(target_os = "linux")]
    {
        linux::show_window()
    }
    #[cfg(target_os = "macos")]
    {
        macos::show_window()
    }
}

/// Hide the window
pub fn hide_window<Message: Send + 'static>() -> Task<Message> {
    #[cfg(target_os = "windows")]
    {
        windows::hide_window()
    }
    #[cfg(target_os = "linux")]
    {
        linux::hide_window()
    }
    #[cfg(target_os = "macos")]
    {
        macos::hide_window()
    }
}

/// Get platform-specific window settings
pub fn window_settings() -> iced::window::Settings {
    iced::window::Settings {
        size: iced::Size::new(1400.0, 900.0),
        exit_on_close_request: false,
        decorations: false,
        #[cfg(target_os = "linux")]
        platform_specific: iced::window::settings::PlatformSpecific {
            application_id: "rustle".to_string(),
            ..Default::default()
        },
        #[cfg(target_os = "macos")]
        platform_specific: iced::window::settings::PlatformSpecific {
            title_hidden: true,
            titlebar_transparent: true,
            fullsize_content_view: true,
        },
        #[cfg(target_os = "windows")]
        platform_specific: Default::default(),
        ..Default::default()
    }
}
