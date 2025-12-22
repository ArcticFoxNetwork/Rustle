//! Discover page - Browse recommended and hot playlists
//!
//! Displays personalized recommendations (for logged-in users) and
//! hot playlists in a modern grid layout.

use iced::widget::{Space, button, column, container, scrollable, text};
use iced::{Element, Fill, Padding};

use crate::app::{DiscoverPageState, DiscoverViewMode, Message};
use crate::i18n::{Key, Locale};
use crate::ui::components::playlist_grid;
use crate::ui::theme;
use crate::ui::widgets::section_header;

/// Build the discover page view
pub fn view<'a>(
    state: &'a DiscoverPageState,
    locale: Locale,
    is_logged_in: bool,
) -> Element<'a, Message> {
    match state.view_mode {
        DiscoverViewMode::Overview => view_overview(state, locale, is_logged_in),
        DiscoverViewMode::AllRecommended => view_all_recommended(state, locale),
        DiscoverViewMode::AllHot => view_all_hot(state, locale),
    }
}

/// Overview view showing both sections with limited items
fn view_overview<'a>(
    state: &'a DiscoverPageState,
    locale: Locale,
    is_logged_in: bool,
) -> Element<'a, Message> {
    let mut content_items: Vec<Element<'a, Message>> = Vec::new();
    let content_width = state.content_width;

    // Recommended playlists section (only for logged-in users)
    if is_logged_in && !state.recommended_playlists.is_empty() {
        content_items.push(section_header::view(
            locale.get(Key::DiscoverRecommended),
            locale.get(Key::DiscoverSeeAll),
            Some(Message::SeeAllRecommended),
        ));
        content_items.push(Space::new().height(16).into());
        content_items.push(playlist_grid::view(
            &state.recommended_playlists,
            &state.playlist_covers,
            &state.card_animations,
            Some(10),
            content_width,
        ));
        content_items.push(Space::new().height(40).into());
    }

    // Hot playlists section (for all users)
    if !state.hot_playlists.is_empty() {
        content_items.push(section_header::view(
            locale.get(Key::DiscoverHot),
            locale.get(Key::DiscoverSeeAll),
            Some(Message::SeeAllHot),
        ));
        content_items.push(Space::new().height(16).into());
        content_items.push(playlist_grid::view(
            &state.hot_playlists,
            &state.playlist_covers,
            &state.card_animations,
            Some(15),
            content_width,
        ));
        content_items.push(Space::new().height(40).into());
    }

    // Empty state if no playlists
    if content_items.is_empty() {
        content_items.push(Space::new().height(100).into());
    }

    // Top padding accounts for global navigation bar height (~48px)
    let content = column(content_items).padding(Padding::new(32.0).top(56.0));

    let scrollable_content = scrollable(content)
        .width(Fill)
        .height(Fill)
        .id(iced::widget::Id::new("discover_scroll"))
        .style(theme::dark_scrollable);

    container(scrollable_content)
        .width(Fill)
        .height(Fill)
        .style(theme::main_content)
        .into()
}

/// Full view of all recommended playlists
fn view_all_recommended<'a>(state: &'a DiscoverPageState, locale: Locale) -> Element<'a, Message> {
    let content_width = state.content_width;

    // Section title only - use global navigation for back
    let header = text(locale.get(Key::DiscoverRecommended)).size(24);

    // Top padding accounts for global navigation bar height (~48px)
    let content = column![
        header,
        Space::new().height(24),
        playlist_grid::view(
            &state.recommended_playlists,
            &state.playlist_covers,
            &state.card_animations,
            None, // Show all
            content_width,
        ),
        Space::new().height(40),
    ]
    .padding(Padding::new(32.0).top(56.0));

    let scrollable_content = scrollable(content)
        .width(Fill)
        .height(Fill)
        .id(iced::widget::Id::new("discover_scroll"))
        .style(theme::dark_scrollable);

    container(scrollable_content)
        .width(Fill)
        .height(Fill)
        .style(theme::main_content)
        .into()
}

/// Full view of all hot playlists with infinite scroll
fn view_all_hot<'a>(state: &'a DiscoverPageState, locale: Locale) -> Element<'a, Message> {
    let content_width = state.content_width;

    // Section title only - use global navigation for back
    let header = text(locale.get(Key::DiscoverHot)).size(24);

    let mut content_items: Vec<Element<'a, Message>> = vec![
        header.into(),
        Space::new().height(24).into(),
        playlist_grid::view(
            &state.hot_playlists,
            &state.playlist_covers,
            &state.card_animations,
            None, // Show all
            content_width,
        ),
    ];

    // Load more button if there are more playlists
    if state.hot_has_more {
        let load_more_btn = button(
            text(if state.hot_loading {
                "加载中..."
            } else {
                "加载更多"
            })
            .size(14),
        )
        .padding(Padding::new(12.0).left(24.0).right(24.0))
        .style(theme::secondary_button)
        .on_press_maybe(if state.hot_loading {
            None
        } else {
            Some(Message::LoadMoreHotPlaylists)
        });

        content_items.push(Space::new().height(24).into());
        content_items.push(
            container(load_more_btn)
                .width(Fill)
                .align_x(iced::Alignment::Center)
                .into(),
        );
    }

    content_items.push(Space::new().height(40).into());

    // Top padding accounts for global navigation bar height (~48px)
    let content = column(content_items).padding(Padding::new(32.0).top(56.0));

    let scrollable_content = scrollable(content)
        .width(Fill)
        .height(Fill)
        .id(iced::widget::Id::new("discover_scroll"))
        .style(theme::dark_scrollable);

    container(scrollable_content)
        .width(Fill)
        .height(Fill)
        .style(theme::main_content)
        .into()
}
