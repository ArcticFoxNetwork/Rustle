//! Section header widget
//!
//! Displays a section title with optional "See All" link.
//! This is a reusable widget that does not depend on application-specific types.
//!
//! # Design
//!
//! Uses generic Message type to allow reuse across different contexts.

use iced::widget::{Space, button, row, svg, text};
use iced::{Alignment, Element, Fill};

use crate::ui::icons;
use crate::ui::theme::{self, BOLD_WEIGHT};

/// Create a section header element
///
/// # Arguments
/// * `title` - The section title text
/// * `see_all_text` - Text for the "See All" button
/// * `on_see_all` - Optional message to send when "See All" is clicked
pub fn view<'a, Message: Clone + 'a>(
    title: &'a str,
    see_all_text: &'a str,
    on_see_all: Option<Message>,
) -> Element<'a, Message> {
    let title_text = text(title)
        .size(22)
        .style(|theme| text::Style {
            color: Some(theme::text_primary(theme)),
        })
        .font(iced::Font {
            weight: BOLD_WEIGHT,
            ..Default::default()
        });

    let see_all_btn: Element<'a, Message> = if let Some(msg) = on_see_all {
        button(
            row![
                text(see_all_text).size(14).color(theme::TEXT_SECONDARY),
                Space::new().width(4),
                svg(svg::Handle::from_memory(icons::CHEVRON_RIGHT.as_bytes()))
                    .width(16)
                    .height(16)
                    .style(|_theme, _status| svg::Style {
                        color: Some(theme::TEXT_SECONDARY),
                    }),
            ]
            .align_y(Alignment::Center),
        )
        .padding(0)
        .style(|_theme, status| {
            let text_color = match status {
                iced::widget::button::Status::Hovered => theme::TEXT_PRIMARY,
                _ => theme::TEXT_SECONDARY,
            };
            iced::widget::button::Style {
                background: None,
                text_color,
                ..Default::default()
            }
        })
        .on_press(msg)
        .into()
    } else {
        Space::new().width(0).into()
    };

    row![title_text, Space::new().width(Fill), see_all_btn,]
        .align_y(Alignment::Center)
        .into()
}
