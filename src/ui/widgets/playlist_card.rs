//! Connected playlist card with a cover-derived blurred metadata footer.

use iced::widget::{Space, button, column, container, mouse_area, svg, text};
use iced::{Color, Element, Padding};

use crate::ui::responsive::{
    CardMetrics, CardRole, IconRole, ResponsiveContext, TextRole, UiTokens,
};
use crate::ui::theme::{self, MEDIUM_WEIGHT};
use crate::ui::{icons, widgets};

pub const CARD_WIDTH: f32 = 161.0;
const COVER_SIZE: f32 = CARD_WIDTH;
const FOOTER_HEIGHT: f32 = 56.0;
const CARD_RADIUS: f32 = 10.0;
const PLAY_BUTTON_SIZE: f32 = 48.0;
const PLAY_BUTTON_START_OFFSET: f32 = 10.0;
const PLAY_BUTTON_END_OFFSET: f32 = 4.0;
const PLAY_ICON_SIZE: f32 = 24.0;
const HOVER_MASK_MAX_ALPHA: f32 = 0.24;
const HOVER_IMAGE_SCALE: f32 = 1.06;

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
    view_with_metrics(
        name,
        cover_handle,
        footer_handle,
        hover_progress,
        on_click,
        on_play,
        on_hover,
        on_unhover,
        CardMetrics {
            width: CARD_WIDTH,
            height: CARD_WIDTH + FOOTER_HEIGHT,
            gap: 24.0,
            radius: CARD_RADIUS,
        },
        UiTokens::default(),
    )
}

/// Render a playlist card using the shared responsive card metrics.
#[allow(clippy::too_many_arguments)]
pub fn view_with_context<'a, Message: Clone + 'a>(
    name: &'a str,
    cover_handle: Option<&'a iced::widget::image::Handle>,
    footer_handle: Option<&'a iced::widget::image::Handle>,
    hover_progress: f32,
    on_click: Message,
    on_play: Message,
    on_hover: Message,
    on_unhover: Message,
    context: ResponsiveContext,
) -> Element<'a, Message> {
    view_with_metrics(
        name,
        cover_handle,
        footer_handle,
        hover_progress,
        on_click,
        on_play,
        on_hover,
        on_unhover,
        context.tokens.card(CardRole::Playlist),
        context.tokens,
    )
}

#[allow(clippy::too_many_arguments)]
fn view_with_metrics<'a, Message: Clone + 'a>(
    name: &'a str,
    cover_handle: Option<&'a iced::widget::image::Handle>,
    footer_handle: Option<&'a iced::widget::image::Handle>,
    hover_progress: f32,
    on_click: Message,
    on_play: Message,
    on_hover: Message,
    on_unhover: Message,
    metrics: CardMetrics,
    tokens: UiTokens,
) -> Element<'a, Message> {
    let card_width = metrics.width;
    let cover_size = metrics.width;
    let footer_height = (metrics.height - metrics.width).max(tokens.size(40.0));
    let card_radius = metrics.radius;
    let play_button_size = tokens.size(PLAY_BUTTON_SIZE).max(36.0);
    let play_button_start_offset = tokens.size(PLAY_BUTTON_START_OFFSET);
    let play_icon_dimension = tokens.icon(IconRole::Large);

    let placeholder = container(
        svg(svg::Handle::from_memory(icons::MUSIC.as_bytes()))
            .width(tokens.size(48.0).max(36.0))
            .height(tokens.size(48.0).max(36.0))
            .style(|theme, _status| svg::Style {
                color: Some(theme::opaque_color(theme::icon_muted(theme))),
            })
            .opacity(0.4_f32),
    )
    .width(cover_size)
    .height(cover_size)
    .center_x(cover_size)
    .center_y(cover_size)
    .style(move |theme| placeholder_style_for(theme, hover_progress, card_radius));
    let cover_image: Element<'a, Message> = widgets::crossfade_image(cover_handle.cloned())
        .width(cover_size)
        .height(cover_size)
        .content_fit(iced::ContentFit::Cover)
        .border_radius(cover_image_radius_for(card_radius))
        .scale(cover_image_scale(hover_progress))
        .into();
    let cover = iced::widget::stack![placeholder, cover_image];

    let play_overlay: Element<'a, Message> = if hover_progress > 0.01 {
        let opacity = hover_progress;
        let mask = container(Space::new())
            .width(cover_size)
            .height(cover_size)
            .style(move |_theme| cover_hover_mask_style_for(opacity, card_radius));
        let play_btn = button(
            container(
                svg(svg::Handle::from_memory(icons::PLAY.as_bytes()))
                    .width(play_icon_dimension)
                    .height(play_icon_dimension)
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
        .style(move |_theme, status| play_button_style_for(opacity, status, play_button_size))
        .on_press(on_play);

        let play_btn = container(play_btn)
            .width(play_button_size)
            .height(play_button_size + play_button_start_offset)
            .padding(Padding::new(0.0).top(play_button_offset_for(
                opacity,
                play_button_start_offset,
                tokens.size(PLAY_BUTTON_END_OFFSET),
            )));
        let play_btn = container(play_btn)
            .width(cover_size)
            .height(cover_size)
            .center_x(cover_size)
            .center_y(cover_size);

        iced::widget::stack![mask, play_btn].into()
    } else {
        Space::new().width(0).height(0).into()
    };

    let cover = iced::widget::stack![cover, play_overlay];

    let footer_fallback = container(Space::new())
        .width(card_width)
        .height(footer_height)
        .style(move |theme| footer_fallback_style_for(theme, card_radius));
    let footer_image: Element<'a, Message> = widgets::crossfade_image(footer_handle.cloned())
        .width(card_width)
        .height(footer_height)
        .content_fit(iced::ContentFit::Cover)
        .border_radius(footer_image_radius_for(card_radius))
        .into();
    let footer_background = iced::widget::stack![footer_fallback, footer_image];

    let scrim = container(Space::new())
        .width(card_width)
        .height(footer_height)
        .style(move |_theme| footer_scrim_style_for(card_radius));

    let name_text = text(truncate_text(name, 20))
        .size(tokens.text(TextRole::Body))
        .color(Color::WHITE)
        .font(iced::Font::DEFAULT.weight(MEDIUM_WEIGHT));

    let footer_text = container(name_text)
        .width(card_width)
        .height(footer_height)
        .padding([tokens.space(8.0), tokens.space(10.0)])
        .align_y(iced::Alignment::Center);

    let footer = iced::widget::stack![footer_background, scrim, footer_text];
    let content = column![cover, footer].spacing(0).width(card_width);

    let card = button(content)
        .padding(0)
        .width(card_width)
        .style(move |_theme, _status| iced::widget::button::Style {
            background: None,
            border: iced::Border {
                radius: card_radius.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .on_press(on_click);

    let elevated =
        container(card).style(move |_theme| card_shadow_style_for(hover_progress, card_radius));
    mouse_area(elevated)
        .on_enter(on_hover.clone())
        .on_exit(on_unhover)
        .into()
}

/// Iced 0.14's image shader applies partial top/bottom radii vertically
/// inverted. Keep the workaround local to image-backed card segments.
fn cover_image_radius() -> iced::border::Radius {
    cover_image_radius_for(CARD_RADIUS)
}

fn cover_image_radius_for(radius: f32) -> iced::border::Radius {
    iced::border::bottom(radius)
}

fn footer_image_radius() -> iced::border::Radius {
    footer_image_radius_for(CARD_RADIUS)
}

fn footer_image_radius_for(radius: f32) -> iced::border::Radius {
    iced::border::top(radius)
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
    card_shadow_style_for(hover_progress, CARD_RADIUS)
}

fn card_shadow_style_for(hover_progress: f32, radius: f32) -> iced::widget::container::Style {
    iced::widget::container::Style {
        border: iced::Border {
            radius: radius.into(),
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
    cover_hover_mask_style_for(hover_progress, CARD_RADIUS)
}

fn cover_hover_mask_style_for(hover_progress: f32, radius: f32) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(iced::Background::Color(Color::from_rgba(
            0.0,
            0.0,
            0.0,
            hover_mask_alpha(hover_progress),
        ))),
        border: iced::Border {
            radius: iced::border::top(radius),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn hover_mask_alpha(hover_progress: f32) -> f32 {
    HOVER_MASK_MAX_ALPHA * hover_progress.clamp(0.0, 1.0)
}

fn play_button_offset(hover_progress: f32) -> f32 {
    play_button_offset_for(
        hover_progress,
        PLAY_BUTTON_START_OFFSET,
        PLAY_BUTTON_END_OFFSET,
    )
}

fn play_button_offset_for(hover_progress: f32, start: f32, end: f32) -> f32 {
    let progress = hover_progress.clamp(0.0, 1.0);
    start + (end - start) * progress
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
    placeholder_style_for(theme, hover_progress, CARD_RADIUS)
}

fn placeholder_style_for(
    theme: &iced::Theme,
    hover_progress: f32,
    radius: f32,
) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(iced::Background::Color(theme::surface_container(theme))),
        border: iced::Border {
            radius: iced::border::top(radius),
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
    footer_fallback_style_for(theme, CARD_RADIUS)
}

fn footer_fallback_style_for(theme: &iced::Theme, radius: f32) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(iced::Background::Color(theme::surface_elevated(theme))),
        border: iced::Border {
            radius: iced::border::bottom(radius),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn footer_scrim_style(_theme: &iced::Theme) -> iced::widget::container::Style {
    footer_scrim_style_for(CARD_RADIUS)
}

fn footer_scrim_style_for(radius: f32) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(iced::Background::Color(Color::from_rgba(
            0.02, 0.02, 0.025, 0.34,
        ))),
        border: iced::Border {
            radius: iced::border::bottom(radius),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn play_button_style(
    opacity: f32,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    play_button_style_for(opacity, status, PLAY_BUTTON_SIZE)
}

fn play_button_style_for(
    opacity: f32,
    status: iced::widget::button::Status,
    button_size: f32,
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
            radius: (button_size / 2.0).into(),
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
