//! Windows window behavior implementation
//!
//! Windows requires special handling: minimize before hide, restore before show

use iced::Task;

pub fn native_window_handle(id: iced::window::Id) -> Task<Option<usize>> {
    use iced::window::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    iced::window::run(id, |window| match window.window_handle() {
        Ok(handle) => match handle.as_raw() {
            RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as usize),
            _ => None,
        },
        Err(_) => None,
    })
}

pub fn set_window_mode<Message: Send + 'static>(mode: iced::window::Mode) -> Task<Message> {
    iced::window::latest().and_then(move |id| match mode {
        iced::window::Mode::Hidden => Task::batch([
            iced::window::minimize(id, true),
            iced::window::set_mode(id, iced::window::Mode::Hidden),
        ]),
        iced::window::Mode::Windowed => Task::batch([
            iced::window::set_mode(id, iced::window::Mode::Windowed),
            iced::window::minimize(id, false),
            iced::window::gain_focus(id),
        ]),
        iced::window::Mode::Fullscreen => Task::batch([
            iced::window::set_mode(id, iced::window::Mode::Fullscreen),
            iced::window::minimize(id, false),
            iced::window::gain_focus(id),
        ]),
    })
}

pub fn focus_window<Message: Send + 'static>() -> Task<Message> {
    iced::window::latest().and_then(|id| {
        Task::batch([
            iced::window::minimize(id, false),
            iced::window::gain_focus(id),
        ])
    })
}
