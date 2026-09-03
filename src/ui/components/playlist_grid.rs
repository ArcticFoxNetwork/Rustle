//! Responsive playlist grid and single-row layouts.

use iced::widget::{Space, column, row};
use iced::{Element, Fill};

use crate::api::PlaylistSummary;
use crate::app::{ImageState, Message};
use crate::image::ImageKind;
use crate::ui::animation::HoverAnimations;
use crate::ui::responsive::{CardRole, ResponsiveContext};
use crate::ui::widgets::{self, playlist_card};

const CARD_SPACING: f32 = 24.0;
const ROW_SPACING: f32 = 32.0;

pub fn visible_column_count(container_width: f32) -> usize {
    widgets::calculate_grid_columns(container_width, playlist_card::CARD_WIDTH, CARD_SPACING)
}

fn card<'a>(
    playlist: &'a PlaylistSummary,
    image_state: &'a ImageState,
    animations: &'a HoverAnimations<u64>,
) -> Element<'a, Message> {
    let hover_progress = animations.get_progress(&playlist.id);
    playlist_card::view(
        &playlist.name,
        image_state.get(ImageKind::PlaylistCover, playlist.id),
        image_state.get_playlist_footer(playlist.id),
        hover_progress,
        Message::OpenNcmPlaylist(playlist.id),
        Message::PlayDiscoverPlaylist(playlist.id),
        Message::HoverDiscoverPlaylist(Some(playlist.id)),
        Message::HoverDiscoverPlaylist(None),
    )
}

fn card_with_context<'a>(
    playlist: &'a PlaylistSummary,
    image_state: &'a ImageState,
    animations: &'a HoverAnimations<u64>,
    context: ResponsiveContext,
) -> Element<'a, Message> {
    let hover_progress = animations.get_progress(&playlist.id);
    playlist_card::view_with_context(
        &playlist.name,
        image_state.get(ImageKind::PlaylistCover, playlist.id),
        image_state.get_playlist_footer(playlist.id),
        hover_progress,
        Message::OpenNcmPlaylist(playlist.id),
        Message::PlayDiscoverPlaylist(playlist.id),
        Message::HoverDiscoverPlaylist(Some(playlist.id)),
        Message::HoverDiscoverPlaylist(None),
        context,
    )
}

/// Render exactly one responsive row, taking only the number of complete cards
/// that fit in the measured content width.
pub fn view_single_row<'a>(
    playlists: &'a [PlaylistSummary],
    image_state: &'a ImageState,
    animations: &'a HoverAnimations<u64>,
    container_width: f32,
) -> Element<'a, Message> {
    let columns = visible_column_count(container_width);
    let cards = playlists
        .iter()
        .take(columns)
        .map(|playlist| card(playlist, image_state, animations))
        .collect::<Vec<_>>();

    if cards.is_empty() {
        Space::new().width(Fill).height(100).into()
    } else {
        row(cards).spacing(CARD_SPACING).into()
    }
}

/// Render one complete responsive row using the shared card token policy.
pub fn view_single_row_with_context<'a>(
    playlists: &'a [PlaylistSummary],
    image_state: &'a ImageState,
    animations: &'a HoverAnimations<u64>,
    container_width: f32,
    context: ResponsiveContext,
) -> Element<'a, Message> {
    let metrics = context.tokens.card(CardRole::Playlist);
    let columns = context
        .grid_columns(container_width, 161.0, 24.0, usize::MAX)
        .max(1);
    let cards = playlists
        .iter()
        .take(columns)
        .map(|playlist| card_with_context(playlist, image_state, animations, context))
        .collect::<Vec<_>>();

    if cards.is_empty() {
        Space::new().width(Fill).height(metrics.height).into()
    } else {
        row(cards).spacing(metrics.gap).into()
    }
}

/// Render a wrapping grid for full-list views.
pub fn view<'a>(
    playlists: &'a [PlaylistSummary],
    image_state: &'a ImageState,
    animations: &'a HoverAnimations<u64>,
    max_items: Option<usize>,
    container_width: f32,
) -> Element<'a, Message> {
    let items = playlists
        .iter()
        .take(max_items.unwrap_or(usize::MAX))
        .collect::<Vec<_>>();
    if items.is_empty() {
        return Space::new().width(Fill).height(100).into();
    }

    let columns = visible_column_count(container_width);
    let rows = items
        .chunks(columns)
        .map(|chunk| {
            let cards = chunk
                .iter()
                .map(|playlist| card(playlist, image_state, animations))
                .collect::<Vec<_>>();
            row(cards).spacing(CARD_SPACING).into()
        })
        .collect::<Vec<Element<'a, Message>>>();

    column(rows).spacing(ROW_SPACING).into()
}

/// Render a complete responsive wrapping grid for full-list views.
pub fn view_with_context<'a>(
    playlists: &'a [PlaylistSummary],
    image_state: &'a ImageState,
    animations: &'a HoverAnimations<u64>,
    max_items: Option<usize>,
    container_width: f32,
    context: ResponsiveContext,
) -> Element<'a, Message> {
    let metrics = context.tokens.card(CardRole::Playlist);
    let items = playlists
        .iter()
        .take(max_items.unwrap_or(usize::MAX))
        .collect::<Vec<_>>();
    if items.is_empty() {
        return Space::new().width(Fill).height(metrics.height).into();
    }

    let columns = match context.profile {
        crate::ui::responsive::LayoutProfile::Tablet
        | crate::ui::responsive::LayoutProfile::Narrow => 1,
        _ => context
            .grid_columns(container_width, 161.0, 24.0, usize::MAX)
            .max(1),
    };
    let rows = items
        .chunks(columns)
        .map(|chunk| {
            let cards = chunk
                .iter()
                .map(|playlist| card_with_context(playlist, image_state, animations, context))
                .collect::<Vec<_>>();
            row(cards).spacing(metrics.gap).into()
        })
        .collect::<Vec<Element<'a, Message>>>();

    column(rows).spacing(metrics.gap).into()
}

#[cfg(test)]
mod tests {
    use super::visible_column_count;

    #[test]
    fn responsive_row_never_reports_zero_columns() {
        assert_eq!(visible_column_count(0.0), 1);
        assert_eq!(visible_column_count(160.0), 1);
        assert!(visible_column_count(900.0) >= 4);
        assert_eq!(visible_column_count(1136.0), 6);
    }
}
