//! Playlist grid component for discover page
//!
//! Displays playlists in a responsive grid layout.

use std::collections::HashMap;

use iced::widget::{Space, column, container, image, row, text};
use iced::{Color, Element, Fill};

use crate::api::SongList;
use crate::app::Message;
use crate::ui::animation::HoverAnimations;
use crate::ui::widgets::playlist_card;

/// Grid configuration
const CARD_WIDTH: f32 = 160.0;
const CARD_SPACING: f32 = 24.0;
const ROW_SPACING: f32 = 32.0;

/// Calculate number of columns based on container width
fn calculate_columns(container_width: f32) -> usize {
    let columns = ((container_width + CARD_SPACING) / (CARD_WIDTH + CARD_SPACING)).floor() as usize;
    columns.max(1)
}

fn daily_recommend_cover<'a>(hover_progress: f32) -> Element<'a, Message> {
    let day = chrono::Local::now().format("%d").to_string();

    container(text(day).size(56).color(Color::WHITE).font(iced::Font {
        weight: iced::font::Weight::Bold,
        ..Default::default()
    }))
    .width(CARD_WIDTH)
    .height(CARD_WIDTH)
    .center_x(CARD_WIDTH)
    .center_y(CARD_WIDTH)
    .style(move |_theme| daily_recommend_cover_style(hover_progress))
    .into()
}

fn daily_recommend_cover_style(hover_progress: f32) -> iced::widget::container::Style {
    let shadow_blur = 16.0 + 8.0 * hover_progress;
    let shadow_alpha = 0.3 + 0.2 * hover_progress;

    iced::widget::container::Style {
        background: Some(iced::Background::Gradient(iced::Gradient::Linear(
            iced::gradient::Linear::new(std::f32::consts::PI * 0.75)
                .add_stop(0.0, Color::from_rgb(0.95, 0.3, 0.4))
                .add_stop(1.0, Color::from_rgb(0.6, 0.2, 0.5)),
        ))),
        border: iced::Border {
            radius: 8.0.into(),
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

/// Create a playlist grid element
pub fn view<'a>(
    playlists: &'a [SongList],
    covers: &'a HashMap<u64, image::Handle>,
    animations: &'a HoverAnimations<u64>,
    max_items: Option<usize>,
    container_width: f32,
) -> Element<'a, Message> {
    // Limit items if max_items is specified
    let items: Vec<_> = if let Some(max) = max_items {
        playlists.iter().take(max).collect()
    } else {
        playlists.iter().collect()
    };

    if items.is_empty() {
        return Space::new().width(Fill).height(100).into();
    }

    // Calculate number of columns based on container width
    let columns = calculate_columns(container_width);

    // Build rows of cards
    let mut rows: Vec<Element<'a, Message>> = Vec::new();

    for chunk in items.chunks(columns) {
        let mut row_items: Vec<Element<'a, Message>> = Vec::new();

        for playlist in chunk {
            let hover_progress = animations.get_progress(&playlist.id);

            let card = if playlist.id == 0 {
                let cover = daily_recommend_cover(hover_progress);
                playlist_card::view_with_custom_cover(
                    &playlist.name,
                    &playlist.author,
                    cover,
                    hover_progress,
                    Message::OpenNcmPlaylist(0),
                    Message::PlayDiscoverPlaylist(0),
                    Message::HoverDiscoverPlaylist(Some(0)),
                    Message::HoverDiscoverPlaylist(None),
                )
            } else {
                let cover_handle = covers.get(&playlist.id);
                playlist_card::view(
                    &playlist.name,
                    &playlist.author,
                    cover_handle,
                    hover_progress,
                    Message::OpenNcmPlaylist(playlist.id),
                    Message::PlayDiscoverPlaylist(playlist.id),
                    Message::HoverDiscoverPlaylist(Some(playlist.id)),
                    Message::HoverDiscoverPlaylist(None),
                )
            };

            row_items.push(card);

            // Add spacing between cards (except after last)
            if row_items.len() < columns * 2 - 1 {
                row_items.push(Space::new().width(CARD_SPACING).into());
            }
        }

        // Fill remaining space if row is not complete
        let items_in_row = chunk.len();
        if items_in_row < columns {
            for _ in items_in_row..columns {
                row_items.push(Space::new().width(CARD_SPACING).into());
                row_items.push(Space::new().width(CARD_WIDTH).into());
            }
        }

        rows.push(row(row_items).into());
    }

    // Add spacing between rows
    let mut content: Vec<Element<'a, Message>> = Vec::new();
    for (i, row_elem) in rows.into_iter().enumerate() {
        content.push(row_elem);
        if i < items.len() / columns {
            content.push(Space::new().height(ROW_SPACING).into());
        }
    }

    column(content).into()
}
