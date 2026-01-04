//! Windows window behavior implementation
//!
//! Windows requires special handling: minimize before hide, restore before show

use iced::Task;

/// Show the window and bring it to front (Windows)
/// Windows needs to restore from minimized state first
pub fn show_window<Message: Send + 'static>() -> Task<Message> {
    iced::window::latest().and_then(|id| {
        Task::batch([
            iced::window::set_visible(id, true),
            iced::window::minimize(id, false),
            iced::window::gain_focus(id),
        ])
    })
}

/// Hide the window (Windows)
/// Windows needs to minimize first, then hide
pub fn hide_window<Message: Send + 'static>() -> Task<Message> {
    iced::window::latest().and_then(|id| {
        Task::batch([
            iced::window::minimize(id, true),
            iced::window::set_visible(id, false),
        ])
    })
}
