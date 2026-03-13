//! Search results page
//!
//! Displays search results for songs, artists, albums, and playlists
//! with tabbed navigation and pagination.

use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Alignment, Element, Fill, Length, Padding};

use crate::app::{Message, SearchPageState, SearchTab};
use crate::i18n::Locale;
use crate::ui::theme;

use crate::ui::primitives::virtual_list::{VirtualList, VirtualListState};

/// Page size for pagination
const PAGE_SIZE: u32 = 50;
const SONG_ROW_HEIGHT: f32 = 64.0;

/// Build the search results page view
pub fn view<'a>(state: &'a SearchPageState, locale: Locale) -> Element<'a, Message> {
    if state.keyword.is_empty() {
        return empty_search_state(locale);
    }

    // Fixed header section (Title + Tabs)
    let header_section = column![
        // Header with keyword
        row![
            text(&state.keyword)
                .size(28)
                .color(theme::TEXT_PRIMARY)
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                }),
            text(" 的相关搜索").size(28).color(theme::TEXT_MUTED),
        ]
        .align_y(Alignment::Center),
        Space::new().height(24),
        // Search tabs
        search_tabs(state.active_tab),
        Space::new().height(24),
    ]
    .padding(Padding::new(32.0).top(80.0).bottom(0.0));

    // Content area
    let content: Element<'a, Message> = if state.loading {
        loading_state()
    } else {
        match state.active_tab {
            SearchTab::Songs => {
                if state.songs.is_empty() {
                    empty_results_state(&state.keyword)
                } else {
                    // Use VirtualList for high performance song list
                    let song_count = state.songs.len();
                    // Clone references needed for closure
                    // Wrap songs in Rc to share between closures cheaply
                    let songs = std::rc::Rc::new(state.songs.clone());
                    let songs_for_builder = songs.clone();
                    let songs_for_hover = songs.clone();
                    
                    let song_animations = state.song_animations.clone();
                    let current_page = state.current_page;

                    // Table header
                    let table_header = search_table_header();

                    // Virtual List
                    let virtual_list = VirtualList::new(song_count, SONG_ROW_HEIGHT, move |index| {
                        if index >= songs_for_builder.len() {
                            return Space::new().into();
                        }

                        let song = &songs_for_builder[index];
                        let hover_progress = song_animations.get_progress(&song.id);
                        let song_clone = song.clone();

                        // Calculate absolute index for display
                        let index_num = current_page * PAGE_SIZE + index as u32 + 1;

                        // Format duration
                        let duration_secs = song.duration / 1000;
                        let duration_str =
                            format!("{}:{:02}", duration_secs / 60, duration_secs % 60);

                        let song_row = button(
                            row![
                                text(format!("{:02}", index_num))
                                    .size(13)
                                    .color(theme::TEXT_MUTED)
                                    .width(40),
                                column![text(song.name.clone()).size(14).color(theme::animated_text(
                                    &iced::Theme::Dark,
                                    hover_progress
                                )),]
                                .width(Fill),
                                text(song.singer.clone())
                                    .size(13)
                                    .color(theme::TEXT_SECONDARY)
                                    .width(Length::FillPortion(2)),
                                text(song.album.clone())
                                    .size(13)
                                    .color(theme::TEXT_MUTED)
                                    .width(Length::FillPortion(2)),
                                text(duration_str)
                                    .size(13)
                                    .color(theme::TEXT_MUTED)
                                    .width(60),
                            ]
                            .spacing(12)
                            .align_y(Alignment::Center)
                            .padding(Padding::new(10.0).left(12.0).right(12.0)),
                        )
                        .style(move |theme, status| {
                            song_row_style(theme, status, hover_progress)
                        })
                        .on_press(Message::PlaySearchSong(song_clone))
                        .width(Fill);

                        Element::from(song_row)
                    })
                    .state(state.scroll_state.clone())
                    .on_item_hover(move |index| {
                        if index < songs_for_hover.len() {
                            Message::HoverSearchSong(Some(songs_for_hover[index].id))
                        } else {
                            Message::HoverSearchSong(None)
                        }
                    })
                    .on_empty_area(Message::HoverSearchSong(None))
                    .height(Length::Fill);

                    // Combine table header, list, and pagination
                    let list_section = column![
                        table_header,
                        Space::new().height(8),
                        container(virtual_list).height(Fill).width(Fill),
                    ]
                    .padding(Padding::new(32.0).top(0.0));

                    if state.total_count > PAGE_SIZE {
                        column![
                            list_section.height(Fill),
                            Space::new().height(16),
                            pagination(state),
                            Space::new().height(32),
                        ]
                        .height(Fill)
                        .into()
                    } else {
                        column![
                            list_section.height(Fill),
                            Space::new().height(32),
                        ]
                        .height(Fill)
                        .into()
                    }
                }
            }
            SearchTab::Albums | SearchTab::Artists => {
                let content = if state.albums.is_empty() {
                    empty_results_state(&state.keyword)
                } else {
                    let grid = grid_results(state, SearchTab::Albums);
                    let mut col = column![grid];
                    
                    if state.total_count > PAGE_SIZE {
                        col = col.push(Space::new().height(24)).push(pagination(state));
                    }
                    col = col.push(Space::new().height(40));
                    
                    col.padding(Padding::new(32.0).top(0.0)).into()
                };
                
                // For grid views, we use a Scrollable since grid items don't virtualize well yet
                scrollable(content)
                    .width(Fill)
                    .height(Fill)
                    .id(iced::widget::Id::new("search_scroll"))
                    .style(theme::dark_scrollable)
                    .into()
            }
            SearchTab::Playlists => {
                let content = if state.playlists.is_empty() {
                    empty_results_state(&state.keyword)
                } else {
                    let grid = grid_results(state, SearchTab::Playlists);
                    let mut col = column![grid];
                    
                    if state.total_count > PAGE_SIZE {
                        col = col.push(Space::new().height(24)).push(pagination(state));
                    }
                    col = col.push(Space::new().height(40));
                    
                    col.padding(Padding::new(32.0).top(0.0)).into()
                };

                scrollable(content)
                    .width(Fill)
                    .height(Fill)
                    .id(iced::widget::Id::new("search_scroll"))
                    .style(theme::dark_scrollable)
                    .into()
            }
        }
    };

    // Main layout: Header (fixed) + Content (flexible)
    container(
        column![
            header_section,
            content
        ]
        .width(Fill)
        .height(Fill)
    )
    .width(Fill)
    .height(Fill)
    .style(theme::main_content)
    .into()
}

/// Search tabs component
fn search_tabs(active_tab: SearchTab) -> Element<'static, Message> {
    let tabs = [
        (SearchTab::Songs, "单曲"),
        (SearchTab::Artists, "歌手"),
        (SearchTab::Albums, "专辑"),
        (SearchTab::Playlists, "歌单"),
    ];

    let tab_buttons: Vec<Element<'static, Message>> = tabs
        .iter()
        .map(|(tab, label)| {
            let is_active = active_tab == *tab;
            let tab_clone = *tab;

            button(text(*label).size(14).color(if is_active {
                theme::TEXT_PRIMARY
            } else {
                theme::TEXT_MUTED
            }))
            .padding(Padding::new(8.0).left(16.0).right(16.0))
            .style(move |theme, status| tab_button_style(theme, status, is_active))
            .on_press(Message::SearchTabChanged(tab_clone))
            .into()
        })
        .collect();

    container(row(tab_buttons).spacing(4))
        .padding(4)
        .style(tabs_container_style)
        .into()
}

/// Tab button style
fn tab_button_style(
    _theme: &iced::Theme,
    status: button::Status,
    is_active: bool,
) -> button::Style {
    let bg = if is_active {
        iced::Background::Color(theme::ACCENT_PINK)
    } else {
        match status {
            button::Status::Hovered => {
                iced::Background::Color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.1))
            }
            _ => iced::Background::Color(iced::Color::TRANSPARENT),
        }
    };

    button::Style {
        background: Some(bg),
        text_color: if is_active {
            iced::Color::WHITE
        } else {
            theme::TEXT_MUTED
        },
        border: iced::Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Tabs container style
fn tabs_container_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            1.0, 1.0, 1.0, 0.05,
        ))),
        border: iced::Border {
            radius: 12.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Search table header
fn search_table_header() -> Element<'static, Message> {
    row![
        text("#").size(12).color(theme::TEXT_MUTED).width(40),
        text("标题").size(12).color(theme::TEXT_MUTED).width(Fill),
        text("歌手")
            .size(12)
            .color(theme::TEXT_MUTED)
            .width(Length::FillPortion(2)),
        text("专辑")
            .size(12)
            .color(theme::TEXT_MUTED)
            .width(Length::FillPortion(2)),
        text("时长").size(12).color(theme::TEXT_MUTED).width(60),
    ]
    .spacing(12)
    .padding(Padding::new(8.0).left(12.0).right(12.0))
    .into()
}

/// Song row style with hover animation
fn song_row_style(
    _theme: &iced::Theme,
    status: button::Status,
    hover_progress: f32,
) -> button::Style {
    let bg_alpha = match status {
        button::Status::Hovered | button::Status::Pressed => 0.08 + 0.04 * hover_progress,
        _ => 0.04 * hover_progress,
    };

    button::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            1.0, 1.0, 1.0, bg_alpha,
        ))),
        text_color: theme::TEXT_PRIMARY,
        border: iced::Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Grid view for albums and playlists
fn grid_results<'a>(state: &'a SearchPageState, tab: SearchTab) -> Element<'a, Message> {
    let items = match tab {
        SearchTab::Albums | SearchTab::Artists => &state.albums,
        SearchTab::Playlists => &state.playlists,
        _ => return Space::new().into(),
    };

    const CARD_WIDTH: f32 = 160.0;
    const CARD_SPACING: f32 = 24.0;
    const ROW_SPACING: f32 = 32.0;

    // Calculate columns (assume ~900px content width)
    let columns = 5usize;

    let mut rows: Vec<Element<'a, Message>> = Vec::new();

    for chunk in items.chunks(columns) {
        let mut row_items: Vec<Element<'a, Message>> = Vec::new();

        for item in chunk {
            let hover_progress = state.card_animations.get_progress(&item.id);
            let item_id = item.id;
            let item_tab = tab;

            let card = grid_card(item, hover_progress, item_id, item_tab);
            row_items.push(card);

            if row_items.len() < columns * 2 - 1 {
                row_items.push(Space::new().width(CARD_SPACING).into());
            }
        }

        // Fill remaining space
        let items_in_row = chunk.len();
        if items_in_row < columns {
            for _ in items_in_row..columns {
                row_items.push(Space::new().width(CARD_SPACING).into());
                row_items.push(Space::new().width(CARD_WIDTH).into());
            }
        }

        rows.push(row(row_items).into());
        rows.push(Space::new().height(ROW_SPACING).into());
    }

    column(rows).into()
}

/// Grid card for album/playlist
fn grid_card<'a>(
    item: &'a crate::api::SongList,
    hover_progress: f32,
    item_id: u64,
    tab: SearchTab,
) -> Element<'a, Message> {
    const CARD_WIDTH: f32 = 160.0;

    // Cover placeholder
    let cover = container(Space::new())
        .width(CARD_WIDTH)
        .height(CARD_WIDTH)
        .style(move |_theme| cover_placeholder_style(hover_progress));

    let card_content = column![
        cover,
        Space::new().height(8),
        text(&item.name)
            .size(14)
            .color(theme::TEXT_PRIMARY)
            .width(CARD_WIDTH),
        text(&item.author)
            .size(12)
            .color(theme::TEXT_MUTED)
            .width(CARD_WIDTH),
    ]
    .width(CARD_WIDTH);

    let card_btn = button(card_content)
        .padding(0)
        .style(|_theme, _status| button::Style {
            background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
            ..Default::default()
        })
        .on_press(Message::OpenSearchResult(item_id, tab));

    iced::widget::mouse_area(card_btn)
        .on_enter(Message::HoverSearchCard(Some(item_id)))
        .on_exit(Message::HoverSearchCard(None))
        .into()
}

/// Cover placeholder style
fn cover_placeholder_style(hover_progress: f32) -> container::Style {
    let shadow_blur = 8.0 + 8.0 * hover_progress;
    let shadow_alpha = 0.2 + 0.2 * hover_progress;
    let scale_offset = -2.0 * hover_progress;

    container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgb(
            0.15, 0.15, 0.15,
        ))),
        border: iced::Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: iced::Color::from_rgba(0.0, 0.0, 0.0, shadow_alpha),
            offset: iced::Vector::new(0.0, 4.0 + scale_offset),
            blur_radius: shadow_blur,
        },
        ..Default::default()
    }
}

/// Pagination component
fn pagination<'a>(state: &'a SearchPageState) -> Element<'a, Message> {
    let total_pages = (state.total_count + PAGE_SIZE - 1) / PAGE_SIZE;
    let current_page = state.current_page;

    let mut items: Vec<Element<'a, Message>> = Vec::new();

    // Previous button
    let prev_btn = button(text("上一页").size(13))
        .padding(Padding::new(8.0).left(16.0).right(16.0))
        .style(theme::secondary_button)
        .on_press_maybe(if current_page > 0 {
            Some(Message::SearchPageChanged(current_page - 1))
        } else {
            None
        });
    items.push(prev_btn.into());

    // Page info
    items.push(
        text(format!("{} / {}", current_page + 1, total_pages))
            .size(14)
            .color(theme::TEXT_SECONDARY)
            .into(),
    );

    // Next button
    let next_btn = button(text("下一页").size(13))
        .padding(Padding::new(8.0).left(16.0).right(16.0))
        .style(theme::secondary_button)
        .on_press_maybe(if current_page + 1 < total_pages {
            Some(Message::SearchPageChanged(current_page + 1))
        } else {
            None
        });
    items.push(next_btn.into());

    container(row(items).spacing(16).align_y(Alignment::Center))
        .width(Fill)
        .align_x(Alignment::Center)
        .into()
}

/// Loading state
fn loading_state<'a>() -> Element<'a, Message> {
    container(text("搜索中...").size(16).color(theme::TEXT_MUTED))
        .width(Fill)
        .height(200)
        .center_x(Fill)
        .center_y(200)
        .into()
}

/// Empty search state (no keyword entered)
fn empty_search_state<'a>(_locale: Locale) -> Element<'a, Message> {
    container(
        column![
            text("🔍").size(48),
            Space::new().height(16),
            text("输入关键词开始搜索").size(16).color(theme::TEXT_MUTED),
        ]
        .align_x(Alignment::Center),
    )
    .width(Fill)
    .height(Fill)
    .center_x(Fill)
    .center_y(Fill)
    .style(theme::main_content)
    .into()
}

/// Empty results state
fn empty_results_state<'a>(keyword: &str) -> Element<'a, Message> {
    container(
        column![
            text("🔍").size(48),
            Space::new().height(16),
            text(format!("未找到 \"{}\" 的相关结果", keyword))
                .size(16)
                .color(theme::TEXT_MUTED),
        ]
        .align_x(Alignment::Center),
    )
    .width(Fill)
    .height(200)
    .center_x(Fill)
    .center_y(200)
    .into()
}
