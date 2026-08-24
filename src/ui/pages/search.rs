//! Search results page
//!
//! Displays search results for songs, artists, albums, and playlists
//! with tabbed navigation and pagination.

use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Background, Border, Element, Fill, Length, Padding};

use crate::app::{ContentWidthTarget, ImageState, Message, SearchPageState, SearchTab};
use crate::i18n::{Key, Locale};
use crate::image::ImageKind;
use crate::ui::components::cover_image;
use crate::ui::theme::BOLD_WEIGHT;
use crate::ui::{theme, widgets};

use crate::ui::primitives::virtual_list::VirtualList;

/// Page size for pagination
const PAGE_SIZE: u32 = 50;
const SONG_ROW_HEIGHT: f32 = 64.0;

/// Build the search results page view
pub fn view<'a>(
    state: &'a SearchPageState,
    image_state: &'a ImageState,
    locale: Locale,
) -> Element<'a, Message> {
    if state.keyword.is_empty() {
        return empty_search_state(locale);
    }

    // Fixed header section (Title + Tabs), matching the settings page header.
    let header_section = container(column![
        row![
            text(&state.keyword)
                .size(theme::TEXT_SIZE_HERO)
                .style(|theme| iced::widget::text::Style {
                    color: Some(theme::text_primary(theme)),
                })
                .font(iced::Font::DEFAULT.weight(BOLD_WEIGHT)),
            text(format!(" {}", locale.get(Key::SearchRelated)))
                .size(theme::TEXT_SIZE_TITLE_LARGE)
                .style(|theme| iced::widget::text::Style {
                    color: Some(theme::text_muted(theme)),
                }),
        ]
        .align_y(Alignment::Center),
        Space::new().height(24),
        search_tabs(state.active_tab, locale),
    ])
    .width(Fill)
    .padding(
        Padding::new(40.0)
            .top(70.0)
            .right(32.0)
            .bottom(20.0)
            .left(32.0),
    )
    .style(|theme| container::Style {
        background: Some(Background::Color(theme::background(theme))),
        ..Default::default()
    });

    // Content area
    let content: Element<'a, Message> = if state.loading {
        loading_state()
    } else {
        match state.active_tab {
            SearchTab::Songs => {
                if state.tracks.is_empty() {
                    empty_results_state(&state.keyword)
                } else {
                    // Use VirtualList for high performance song list
                    let song_count = state.tracks.len();
                    let songs = &state.tracks;
                    let song_animations = &state.song_animations;
                    let current_page = state.current_page;

                    let table_header = search_table_header();

                    let virtual_list =
                        VirtualList::new(song_count, SONG_ROW_HEIGHT, move |index| {
                            if index >= songs.len() {
                                return Space::new().into();
                            }

                            let song = &songs[index];
                            let hover_progress = song_animations.get_progress(&song.id);
                            let index_num = current_page * PAGE_SIZE + index as u32 + 1;
                            let duration_secs = song.duration_ms / 1000;
                            let duration_str =
                                format!("{}:{:02}", duration_secs / 60, duration_secs % 60);

                            let song_row = button(
                                row![
                                    text(format!("{:02}", index_num))
                                        .size(theme::TEXT_SIZE_LABEL)
                                        .style(|theme| iced::widget::text::Style {
                                            color: Some(theme::text_muted(theme)),
                                        })
                                        .width(40),
                                    column![
                                        text(song.title.as_str())
                                            .size(theme::TEXT_SIZE_BODY)
                                            .style(move |theme| {
                                                iced::widget::text::Style {
                                                    color: Some(theme::animated_text(
                                                        theme,
                                                        hover_progress,
                                                    )),
                                                }
                                            }),
                                    ]
                                    .width(Fill),
                                    text(song.artist_names())
                                        .size(theme::TEXT_SIZE_LABEL)
                                        .style(|theme| iced::widget::text::Style {
                                            color: Some(theme::text_secondary(theme)),
                                        })
                                        .width(Length::FillPortion(2)),
                                    text(song.album.name.as_str())
                                        .size(theme::TEXT_SIZE_LABEL)
                                        .style(|theme| iced::widget::text::Style {
                                            color: Some(theme::text_muted(theme)),
                                        })
                                        .width(Length::FillPortion(2)),
                                    text(duration_str)
                                        .size(theme::TEXT_SIZE_LABEL)
                                        .style(|theme| iced::widget::text::Style {
                                            color: Some(theme::text_muted(theme)),
                                        })
                                        .width(60),
                                ]
                                .spacing(12)
                                .align_y(Alignment::Center)
                                .padding(Padding::new(10.0).left(12.0).right(12.0)),
                            )
                            .style(move |theme, status| {
                                song_row_style(theme, status, hover_progress)
                            })
                            .on_press(Message::PlaySearchSong(song.id))
                            .width(Fill);

                            Element::from(song_row)
                        })
                        .keyed_by(move |index| {
                            songs
                                .get(index)
                                .map(|song| (song.id, current_page, index))
                                .unwrap_or((0, current_page, index))
                        })
                        .state(state.scroll_state.clone())
                        .on_item_hover(move |index| {
                            if index < songs.len() {
                                Message::HoverSearchSong(Some(songs[index].id))
                            } else {
                                Message::HoverSearchSong(None)
                            }
                        })
                        .on_empty_area(Message::HoverSearchSong(None))
                        .height(Length::Fill);

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
                        column![list_section.height(Fill), Space::new().height(32),]
                            .height(Fill)
                            .into()
                    }
                }
            }
            SearchTab::Albums | SearchTab::Artists => {
                let is_empty = match state.active_tab {
                    SearchTab::Artists => state.artists.is_empty(),
                    _ => state.albums.is_empty(),
                };
                let content = if is_empty {
                    empty_results_state(&state.keyword)
                } else {
                    let grid = grid_results(state, image_state, state.active_tab);
                    let mut col = column![grid];

                    if state.total_count > PAGE_SIZE {
                        col = col.push(Space::new().height(24)).push(pagination(state));
                    }
                    col = col.push(Space::new().height(40));

                    col.padding(Padding::new(32.0).top(0.0)).into()
                };

                widgets::measured_scrollable(content, "search_scroll", |size| {
                    Message::ContentWidthResized(ContentWidthTarget::Search, size)
                })
            }
            SearchTab::Playlists => {
                let content = if state.playlists.is_empty() {
                    empty_results_state(&state.keyword)
                } else {
                    let grid = grid_results(state, image_state, SearchTab::Playlists);
                    let mut col = column![grid];

                    if state.total_count > PAGE_SIZE {
                        col = col.push(Space::new().height(24)).push(pagination(state));
                    }
                    col = col.push(Space::new().height(40));

                    col.padding(Padding::new(32.0).top(0.0)).into()
                };

                widgets::measured_scrollable(content, "search_scroll", |size| {
                    Message::ContentWidthResized(ContentWidthTarget::Search, size)
                })
            }
        }
    };

    container(column![header_section, content].width(Fill).height(Fill))
        .width(Fill)
        .height(Fill)
        .style(theme::main_content)
        .into()
}

/// Search tabs component
fn search_tabs(active_tab: SearchTab, locale: Locale) -> Element<'static, Message> {
    let tabs = [
        (SearchTab::Songs, Key::SearchTabSongs),
        (SearchTab::Artists, Key::SearchTabArtists),
        (SearchTab::Albums, Key::SearchTabAlbums),
        (SearchTab::Playlists, Key::SearchTabPlaylists),
    ];

    let tab_buttons: Vec<Element<'static, Message>> = tabs
        .iter()
        .map(|(tab, label_key)| {
            let is_active = active_tab == *tab;
            let tab_clone = *tab;

            let tab_button = button(
                container(
                    text(locale.get(*label_key).to_string())
                        .size(theme::TEXT_SIZE_BODY)
                        .style(move |theme| iced::widget::text::Style {
                            color: Some(if is_active {
                                theme::ACCENT_PINK
                            } else {
                                theme::settings_inactive_tab(theme)
                            }),
                        }),
                )
                .width(Fill)
                .center_x(Fill),
            )
            .style(move |theme, status| {
                let hover_bg = match status {
                    button::Status::Hovered => {
                        Some(Background::Color(theme::hover_bg_alpha(theme, 0.05)))
                    }
                    _ => None,
                };
                button::Style {
                    background: hover_bg,
                    text_color: theme::text_primary(theme),
                    border: Border::default(),
                    ..Default::default()
                }
            })
            .on_press(Message::SearchTabChanged(tab_clone))
            .padding([12, 0])
            .width(Fill);

            let underline = container(Space::new().height(2))
                .width(Fill)
                .style(move |theme| container::Style {
                    background: Some(Background::Color(if is_active {
                        theme::ACCENT_PINK
                    } else {
                        theme::settings_inactive_underline(theme)
                    })),
                    ..Default::default()
                });

            container(column![tab_button, underline].spacing(0).width(Fill))
                .width(100)
                .into()
        })
        .collect();

    scrollable(row(tab_buttons).spacing(0))
        .direction(iced::widget::scrollable::Direction::Horizontal(
            iced::widget::scrollable::Scrollbar::new()
                .width(0)
                .scroller_width(0),
        ))
        .width(Fill)
        .into()
}

/// Search table header
fn search_table_header() -> Element<'static, Message> {
    row![
        text("#")
            .size(theme::TEXT_SIZE_CAPTION)
            .style(|theme| iced::widget::text::Style {
                color: Some(theme::text_muted(theme)),
            })
            .width(40),
        text("标题")
            .size(theme::TEXT_SIZE_CAPTION)
            .style(|theme| iced::widget::text::Style {
                color: Some(theme::text_muted(theme)),
            })
            .width(Fill),
        text("歌手")
            .size(theme::TEXT_SIZE_CAPTION)
            .style(|theme| iced::widget::text::Style {
                color: Some(theme::text_muted(theme)),
            })
            .width(Length::FillPortion(2)),
        text("专辑")
            .size(theme::TEXT_SIZE_CAPTION)
            .style(|theme| iced::widget::text::Style {
                color: Some(theme::text_muted(theme)),
            })
            .width(Length::FillPortion(2)),
        text("时长")
            .size(theme::TEXT_SIZE_CAPTION)
            .style(|theme| iced::widget::text::Style {
                color: Some(theme::text_muted(theme)),
            })
            .width(60),
    ]
    .spacing(12)
    .padding(Padding::new(8.0).left(12.0).right(12.0))
    .into()
}

/// Song row style with hover animation
fn song_row_style(
    theme: &iced::Theme,
    status: button::Status,
    hover_progress: f32,
) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => {
            theme::hover_bg_alpha(theme, 0.08 + 0.04 * hover_progress)
        }
        _ => theme::hover_bg_alpha(theme, 0.04 * hover_progress),
    };

    button::Style {
        background: Some(iced::Background::Color(bg)),
        text_color: theme::text_primary(theme),
        border: iced::Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Grid view for albums and playlists
fn grid_results<'a>(
    state: &'a SearchPageState,
    image_state: &'a ImageState,
    tab: SearchTab,
) -> Element<'a, Message> {
    let items: Vec<GridItemRef<'a>> = match tab {
        SearchTab::Albums => state.albums.iter().map(GridItemRef::Album).collect(),
        SearchTab::Artists => state.artists.iter().map(GridItemRef::Artist).collect(),
        SearchTab::Playlists => state.playlists.iter().map(GridItemRef::Playlist).collect(),
        _ => return Space::new().into(),
    };
    let kind = search_image_kind(tab);

    const CARD_WIDTH: f32 = 160.0;
    const CARD_SPACING: f32 = 24.0;
    const ROW_SPACING: f32 = 32.0;

    let columns = widgets::calculate_grid_columns(state.content_width, CARD_WIDTH, CARD_SPACING);

    let mut rows: Vec<Element<'a, Message>> = Vec::new();

    for chunk in items.chunks(columns) {
        let mut row_items: Vec<Element<'a, Message>> = Vec::new();

        for item in chunk {
            let hover_progress = state.card_animations.get_progress(&item.id());
            let item_id = item.id();
            let item_tab = tab;

            let cover_handle = image_state.get(kind, item_id);
            let card = grid_card(*item, cover_handle, kind, hover_progress, item_id, item_tab);
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
    item: GridItemRef<'a>,
    cover_handle: Option<&'a iced::widget::image::Handle>,
    kind: ImageKind,
    hover_progress: f32,
    item_id: u64,
    tab: SearchTab,
) -> Element<'a, Message> {
    const CARD_WIDTH: f32 = 160.0;
    let has_cover = cover_handle.is_some();

    let cover: Element<'a, Message> =
        container(cover_image::custom(cover_handle, kind, CARD_WIDTH, 8.0))
            .width(CARD_WIDTH)
            .height(CARD_WIDTH)
            .style(move |theme| {
                if has_cover {
                    cover_card_style(theme, hover_progress)
                } else {
                    cover_placeholder_style(theme, hover_progress)
                }
            })
            .into();

    let card_content = column![
        cover,
        Space::new().height(8),
        text(item.name())
            .size(theme::TEXT_SIZE_BODY)
            .style(|theme| iced::widget::text::Style {
                color: Some(theme::text_primary(theme)),
            })
            .width(CARD_WIDTH),
        text(item.subtitle())
            .size(theme::TEXT_SIZE_CAPTION)
            .style(|theme| iced::widget::text::Style {
                color: Some(theme::text_muted(theme)),
            })
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

#[derive(Clone, Copy)]
enum GridItemRef<'a> {
    Album(&'a crate::api::AlbumSummary),
    Artist(&'a crate::api::ArtistSummary),
    Playlist(&'a crate::api::PlaylistSummary),
}

impl<'a> GridItemRef<'a> {
    fn id(self) -> u64 {
        match self {
            Self::Album(item) => item.id,
            Self::Artist(item) => item.id,
            Self::Playlist(item) => item.id,
        }
    }

    fn name(self) -> &'a str {
        match self {
            Self::Album(item) => &item.name,
            Self::Artist(item) => &item.name,
            Self::Playlist(item) => &item.name,
        }
    }

    fn subtitle(self) -> String {
        match self {
            Self::Album(item) => item.artist_names(),
            Self::Artist(_) => "歌手".to_string(),
            Self::Playlist(item) => item.creator.nickname.clone(),
        }
    }
}

fn search_image_kind(tab: SearchTab) -> ImageKind {
    match tab {
        SearchTab::Artists => ImageKind::ArtistCover,
        SearchTab::Albums => ImageKind::AlbumCover,
        SearchTab::Playlists => ImageKind::PlaylistCover,
        SearchTab::Songs => ImageKind::SongCover,
    }
}

/// Cover placeholder style
fn cover_placeholder_style(theme: &iced::Theme, hover_progress: f32) -> container::Style {
    cover_base_style(
        theme,
        hover_progress,
        iced::Background::Color(theme::surface(theme)),
    )
}

fn cover_card_style(theme: &iced::Theme, hover_progress: f32) -> container::Style {
    cover_base_style(
        theme,
        hover_progress,
        iced::Background::Color(iced::Color::TRANSPARENT),
    )
}

fn cover_base_style(
    theme: &iced::Theme,
    hover_progress: f32,
    background: iced::Background,
) -> container::Style {
    let shadow_blur = 8.0 + 8.0 * hover_progress;
    let shadow_alpha = if theme::is_dark_theme(theme) {
        0.2 + 0.2 * hover_progress
    } else {
        0.08 + 0.08 * hover_progress
    };
    let scale_offset = -2.0 * hover_progress;

    container::Style {
        background: Some(background),
        border: iced::Border {
            radius: 8.0.into(),
            width: 1.0,
            color: theme::border_color(theme),
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
    let total_pages = state.total_count.div_ceil(PAGE_SIZE);
    let current_page = state.current_page;

    let mut items: Vec<Element<'a, Message>> = Vec::new();

    // Previous button
    let prev_btn = button(text("上一页").size(theme::TEXT_SIZE_LABEL))
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
            .size(theme::TEXT_SIZE_BODY)
            .style(|theme| iced::widget::text::Style {
                color: Some(theme::text_secondary(theme)),
            })
            .into(),
    );

    // Next button
    let next_btn = button(text("下一页").size(theme::TEXT_SIZE_LABEL))
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
    container(
        text("搜索中...")
            .size(theme::TEXT_SIZE_BODY_LARGE)
            .style(|theme| iced::widget::text::Style {
                color: Some(theme::text_muted(theme)),
            }),
    )
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
            text("🔍").size(theme::TEXT_SIZE_DISPLAY),
            Space::new().height(16),
            text("输入关键词开始搜索")
                .size(theme::TEXT_SIZE_BODY_LARGE)
                .style(|theme| iced::widget::text::Style {
                    color: Some(theme::text_muted(theme)),
                }),
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
            text("🔍").size(theme::TEXT_SIZE_DISPLAY),
            Space::new().height(16),
            text(format!("未找到 \"{}\" 的相关结果", keyword))
                .size(theme::TEXT_SIZE_BODY_LARGE)
                .style(|theme| iced::widget::text::Style {
                    color: Some(theme::text_muted(theme)),
                }),
        ]
        .align_x(Alignment::Center),
    )
    .width(Fill)
    .height(200)
    .center_x(Fill)
    .center_y(200)
    .into()
}
