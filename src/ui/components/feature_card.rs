//! Wide home feature card used by Daily Recommend, Private Radar and FM.

use iced::widget::{Space, button, column, container, mouse_area, row, svg, text};
use iced::{Alignment, Color, Element, Fill, Length, Padding};

use crate::ui::responsive::{
    CardRole, IconRole, RadiusRole, ResponsiveContext, TargetRole, TextRole, UiTokens,
};
use crate::ui::{icons, theme, widgets};

// Visual dimensions are 1080P reference pixels resolved through `UiTokens`.
const PLAY_BUTTON_SIZE: f32 = 42.0;
const PLAY_BUTTON_START_OFFSET: f32 = 12.0;
const PLAY_BUTTON_END_OFFSET: f32 = 6.0;
const PLAY_ICON_SIZE: f32 = 19.0;
const HOVER_IMAGE_SCALE: f32 = 1.06;

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
    context: ResponsiveContext,
) -> Element<'a, Message> {
    let tokens = context.tokens;
    let metrics = tokens.card(CardRole::Feature);
    let card_height = metrics.height;
    let card_radius = metrics.radius.max(tokens.radius(RadiusRole::Medium));
    let play_button_size = tokens
        .target(TargetRole::Icon)
        .max(tokens.size(PLAY_BUTTON_SIZE));

    let fallback = container(Space::new())
        .width(Fill)
        .height(card_height)
        .style(move |_theme| gradient_style(gradient, card_radius));
    let cover: Element<'a, Message> = widgets::crossfade_image(cover_handle.cloned())
        .width(Fill)
        .height(card_height)
        .content_fit(iced::ContentFit::Cover)
        .content_position(widgets::ContentPosition::TOP)
        .border_radius(card_radius)
        .scale(background_image_scale(hover_progress))
        .into();
    let background = iced::widget::stack![fallback, cover];

    let scrim = container(Space::new())
        .width(Fill)
        .height(card_height)
        .style(move |_theme| scrim_style(card_radius));

    let icon = svg(svg::Handle::from_memory(icon.as_bytes()))
        .width(tokens.icon(IconRole::Large))
        .height(tokens.icon(IconRole::Large))
        .style(|_theme, _status| svg::Style {
            color: Some(Color::WHITE),
        });
    let title = text(title)
        .size(tokens.text(TextRole::Title))
        .color(Color::WHITE)
        .font(iced::Font::DEFAULT.weight(theme::BOLD_WEIGHT));
    let mut top = row![icon, title]
        .spacing(tokens.space(9.0))
        .align_y(Alignment::Center)
        .width(Fill);
    if let Some(badge) = badge {
        top = top.push(Space::new().width(Fill)).push(
            text(badge)
                .size(tokens.text(TextRole::Display))
                .color(Color::from_rgba(1.0, 1.0, 1.0, 0.72))
                .font(iced::Font::DEFAULT.weight(theme::BOLD_WEIGHT)),
        );
    }

    let play: Element<'a, Message> = if hover_progress > 0.001 {
        let opacity = hover_progress.clamp(0.0, 1.0);
        let icon_size = tokens.icon(IconRole::Medium).max(play_icon_size(tokens));
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
            .width(play_button_size)
            .height(play_button_size)
            .center_x(play_button_size)
            .center_y(play_button_size),
        )
        .padding(0)
        .style(move |_theme, status| {
            play_button_style(opacity, status, play_button_size / 2.0, tokens)
        })
        .on_press(on_play);

        container(play_button)
            .width(play_button_size)
            .height(play_button_size + tokens.space(PLAY_BUTTON_START_OFFSET))
            .padding(Padding::new(0.0).top(play_button_offset_for(
                opacity,
                tokens.space(PLAY_BUTTON_START_OFFSET),
                tokens.space(PLAY_BUTTON_END_OFFSET),
            )))
            .into()
    } else {
        Space::new()
            .width(play_button_size)
            .height(play_button_size + tokens.space(PLAY_BUTTON_START_OFFSET))
            .into()
    };

    let bottom = row![
        text(subtitle)
            .size(tokens.text(TextRole::Body))
            .color(Color::from_rgba(1.0, 1.0, 1.0, 0.82)),
        Space::new().width(Fill),
        play,
    ]
    .align_y(Alignment::End)
    .width(Fill);

    let foreground = container(column![top, Space::new().height(Fill), bottom])
        .width(Fill)
        .height(card_height)
        .padding(tokens.space(16.0));
    let content = iced::widget::stack![background, scrim, foreground];

    let card = button(content)
        .padding(0)
        .width(width)
        .height(card_height)
        .style(move |_theme, _status| iced::widget::button::Style {
            background: None,
            border: iced::Border {
                radius: card_radius.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .on_press(on_click);

    mouse_area(
        container(card).style(move |_theme| shadow_style(hover_progress, card_radius, tokens)),
    )
    .on_enter(on_hover.clone())
    .on_exit(on_unhover)
    .into()
}

fn gradient_style(colors: (Color, Color), radius: f32) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(iced::Background::Gradient(iced::Gradient::Linear(
            iced::gradient::Linear::new(std::f32::consts::PI * 0.82)
                .add_stop(0.0, colors.0)
                .add_stop(1.0, colors.1),
        ))),
        border: iced::Border {
            radius: radius.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn scrim_style(radius: f32) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(iced::Background::Gradient(iced::Gradient::Linear(
            iced::gradient::Linear::new(std::f32::consts::FRAC_PI_2)
                .add_stop(0.0, Color::from_rgba(0.0, 0.0, 0.0, 0.1))
                .add_stop(1.0, Color::from_rgba(0.0, 0.0, 0.0, 0.62)),
        ))),
        border: iced::Border {
            radius: radius.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn shadow_style(
    hover_progress: f32,
    radius: f32,
    tokens: UiTokens,
) -> iced::widget::container::Style {
    iced::widget::container::Style {
        border: iced::Border {
            radius: radius.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.22 + 0.14 * hover_progress),
            offset: iced::Vector::new(0.0, tokens.size(5.0 + 3.0 * hover_progress)),
            blur_radius: tokens.size(18.0 + 8.0 * hover_progress),
        },
        ..Default::default()
    }
}

fn play_button_offset_for(hover_progress: f32, start: f32, end: f32) -> f32 {
    let progress = hover_progress.clamp(0.0, 1.0);
    start + (end - start) * progress
}

// Keep SVG raster bounds stable across the hover animation. Subpixel image
// bounds can abort preparation of the remaining Iced image layer.
fn play_icon_size(tokens: UiTokens) -> f32 {
    tokens.size(PLAY_ICON_SIZE)
}

fn background_image_scale(hover_progress: f32) -> f32 {
    1.0 + (HOVER_IMAGE_SCALE - 1.0) * hover_progress.clamp(0.0, 1.0)
}

fn play_button_style(
    opacity: f32,
    status: iced::widget::button::Status,
    radius: f32,
    tokens: UiTokens,
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
            radius: radius.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.26 * opacity),
            offset: iced::Vector::new(0.0, tokens.size(3.0)),
            blur_radius: tokens.size(8.0),
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HOVER_IMAGE_SCALE, PLAY_BUTTON_END_OFFSET, PLAY_BUTTON_START_OFFSET, PLAY_ICON_SIZE,
        background_image_scale, play_button_offset_for, play_icon_size,
    };
    use crate::ui::responsive::UiTokens;

    #[test]
    fn feature_play_button_moves_up_as_hover_progresses() {
        assert_eq!(
            play_button_offset_for(0.0, PLAY_BUTTON_START_OFFSET, PLAY_BUTTON_END_OFFSET),
            PLAY_BUTTON_START_OFFSET
        );
        assert_eq!(
            play_button_offset_for(0.5, PLAY_BUTTON_START_OFFSET, PLAY_BUTTON_END_OFFSET),
            (PLAY_BUTTON_START_OFFSET + PLAY_BUTTON_END_OFFSET) / 2.0
        );
        assert_eq!(
            play_button_offset_for(1.0, PLAY_BUTTON_START_OFFSET, PLAY_BUTTON_END_OFFSET),
            PLAY_BUTTON_END_OFFSET
        );
    }

    #[test]
    fn feature_icon_stays_fixed_while_background_animates_with_hover() {
        let tokens = UiTokens::default();
        assert_eq!(play_icon_size(tokens), PLAY_ICON_SIZE);
        assert_eq!(background_image_scale(0.0), 1.0);
        assert_eq!(background_image_scale(1.0), HOVER_IMAGE_SCALE);
    }
}
