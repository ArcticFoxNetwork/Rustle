//! Connected playlist card with a cover-derived blurred metadata footer.

use iced::widget::{Space, button, column, container, image, mouse_area, svg, text};
use iced::{Color, Element, Padding};

use crate::ui::icons;
use crate::ui::theme::{self, MEDIUM_WEIGHT};

pub const CARD_WIDTH: f32 = 160.0;
const COVER_SIZE: f32 = CARD_WIDTH;
const FOOTER_HEIGHT: f32 = 56.0;
const CARD_RADIUS: f32 = 10.0;
const PLAY_BUTTON_SIZE: f32 = 48.0;
const PLAY_BUTTON_START_OFFSET: f32 = 10.0;
const PLAY_BUTTON_END_OFFSET: f32 = 4.0;
const PLAY_ICON_SIZE: f32 = 24.0;
const HOVER_MASK_MAX_ALPHA: f32 = 0.24;
const HOVER_IMAGE_SCALE: f32 = 1.04;

#[allow(clippy::too_many_arguments)]
pub fn view<'a, Message: Clone + 'a>(
    name: &'a str,
    cover_handle: Option<&'a iced::widget::image::Handle>,
    footer_handle: Option<&'a iced::widget::image::Handle>,
    hover_progress: f32,
    on_click: Message,
    on_play: Message,
    on_hover: Message,
    on_unhover: Message,
) -> Element<'a, Message> {
    let cover: Element<'a, Message> = if let Some(handle) = cover_handle {
        image(handle.clone())
            .width(COVER_SIZE)
            .height(COVER_SIZE)
            .content_fit(iced::ContentFit::Cover)
            .border_radius(cover_image_radius())
            .scale(cover_image_scale(hover_progress))
            .into()
    } else {
        container(
            svg(svg::Handle::from_memory(icons::MUSIC.as_bytes()))
                .width(48)
                .height(48)
                .style(|theme, _status| svg::Style {
                    color: Some(theme::opaque_color(theme::icon_muted(theme))),
                })
                .opacity(0.4),
        )
        .width(COVER_SIZE)
        .height(COVER_SIZE)
        .center_x(COVER_SIZE)
        .center_y(COVER_SIZE)
        .style(move |theme| placeholder_style(theme, hover_progress))
        .into()
    };

    let play_overlay: Element<'a, Message> = if hover_progress > 0.01 {
        let opacity = hover_progress;
        let mask = container(Space::new())
            .width(COVER_SIZE)
            .height(COVER_SIZE)
            .style(move |_theme| cover_hover_mask_style(opacity));
        let icon_size = play_icon_size(opacity);
        let play_btn = button(
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

        let play_btn = container(play_btn)
            .width(PLAY_BUTTON_SIZE)
            .height(PLAY_BUTTON_SIZE + PLAY_BUTTON_START_OFFSET)
            .padding(Padding::new(0.0).top(play_button_offset(opacity)));
        let play_btn = container(play_btn)
            .width(COVER_SIZE)
            .height(COVER_SIZE)
            .center_x(COVER_SIZE)
            .center_y(COVER_SIZE);

        iced::widget::stack![mask, play_btn].into()
    } else {
        Space::new().width(0).height(0).into()
    };

    let cover = iced::widget::stack![cover, play_overlay];

    let footer_background: Element<'a, Message> = if let Some(handle) = footer_handle {
        image(handle.clone())
            .width(CARD_WIDTH)
            .height(FOOTER_HEIGHT)
            .content_fit(iced::ContentFit::Cover)
            .border_radius(footer_image_radius())
            .into()
    } else {
        container(Space::new())
            .width(CARD_WIDTH)
            .height(FOOTER_HEIGHT)
            .style(footer_fallback_style)
            .into()
    };

    let scrim = container(Space::new())
        .width(CARD_WIDTH)
        .height(FOOTER_HEIGHT)
        .style(footer_scrim_style);

    let name_text = text(truncate_text(name, 20))
        .size(theme::TEXT_SIZE_BODY)
        .color(Color::WHITE)
        .font(iced::Font::DEFAULT.weight(MEDIUM_WEIGHT));

    let footer_text = container(name_text)
        .width(CARD_WIDTH)
        .height(FOOTER_HEIGHT)
        .padding([8, 10])
        .align_y(iced::Alignment::Center);

    let footer = iced::widget::stack![footer_background, scrim, footer_text];
    let content = column![cover, footer].spacing(0).width(CARD_WIDTH);

    let card = button(content)
        .padding(0)
        .width(CARD_WIDTH)
        .style(|_theme, _status| iced::widget::button::Style {
            background: None,
            border: iced::Border {
                radius: CARD_RADIUS.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .on_press(on_click);

    let elevated = container(card).style(move |_theme| card_shadow_style(hover_progress));
    mouse_area(elevated)
        .on_enter(on_hover.clone())
        .on_exit(on_unhover)
        .into()
}

/// Iced 0.14's image shader applies partial top/bottom radii vertically
/// inverted. Keep the workaround local to image-backed card segments.
fn cover_image_radius() -> iced::border::Radius {
    iced::border::bottom(CARD_RADIUS)
}

fn footer_image_radius() -> iced::border::Radius {
    iced::border::top(CARD_RADIUS)
}

fn truncate_text(s: &str, max_chars: usize) -> String {
    if s.chars().count() > max_chars {
        let truncated: String = s.chars().take(max_chars - 1).collect();
        format!("{}…", truncated)
    } else {
        s.to_string()
    }
}

fn card_shadow_style(hover_progress: f32) -> iced::widget::container::Style {
    iced::widget::container::Style {
        border: iced::Border {
            radius: CARD_RADIUS.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.28 + 0.16 * hover_progress),
            offset: iced::Vector::new(0.0, 4.0 + 4.0 * hover_progress),
            blur_radius: 14.0 + 8.0 * hover_progress,
        },
        ..Default::default()
    }
}

fn cover_hover_mask_style(hover_progress: f32) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(iced::Background::Color(Color::from_rgba(
            0.0,
            0.0,
            0.0,
            hover_mask_alpha(hover_progress),
        ))),
        border: iced::Border {
            radius: iced::border::top(CARD_RADIUS),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn hover_mask_alpha(hover_progress: f32) -> f32 {
    HOVER_MASK_MAX_ALPHA * hover_progress.clamp(0.0, 1.0)
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

fn cover_image_scale(hover_progress: f32) -> f32 {
    1.0 + (HOVER_IMAGE_SCALE - 1.0) * hover_progress.clamp(0.0, 1.0)
}

fn placeholder_style(theme: &iced::Theme, hover_progress: f32) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(iced::Background::Color(theme::surface_container(theme))),
        border: iced::Border {
            radius: iced::border::top(CARD_RADIUS),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.12 * hover_progress),
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        },
        ..Default::default()
    }
}

fn footer_fallback_style(theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(iced::Background::Color(theme::surface_elevated(theme))),
        border: iced::Border {
            radius: iced::border::bottom(CARD_RADIUS),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn footer_scrim_style(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(iced::Background::Color(Color::from_rgba(
            0.02, 0.02, 0.025, 0.34,
        ))),
        border: iced::Border {
            radius: iced::border::bottom(CARD_RADIUS),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn play_button_style(
    opacity: f32,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let bg_alpha = match status {
        iced::widget::button::Status::Hovered => opacity,
        iced::widget::button::Status::Pressed => 0.8 * opacity,
        _ => 0.9 * opacity,
    };

    iced::widget::button::Style {
        background: Some(iced::Background::Color(Color::from_rgba(
            theme::ACCENT_PINK.r,
            theme::ACCENT_PINK.g,
            theme::ACCENT_PINK.b,
            bg_alpha,
        ))),
        border: iced::Border {
            radius: 24.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.3 * opacity),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 8.0,
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CARD_RADIUS, HOVER_IMAGE_SCALE, HOVER_MASK_MAX_ALPHA, PLAY_BUTTON_END_OFFSET,
        PLAY_BUTTON_START_OFFSET, PLAY_ICON_SIZE, cover_image_radius, cover_image_scale,
        footer_image_radius, hover_mask_alpha, play_button_offset, play_icon_size,
    };

    #[test]
    fn image_radius_workaround_keeps_the_visual_join_square() {
        let cover = cover_image_radius();
        assert_eq!(cover.top_left, 0.0);
        assert_eq!(cover.top_right, 0.0);
        assert_eq!(cover.bottom_left, CARD_RADIUS);
        assert_eq!(cover.bottom_right, CARD_RADIUS);

        let footer = footer_image_radius();
        assert_eq!(footer.top_left, CARD_RADIUS);
        assert_eq!(footer.top_right, CARD_RADIUS);
        assert_eq!(footer.bottom_left, 0.0);
        assert_eq!(footer.bottom_right, 0.0);
    }

    #[test]
    fn hover_layers_fade_and_move_continuously() {
        assert_eq!(hover_mask_alpha(0.0), 0.0);
        assert_eq!(hover_mask_alpha(0.5), HOVER_MASK_MAX_ALPHA / 2.0);
        assert_eq!(hover_mask_alpha(1.0), HOVER_MASK_MAX_ALPHA);
        assert_eq!(play_button_offset(0.0), PLAY_BUTTON_START_OFFSET);
        assert_eq!(
            play_button_offset(0.5),
            (PLAY_BUTTON_START_OFFSET + PLAY_BUTTON_END_OFFSET) / 2.0
        );
        assert_eq!(play_button_offset(1.0), PLAY_BUTTON_END_OFFSET);
        assert_eq!(play_icon_size(0.0), PLAY_ICON_SIZE);
        assert_eq!(play_icon_size(0.5), PLAY_ICON_SIZE);
        assert_eq!(play_icon_size(1.0), PLAY_ICON_SIZE);
        assert_eq!(cover_image_scale(0.0), 1.0);
        assert_eq!(cover_image_scale(1.0), HOVER_IMAGE_SCALE);
    }
}
