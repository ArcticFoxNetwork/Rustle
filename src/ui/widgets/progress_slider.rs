//! Unified progress slider widget
//!
//! Provides a reusable progress slider with consistent styling.
//! Used by both the player bar and lyrics page.

use iced::widget::{slider, vertical_slider};
use iced::{Color, Element, Length};

use super::multi_track_slider::{self, MultiTrackSlider};
use crate::app::Message;
use crate::ui::theme;

/// Size variant for progress slider
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SliderSize {
    /// Full width for lyrics page
    Full,
    /// Full-width edge progress for the top of the player bar
    Edge,
}

/// Build the progress slider with optional download progress indicator
///
/// # Arguments
/// * `position` - Current playback position (0.0 to 1.0)
/// * `download_progress` - Download progress (0.0 to 1.0), None if not streaming
/// * `size` - Size variant
pub fn view(
    position: f32,
    download_progress: Option<f32>,
    size: SliderSize,
) -> Element<'static, Message> {
    view_with_gradient(position, download_progress, size, None)
}

/// Build a progress slider whose played track can use cover-derived colors.
pub fn view_with_gradient(
    position: f32,
    download_progress: Option<f32>,
    size: SliderSize,
    played_gradient: Option<[Color; 3]>,
) -> Element<'static, Message> {
    let clamped_position = position.clamp(0.0, 1.0);
    let played_gradient = played_gradient.map(colors_light_to_dark);

    let width = Length::Fill;

    let height = match size {
        SliderSize::Edge => 8,
        SliderSize::Full => 16,
    };

    // Use multi-track slider for download progress display
    MultiTrackSlider::new(0.0..=1.0, clamped_position, Message::SeekPreview)
        .secondary(download_progress)
        .on_release(Message::SeekRelease)
        .width(width)
        .height(height)
        .step(0.001)
        .style(move |iced_theme, status| {
            let fallback_color = if size == SliderSize::Full {
                Color::WHITE
            } else {
                theme::ACCENT_PINK
            };
            let played_background = played_gradient.map_or(
                iced::Background::Color(fallback_color),
                |[light, middle, dark]| {
                    iced::Background::Gradient(iced::Gradient::Linear(
                        iced::gradient::Linear::new(std::f32::consts::FRAC_PI_2)
                            .add_stop(0.0, light)
                            .add_stop(0.5, middle)
                            .add_stop(1.0, dark),
                    ))
                },
            );
            let handle_color = played_gradient.map_or(fallback_color, |[light, _, _]| light);
            let rail_radius = if size == SliderSize::Edge { 0.0 } else { 2.0 };
            let handle_radius = match status {
                multi_track_slider::Status::Hovered | multi_track_slider::Status::Dragged => 6.0,
                _ => 0.0, // Hide handle when not interacting
            };
            multi_track_slider::Style {
                rail: multi_track_slider::Rail {
                    backgrounds: (
                        played_background,
                        iced::Background::Color(if size == SliderSize::Edge {
                            theme::player_bar_border(iced_theme)
                        } else {
                            theme::divider(iced_theme)
                        }),
                    ),
                    // Downloaded but not played - slightly brighter than background
                    secondary_background: Some(iced::Background::Color(Color::from_rgba(
                        0.6, 0.6, 0.6, 0.5,
                    ))),
                    width: 4.0,
                    border: iced::Border {
                        radius: rail_radius.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                },
                handle: multi_track_slider::Handle {
                    shape: multi_track_slider::HandleShape::Circle {
                        radius: handle_radius,
                    },
                    background: iced::Background::Color(handle_color),
                    border_width: 0.0,
                    border_color: Color::TRANSPARENT,
                },
            }
        })
        .into()
}

fn colors_light_to_dark(mut colors: [Color; 3]) -> [Color; 3] {
    colors.sort_by(|left, right| {
        perceived_brightness(*right).total_cmp(&perceived_brightness(*left))
    });
    colors
}

fn perceived_brightness(color: Color) -> f32 {
    color.r * 0.299 + color.g * 0.587 + color.b * 0.114
}

/// Build a volume slider
///
/// # Arguments
/// * `volume` - Current volume (0.0 to 1.0)
pub fn volume_slider(volume: f32, width: f32) -> Element<'static, Message> {
    slider(0.0..=1.0, volume, Message::SetVolume)
        .width(width)
        .height(4)
        .step(0.01_f32)
        .shift_step(0.05_f32)
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

/// Build the vertical volume slider used by a narrow player bar popup.
pub fn vertical_volume_slider(volume: f32, height: f32) -> Element<'static, Message> {
    vertical_slider(0.0..=1.0, volume, Message::SetVolume)
        .width(16)
        .height(height)
        .step(0.01_f32)
        .shift_step(0.05_f32)
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

#[cfg(test)]
mod tests {
    use super::{colors_light_to_dark, perceived_brightness};
    use iced::Color;

    #[test]
    fn gradient_colors_are_ordered_from_light_to_dark() {
        let colors = colors_light_to_dark([
            Color::from_rgb(0.1, 0.1, 0.1),
            Color::from_rgb(0.9, 0.9, 0.9),
            Color::from_rgb(0.5, 0.5, 0.5),
        ]);

        assert!(perceived_brightness(colors[0]) > perceived_brightness(colors[1]));
        assert!(perceived_brightness(colors[1]) > perceived_brightness(colors[2]));
    }
}
