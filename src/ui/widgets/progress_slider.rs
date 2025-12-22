//! Unified progress slider widget
//!
//! Provides a reusable progress slider with consistent styling.
//! Used by both the player bar and lyrics page.

use iced::widget::slider;
use iced::{Color, Element, Length};

use crate::app::Message;
use crate::ui::theme;

/// Size variant for progress slider
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SliderSize {
    /// Standard size for player bar (400px width)
    Standard,
    /// Full width for lyrics page
    Full,
}

/// Build the progress slider
///
/// # Arguments
/// * `position` - Current position (0.0 to 1.0)
/// * `size` - Size variant
pub fn view(position: f32, size: SliderSize) -> Element<'static, Message> {
    let clamped_position = position.clamp(0.0, 1.0);

    let width = match size {
        SliderSize::Standard => Length::Fixed(400.0),
        SliderSize::Full => Length::Fill,
    };

    slider(0.0..=1.0, clamped_position, Message::SeekPreview)
        .on_release(Message::SeekRelease)
        .width(width)
        .height(4)
        .step(0.001)
        .style(|iced_theme, status| {
            let handle_radius = match status {
                slider::Status::Hovered | slider::Status::Dragged => 6.0,
                _ => 0.0, // Hide handle when not interacting
            };
            slider::Style {
                rail: slider::Rail {
                    backgrounds: (
                        iced::Background::Color(theme::ACCENT_PINK),
                        iced::Background::Color(theme::divider(iced_theme)),
                    ),
                    width: 4.0,
                    border: iced::Border {
                        radius: 2.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                },
                handle: slider::Handle {
                    shape: slider::HandleShape::Circle {
                        radius: handle_radius,
                    },
                    background: iced::Background::Color(theme::ACCENT_PINK),
                    border_width: 0.0,
                    border_color: Color::TRANSPARENT,
                },
            }
        })
        .into()
}

/// Build a volume slider
///
/// # Arguments
/// * `volume` - Current volume (0.0 to 1.0)
pub fn volume_slider(volume: f32) -> Element<'static, Message> {
    slider(0.0..=1.0, volume, Message::SetVolume)
        .width(100)
        .height(4)
        .step(0.01)
        .shift_step(0.05)
        .style(|iced_theme, status| {
            let handle_radius = match status {
                slider::Status::Hovered | slider::Status::Dragged => 6.0,
                _ => 0.0,
            };
            slider::Style {
                rail: slider::Rail {
                    backgrounds: (
                        iced::Background::Color(theme::text_primary(iced_theme)),
                        iced::Background::Color(theme::divider(iced_theme)),
                    ),
                    width: 4.0,
                    border: iced::Border {
                        radius: 2.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                },
                handle: slider::Handle {
                    shape: slider::HandleShape::Circle {
                        radius: handle_radius,
                    },
                    background: iced::Background::Color(theme::text_primary(iced_theme)),
                    border_width: 0.0,
                    border_color: Color::TRANSPARENT,
                },
            }
        })
        .into()
}
