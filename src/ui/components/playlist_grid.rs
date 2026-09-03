//! Responsive playlist grid and single-row layouts.

use iced::widget::{Space, column, row};
use iced::{Element, Fill};

use crate::api::PlaylistSummary;
use crate::app::{ImageState, Message};
use crate::image::ImageKind;
use crate::ui::animation::HoverAnimations;
use crate::ui::responsive::{CardRole, ResponsiveContext, complete_card_grid_columns};
use crate::ui::widgets::{self, playlist_card};

const CARD_SPACING: f32 = 24.0;
const ROW_SPACING: f32 = 32.0;
const GRID_HORIZONTAL_PADDING: f32 = 64.0;

pub fn visible_column_count(container_width: f32) -> usize {
    widgets::calculate_grid_columns(container_width, playlist_card::CARD_WIDTH, CARD_SPACING)
}

/// Return the number of complete token-scaled playlist cards that fit.
///
/// Full grids and single-row sections deliberately share this entry point so
/// changing view mode cannot change the column policy.
pub fn visible_column_count_with_context(
    container_width: f32,
    context: ResponsiveContext,
) -> usize {
    complete_card_grid_columns(
        container_width,
        context,
        CardRole::Playlist,
        GRID_HORIZONTAL_PADDING,
        usize::MAX,
    )
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
    let columns = visible_column_count_with_context(container_width, context);
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

    let columns = visible_column_count_with_context(container_width, context);
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
    use super::{visible_column_count, visible_column_count_with_context};
    use crate::ui::responsive::{MIN_USABLE_CONTENT_WIDTH, ResponsiveContext};
    use iced::Size;

    #[test]
    fn responsive_row_never_reports_zero_columns() {
        assert_eq!(visible_column_count(0.0), 1);
        assert_eq!(visible_column_count(160.0), 1);
        assert!(visible_column_count(900.0) >= 4);
        assert_eq!(visible_column_count(1136.0), 6);
    }

    #[test]
    fn contextual_grid_uses_complete_cards_for_validation_viewports() {
        let fixtures = [
            (Size::new(1_920.0, 1_080.0), 8),
            (Size::new(2_560.0, 1_440.0), 8),
            (Size::new(960.0, 1_080.0), 5),
            (Size::new(768.0, 1_024.0), 4),
            (Size::new(720.0, 800.0), 3),
            (Size::new(960.0, 540.0), 5),
            (Size::new(560.0, 800.0), 3),
        ];

        for (viewport, expected_columns) in fixtures {
            let context = ResponsiveContext::from_viewport(viewport);
            assert_eq!(
                visible_column_count_with_context(MIN_USABLE_CONTENT_WIDTH, context),
                expected_columns,
                "unexpected complete playlist-card columns for {viewport:?}"
            );
        }
    }
}
