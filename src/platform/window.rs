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

pub fn initialize_process() {
    #[cfg(target_os = "windows")]
    {
        windows::initialize_process();
    }
}

pub fn set_window_mode<Message: Send + 'static>(mode: iced::window::Mode) -> Task<Message> {
    #[cfg(target_os = "windows")]
    {
        windows::set_window_mode(mode)
    }
    #[cfg(target_os = "linux")]
    {
        linux::set_window_mode(mode)
    }
    #[cfg(target_os = "macos")]
    {
        macos::set_window_mode(mode)
    }
}

pub fn focus_window<Message: Send + 'static>() -> Task<Message> {
    #[cfg(target_os = "windows")]
    {
        windows::focus_window()
    }
    #[cfg(target_os = "linux")]
    {
        linux::focus_window()
    }
    #[cfg(target_os = "macos")]
    {
        macos::focus_window()
    }
}

pub fn is_wayland_backend() -> bool {
    #[cfg(target_os = "linux")]
    {
        return linux::is_wayland_backend();
    }

    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

pub fn native_window_handle(id: iced::window::Id) -> Task<Option<usize>> {
    #[cfg(target_os = "windows")]
    {
        windows::native_window_handle(id)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = id;
        Task::done(None)
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
