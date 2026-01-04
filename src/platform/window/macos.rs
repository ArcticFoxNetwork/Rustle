//! macOS window behavior implementation

use iced::Task;

/// Show the window and bring it to front (macOS)
pub fn show_window<Message: Send + 'static>() -> Task<Message> {
    iced::window::latest().and_then(|id| {
        Task::batch([
            iced::window::set_visible(id, true),
            iced::window::gain_focus(id),
        ])
    })
}

/// Hide the window (macOS)
pub fn hide_window<Message: Send + 'static>() -> Task<Message> {
    iced::window::latest().and_then(|id| iced::window::set_visible(id, false))
}
