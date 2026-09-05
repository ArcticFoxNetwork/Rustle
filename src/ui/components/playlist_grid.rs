//! Responsive playlist grid and single-row layouts.

use iced::widget::{Space, column, row};
use iced::{Element, Fill};

use crate::api::PlaylistSummary;
use crate::app::{ImageState, Message};
use crate::image::ImageKind;
use crate::ui::animation::HoverAnimations;
use crate::ui::responsive::{ResponsiveContext, playlist_card_metrics};
use crate::ui::widgets::{self, playlist_card};

fn card<'a>(
    playlist: &'a PlaylistSummary,
    image_state: &'a ImageState,
    animations: &'a HoverAnimations<u64>,
    context: ResponsiveContext,
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
        context,
    )
}

/// Render one complete responsive row using the shared card token policy.
pub fn view_single_row<'a>(
    playlists: &'a [PlaylistSummary],
    image_state: &'a ImageState,
    animations: &'a HoverAnimations<u64>,
    context: ResponsiveContext,
) -> Element<'a, Message> {
    let metrics = playlist_card_metrics(context);
    widgets::responsive_card_columns(metrics, usize::MAX, move |columns| {
        let cards = playlists
            .iter()
            .take(columns)
            .map(|playlist| card(playlist, image_state, animations, context))
            .collect::<Vec<_>>();

        if cards.is_empty() {
            Space::new().width(Fill).height(metrics.height).into()
        } else {
            row(cards).spacing(metrics.gap).into()
        }
    })
}

/// Render a complete responsive wrapping grid for full-list views.
pub fn view<'a>(
    playlists: &'a [PlaylistSummary],
    image_state: &'a ImageState,
    animations: &'a HoverAnimations<u64>,
    max_items: Option<usize>,
    context: ResponsiveContext,
) -> Element<'a, Message> {
    let metrics = playlist_card_metrics(context);
    let items = playlists
        .iter()
        .take(max_items.unwrap_or(usize::MAX))
        .collect::<Vec<_>>();
    if items.is_empty() {
        return Space::new().width(Fill).height(metrics.height).into();
    }

    widgets::responsive_card_columns(metrics, usize::MAX, move |columns| {
        let rows = items
            .chunks(columns)
            .map(|chunk| {
                let cards = chunk
                    .iter()
                    .map(|playlist| card(playlist, image_state, animations, context))
                    .collect::<Vec<_>>();
                row(cards).spacing(metrics.gap).into()
            })
            .collect::<Vec<Element<'a, Message>>>();

        column(rows).spacing(metrics.gap).into()
    })
}

#[cfg(test)]
mod tests {
    use crate::ui::responsive::{
        ResponsiveContext, calculate_grid_columns_clamped, playlist_card_metrics,
    };
    use iced::Size;

    fn visible_column_count(available_width: f32, context: ResponsiveContext) -> usize {
        let metrics = playlist_card_metrics(context);
        calculate_grid_columns_clamped(available_width, metrics.width, metrics.gap, usize::MAX)
    }

    #[test]
    fn responsive_row_never_reports_zero_columns() {
        let context = ResponsiveContext::from_viewport(Size::new(1_920.0, 1_080.0));
        assert!(visible_column_count(0.0, context) >= 1);
        assert!(visible_column_count(160.0, context) >= 1);
    }

    #[test]
    fn contextual_grid_uses_complete_cards_for_validation_viewports() {
        let fixtures = [
            (Size::new(1_920.0, 1_080.0), 1_650.0, 8),
            (Size::new(2_560.0, 1_440.0), 2_093.0, 8),
            (Size::new(2_558.0, 1_398.0), 2_093.0, 8),
            (Size::new(960.0, 1_080.0), 828.0, 5),
            (Size::new(1_280.0, 1_440.0), 1_104.0, 5),
            (Size::new(1_280.0, 720.0), 969.0, 5),
            (Size::new(768.0, 1_024.0), 642.0, 3),
            (Size::new(720.0, 800.0), 601.0, 3),
            (Size::new(960.0, 540.0), 841.0, 5),
            (Size::new(560.0, 800.0), 502.0, 3),
        ];

        for (viewport, available_width, expected_columns) in fixtures {
            let context = ResponsiveContext::from_viewport(viewport);
            assert_eq!(
                visible_column_count(available_width, context),
                expected_columns,
                "unexpected complete playlist-card columns for {viewport:?}"
            );
        }
    }
}
