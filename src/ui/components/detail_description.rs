//! Shared sizing and interaction helpers for detail-page descriptions.

use iced::widget::{button, text};
use iced::{Background, Color, Element, Length, Padding};

use crate::app::Message;
use crate::ui::theme;

const COLLAPSED_HEIGHT: f32 = theme::TEXT_SIZE_BODY_LARGE * 1.5 * 2.0 + 4.0;

/// Reserve exactly two text lines for descriptions already classified as long.
///
/// A fluid height would propagate through the detail header and allow the
/// surrounding page layout to squash fixed-size artwork.
pub fn collapsed_height() -> Length {
    Length::Fixed(COLLAPSED_HEIGHT)
}

/// Keep short descriptions intrinsic and let the parent constrain them.
pub fn text_width() -> Length {
    Length::Shrink
}

/// Render an inline text action with a text-width hit target.
///
/// Iced dispatches pointer events to rectangular widget bounds, not individual
/// glyph outlines. Horizontal padding stays at zero so the hit box follows the
/// label width; vertical padding makes the action easier to click.
pub fn toggle_button(label: &'static str, on_press: Message) -> Element<'static, Message> {
    button(text(label).size(theme::TEXT_SIZE_CAPTION))
        .padding(Padding::new(8.0).left(0.0).right(0.0))
        .style(|_theme, status| button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: match status {
                button::Status::Hovered | button::Status::Pressed => theme::ACCENT_PINK_HOVER,
                _ => theme::ACCENT_PINK,
            },
            ..Default::default()
        })
        .on_press(on_press)
        .into()
}

#[cfg(test)]
mod tests {
    use super::{collapsed_height, text_width};

    #[test]
    fn detail_description_caps_do_not_become_fluid() {
        assert_eq!(collapsed_height().fill_factor(), 0);
        assert_eq!(text_width().fill_factor(), 0);
    }
}
