//! Sidebar resize handle component
//! An invisible draggable area at the right edge of sidebar

use iced::widget::{container, mouse_area};
use iced::{Color, Element, Fill, Length};

use crate::app::Message;
use crate::ui::theme;

/// Width of the resize handle hit area in pixels
const HANDLE_WIDTH: f32 = 6.0;

/// Build the sidebar resize handle - uses sidebar background color
pub fn view(is_dragging: bool) -> Element<'static, Message> {
    let handle = container(iced::widget::Space::new().width(HANDLE_WIDTH).height(Fill))
        .width(Length::Fixed(HANDLE_WIDTH))
        .height(Fill)
        .style(move |t| {
            // Use sidebar background, with subtle highlight when dragging
            let bg = if is_dragging {
                // Slightly lighter when dragging
                Color::from_rgba(1.0, 1.0, 1.0, 0.05)
            } else {
                theme::sidebar_bg(t)
            };
            iced::widget::container::Style {
                background: Some(iced::Background::Color(bg)),
                ..Default::default()
            }
        });

    mouse_area(handle)
        .on_press(Message::SidebarResizeStart)
        .on_release(Message::SidebarResizeEnd)
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .into()
}
