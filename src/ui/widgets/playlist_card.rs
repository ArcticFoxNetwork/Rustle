//! Playlist card widget
//!
//! Displays a playlist with cover image, name, and author.
//! Supports hover animation with play button overlay.
//!
//! # Design
//!
//! This is a reusable widget that uses generic Message types.
//! It does not depend on application-specific types.

use iced::widget::{Space, button, column, container, image, mouse_area, svg, text};
use iced::{Alignment, Color, Element, Fill};

use crate::ui::icons;
use crate::ui::theme::{self, MEDIUM_WEIGHT};

/// Card size configuration
const COVER_SIZE: f32 = 160.0;
const COVER_RADIUS: f32 = 8.0;

/// Create a playlist card element
///
/// # Arguments
/// * `name` - Playlist name
/// * `author` - Author/creator name
/// * `cover_handle` - Optional cover image handle
/// * `hover_progress` - Hover animation progress (0.0 to 1.0)
/// * `on_click` - Message to send when card is clicked
/// * `on_play` - Message to send when play button is clicked
/// * `on_hover` - Message to send when card is hovered
/// * `on_unhover` - Message to send when mouse leaves the card
pub fn view<'a, Message: Clone + 'a>(
    name: &'a str,
    author: &'a str,
    cover_handle: Option<&'a iced::widget::image::Handle>,
    hover_progress: f32,
    on_click: Message,
    on_play: Message,
    on_hover: Message,
    on_unhover: Message,
) -> Element<'a, Message> {
    // Cover image or placeholder
    let cover: Element<'a, Message> = if let Some(handle) = cover_handle {
        container(
            image(handle.clone())
                .width(Fill)
                .height(Fill)
                .content_fit(iced::ContentFit::Cover)
                .border_radius(COVER_RADIUS),
        )
        .width(COVER_SIZE)
        .height(COVER_SIZE)
        .style(move |_theme| cover_style(hover_progress))
        .into()
    } else {
        // Placeholder with music icon
        container(
            svg(svg::Handle::from_memory(icons::MUSIC.as_bytes()))
                .width(48)
                .height(48)
                .style(|_theme, _status| svg::Style {
                    color: Some(theme::icon_muted(&iced::Theme::Dark)),
                }),
        )
        .width(COVER_SIZE)
        .height(COVER_SIZE)
        .center_x(COVER_SIZE)
        .center_y(COVER_SIZE)
        .style(move |_theme| placeholder_style(hover_progress))
        .into()
    };

    // Play button overlay (visible on hover)
    let play_overlay: Element<'a, Message> = if hover_progress > 0.01 {
        let opacity = hover_progress;
        let play_btn = button(
            container(
                svg(svg::Handle::from_memory(icons::PLAY.as_bytes()))
                    .width(24)
                    .height(24)
                    .style(move |_theme, _status| svg::Style {
                        color: Some(Color::from_rgba(1.0, 1.0, 1.0, opacity)),
                    }),
            )
            .width(48)
            .height(48)
            .center_x(48)
            .center_y(48),
        )
        .padding(0)
        .style(move |_theme, status| play_button_style(opacity, status))
        .on_press(on_play);

        container(play_btn)
            .width(COVER_SIZE)
            .height(COVER_SIZE)
            .center_x(COVER_SIZE)
            .center_y(COVER_SIZE)
            .into()
    } else {
        Space::new().width(0).height(0).into()
    };

    // Stack cover and play overlay
    let cover_with_overlay = iced::widget::stack![cover, play_overlay];

    // Playlist name (truncated)
    let name_text = text(truncate_text(name, 20))
        .size(14)
        .style(|theme| text::Style {
            color: Some(theme::text_primary(theme)),
        })
        .font(iced::Font {
            weight: MEDIUM_WEIGHT,
            ..Default::default()
        });

    // Author name
    let author_text = text(truncate_text(author, 25))
        .size(12)
        .color(theme::TEXT_MUTED);

    // Card content
    let content = column![
        cover_with_overlay,
        Space::new().height(8),
        name_text,
        Space::new().height(2),
        author_text,
    ]
    .width(COVER_SIZE)
    .align_x(Alignment::Start);

    // Wrap in clickable container
    let card = button(content)
        .padding(0)
        .style(|_theme, _status| iced::widget::button::Style {
            background: None,
            ..Default::default()
        })
        .on_press(on_click);

    // Add hover detection
    mouse_area(card)
        .on_enter(on_hover.clone())
        .on_exit(on_unhover)
        .into()
}

/// Create a playlist card with custom cover element
///
/// Same as `view` but accepts a custom cover element instead of an image handle.
pub fn view_with_custom_cover<'a, Message: Clone + 'a>(
    name: &'a str,
    author: &'a str,
    cover: Element<'a, Message>,
    hover_progress: f32,
    on_click: Message,
    on_play: Message,
    on_hover: Message,
    on_unhover: Message,
) -> Element<'a, Message> {
    // Play button overlay (visible on hover)
    let play_overlay: Element<'a, Message> = if hover_progress > 0.01 {
        let opacity = hover_progress;
        let play_btn = button(
            container(
                svg(svg::Handle::from_memory(icons::PLAY.as_bytes()))
                    .width(24)
                    .height(24)
                    .style(move |_theme, _status| svg::Style {
                        color: Some(Color::from_rgba(1.0, 1.0, 1.0, opacity)),
                    }),
            )
            .width(48)
            .height(48)
            .center_x(48)
            .center_y(48),
        )
        .padding(0)
        .style(move |_theme, status| play_button_style(opacity, status))
        .on_press(on_play);

        container(play_btn)
            .width(COVER_SIZE)
            .height(COVER_SIZE)
            .center_x(COVER_SIZE)
            .center_y(COVER_SIZE)
            .into()
    } else {
        Space::new().width(0).height(0).into()
    };

    // Stack cover and play overlay
    let cover_with_overlay = iced::widget::stack![cover, play_overlay];

    // Playlist name (truncated)
    let name_text = text(truncate_text(name, 20))
        .size(14)
        .style(|theme| text::Style {
            color: Some(theme::text_primary(theme)),
        })
        .font(iced::Font {
            weight: MEDIUM_WEIGHT,
            ..Default::default()
        });

    // Author name
    let author_text = text(truncate_text(author, 25))
        .size(12)
        .color(theme::TEXT_MUTED);

    // Card content
    let content = column![
        cover_with_overlay,
        Space::new().height(8),
        name_text,
        Space::new().height(2),
        author_text,
    ]
    .width(COVER_SIZE)
    .align_x(Alignment::Start);

    // Wrap in clickable container
    let card = button(content)
        .padding(0)
        .style(|_theme, _status| iced::widget::button::Style {
            background: None,
            ..Default::default()
        })
        .on_press(on_click);

    // Add hover detection
    mouse_area(card)
        .on_enter(on_hover.clone())
        .on_exit(on_unhover)
        .into()
}

/// Truncate text with ellipsis if too long
fn truncate_text(s: &str, max_chars: usize) -> String {
    if s.chars().count() > max_chars {
        let truncated: String = s.chars().take(max_chars - 1).collect();
        format!("{}…", truncated)
    } else {
        s.to_string()
    }
}

/// Cover container style with hover effect
pub fn cover_style(hover_progress: f32) -> iced::widget::container::Style {
    let shadow_blur = 16.0 + 8.0 * hover_progress;
    let shadow_alpha = 0.3 + 0.2 * hover_progress;

    iced::widget::container::Style {
        border: iced::Border {
            radius: COVER_RADIUS.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, shadow_alpha),
            offset: iced::Vector::new(0.0, 4.0 + 4.0 * hover_progress),
            blur_radius: shadow_blur,
        },
        ..Default::default()
    }
}

/// Placeholder style with hover effect
fn placeholder_style(hover_progress: f32) -> iced::widget::container::Style {
    let bg_brightness = 0.12 + 0.03 * hover_progress;

    iced::widget::container::Style {
        background: Some(iced::Background::Color(Color::from_rgb(
            bg_brightness,
            bg_brightness,
            bg_brightness,
        ))),
        border: iced::Border {
            radius: COVER_RADIUS.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.2 + 0.1 * hover_progress),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        ..Default::default()
    }
}

/// Play button style
pub fn play_button_style(
    opacity: f32,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let bg_alpha = match status {
        iced::widget::button::Status::Hovered => 1.0 * opacity,
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
