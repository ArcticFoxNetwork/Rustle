//! Playlist grid component for discover page
//!
//! Displays playlists in a responsive grid layout.

use std::collections::HashMap;

use iced::widget::{Space, column, image, row};
use iced::{Element, Fill};

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
    // Formula: columns = floor((container_width + spacing) / (card_width + spacing))
    let columns = ((container_width + CARD_SPACING) / (CARD_WIDTH + CARD_SPACING)).floor() as usize;
    columns.max(1) // At least 1 column
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
            let cover_handle = covers.get(&playlist.id);
            let hover_progress = animations.get_progress(&playlist.id);

            let card = playlist_card::view(
                &playlist.name,
                &playlist.author,
                cover_handle,
                hover_progress,
                Message::OpenNcmPlaylist(playlist.id),
                Message::PlayDiscoverPlaylist(playlist.id),
                Message::HoverDiscoverPlaylist(Some(playlist.id)),
                Message::HoverDiscoverPlaylist(None),
            );

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
