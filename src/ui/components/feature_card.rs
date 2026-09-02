//! Wide home feature card used by Daily Recommend, Private Radar and FM.

use iced::widget::{Space, button, column, container, image, mouse_area, row, svg, text};
use iced::{Alignment, Color, Element, Fill, Length, Padding};

use crate::ui::{icons, theme};

const CARD_HEIGHT: f32 = 172.0;
const CARD_RADIUS: f32 = 14.0;
const PLAY_BUTTON_SIZE: f32 = 42.0;
const PLAY_BUTTON_START_OFFSET: f32 = 12.0;
const PLAY_BUTTON_END_OFFSET: f32 = 6.0;
const PLAY_ICON_SIZE: f32 = 19.0;
const HOVER_IMAGE_SCALE: f32 = 1.04;

#[allow(clippy::too_many_arguments)]
pub fn view<'a, Message: Clone + 'a>(
    title: String,
    subtitle: String,
    badge: Option<String>,
    icon: &'static str,
    cover_handle: Option<&'a iced::widget::image::Handle>,
    gradient: (Color, Color),
    width: Length,
    hover_progress: f32,
    on_click: Message,
    on_play: Message,
    on_hover: Message,
    on_unhover: Message,
) -> Element<'a, Message> {
    let background: Element<'a, Message> = if let Some(handle) = cover_handle {
        image(handle.clone())
            .width(Fill)
            .height(CARD_HEIGHT)
            .content_fit(iced::ContentFit::Cover)
            .border_radius(CARD_RADIUS)
            .scale(background_image_scale(hover_progress))
            .into()
    } else {
        container(Space::new())
            .width(Fill)
            .height(CARD_HEIGHT)
            .style(move |_theme| gradient_style(gradient))
            .into()
    };

    let scrim = container(Space::new())
        .width(Fill)
        .height(CARD_HEIGHT)
        .style(|_theme| scrim_style());

    let icon = svg(svg::Handle::from_memory(icon.as_bytes()))
        .width(24)
        .height(24)
        .style(|_theme, _status| svg::Style {
            color: Some(Color::WHITE),
        });
    let title = text(title)
        .size(theme::TEXT_SIZE_TITLE)
        .color(Color::WHITE)
        .font(iced::Font::DEFAULT.weight(theme::BOLD_WEIGHT));
    let mut top = row![icon, title]
        .spacing(9)
        .align_y(Alignment::Center)
        .width(Fill);
    if let Some(badge) = badge {
        top = top.push(Space::new().width(Fill)).push(
            text(badge)
                .size(theme::TEXT_SIZE_DISPLAY)
                .color(Color::from_rgba(1.0, 1.0, 1.0, 0.72))
                .font(iced::Font::DEFAULT.weight(theme::BOLD_WEIGHT)),
        );
    }

    let play: Element<'a, Message> = if hover_progress > 0.001 {
        let opacity = hover_progress.clamp(0.0, 1.0);
        let icon_size = play_icon_size(opacity);
        let play_button = button(
            container(
                svg(svg::Handle::from_memory(icons::PLAY.as_bytes()))
                    .width(icon_size)
                    .height(icon_size)
                    .style(|_theme, _status| svg::Style {
                        color: Some(Color::WHITE),
                    })
                    .opacity(opacity),
            )
            .width(PLAY_BUTTON_SIZE)
            .height(PLAY_BUTTON_SIZE)
            .center_x(PLAY_BUTTON_SIZE)
            .center_y(PLAY_BUTTON_SIZE),
        )
        .padding(0)
        .style(move |_theme, status| play_button_style(opacity, status))
        .on_press(on_play);

        container(play_button)
            .width(PLAY_BUTTON_SIZE)
            .height(PLAY_BUTTON_SIZE + PLAY_BUTTON_START_OFFSET)
            .padding(Padding::new(0.0).top(play_button_offset(opacity)))
            .into()
    } else {
        Space::new()
            .width(PLAY_BUTTON_SIZE)
            .height(PLAY_BUTTON_SIZE + PLAY_BUTTON_START_OFFSET)
            .into()
    };

    let bottom = row![
        text(subtitle)
            .size(theme::TEXT_SIZE_BODY)
            .color(Color::from_rgba(1.0, 1.0, 1.0, 0.82)),
        Space::new().width(Fill),
        play,
    ]
    .align_y(Alignment::End)
    .width(Fill);

    let foreground = container(column![top, Space::new().height(Fill), bottom])
        .width(Fill)
        .height(CARD_HEIGHT)
        .padding(16);
    let content = iced::widget::stack![background, scrim, foreground];

    let card = button(content)
        .padding(0)
        .width(width)
        .height(CARD_HEIGHT)
        .style(|_theme, _status| iced::widget::button::Style {
            background: None,
            border: iced::Border {
                radius: CARD_RADIUS.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .on_press(on_click);

    mouse_area(container(card).style(move |_theme| shadow_style(hover_progress)))
        .on_enter(on_hover.clone())
        .on_exit(on_unhover)
        .into()
}

fn gradient_style(colors: (Color, Color)) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(iced::Background::Gradient(iced::Gradient::Linear(
            iced::gradient::Linear::new(std::f32::consts::PI * 0.82)
                .add_stop(0.0, colors.0)
                .add_stop(1.0, colors.1),
        ))),
        border: iced::Border {
            radius: CARD_RADIUS.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn scrim_style() -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(iced::Background::Gradient(iced::Gradient::Linear(
            iced::gradient::Linear::new(std::f32::consts::FRAC_PI_2)
                .add_stop(0.0, Color::from_rgba(0.0, 0.0, 0.0, 0.1))
                .add_stop(1.0, Color::from_rgba(0.0, 0.0, 0.0, 0.62)),
        ))),
        border: iced::Border {
            radius: CARD_RADIUS.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn shadow_style(hover_progress: f32) -> iced::widget::container::Style {
    iced::widget::container::Style {
        border: iced::Border {
            radius: CARD_RADIUS.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.22 + 0.14 * hover_progress),
            offset: iced::Vector::new(0.0, 5.0 + 3.0 * hover_progress),
            blur_radius: 18.0 + 8.0 * hover_progress,
        },
        ..Default::default()
    }
}

fn play_button_offset(hover_progress: f32) -> f32 {
    let progress = hover_progress.clamp(0.0, 1.0);
    PLAY_BUTTON_START_OFFSET + (PLAY_BUTTON_END_OFFSET - PLAY_BUTTON_START_OFFSET) * progress
}

// Keep SVG raster bounds stable across the hover animation. Subpixel image
// bounds can abort preparation of the remaining Iced image layer.
fn play_icon_size(_hover_progress: f32) -> f32 {
    PLAY_ICON_SIZE
}

fn background_image_scale(hover_progress: f32) -> f32 {
    1.0 + (HOVER_IMAGE_SCALE - 1.0) * hover_progress.clamp(0.0, 1.0)
}

fn play_button_style(
    opacity: f32,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let status_alpha = match status {
        iced::widget::button::Status::Hovered => 0.98,
        iced::widget::button::Status::Pressed => 0.72,
        _ => 0.86,
    };
    let alpha = status_alpha * opacity;
    iced::widget::button::Style {
        background: Some(iced::Background::Color(Color::from_rgba(
            theme::ACCENT_PINK.r,
            theme::ACCENT_PINK.g,
            theme::ACCENT_PINK.b,
            alpha,
        ))),
        border: iced::Border {
            radius: 21.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.26 * opacity),
            offset: iced::Vector::new(0.0, 3.0),
            blur_radius: 8.0,
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HOVER_IMAGE_SCALE, PLAY_BUTTON_END_OFFSET, PLAY_BUTTON_START_OFFSET, PLAY_ICON_SIZE,
        background_image_scale, play_button_offset, play_icon_size,
    };

    #[test]
    fn feature_play_button_moves_up_as_hover_progresses() {
        assert_eq!(play_button_offset(0.0), PLAY_BUTTON_START_OFFSET);
        assert_eq!(
            play_button_offset(0.5),
            (PLAY_BUTTON_START_OFFSET + PLAY_BUTTON_END_OFFSET) / 2.0
        );
        assert_eq!(play_button_offset(1.0), PLAY_BUTTON_END_OFFSET);
    }

    #[test]
    fn feature_icon_stays_fixed_while_background_animates_with_hover() {
        assert_eq!(play_icon_size(0.0), PLAY_ICON_SIZE);
        assert_eq!(play_icon_size(0.5), PLAY_ICON_SIZE);
        assert_eq!(play_icon_size(1.0), PLAY_ICON_SIZE);
        assert_eq!(background_image_scale(0.0), 1.0);
        assert_eq!(background_image_scale(1.0), HOVER_IMAGE_SCALE);
    }
}
