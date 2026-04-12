//! Artist detail page.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use iced::widget::{Space, button, column, container, image, row, scrollable, text};
use iced::{Alignment, Color, Element, Fill, Length, Padding};

use crate::app::Message;
use crate::i18n::Locale;
use crate::ui::components::playlist_view::{self, PlaylistColumns};
use crate::ui::pages::playlist::{self, ArtistPageTab, PlaylistView};
use crate::ui::theme::BOLD_WEIGHT;
use crate::ui::{theme, widgets};

pub fn view<'a>(
    artist: &PlaylistView,
    artist_album_covers: &'a std::collections::HashMap<u64, iced::widget::image::Handle>,
    song_animations: &'a crate::ui::animation::HoverAnimations<i64>,
    icon_animations: &crate::ui::animation::HoverAnimations<crate::app::IconId>,
    search_animation: &crate::ui::animation::SingleHoverAnimation,
    search_expanded: bool,
    search_query: &str,
    liked_songs: HashSet<u64>,
    locale: Locale,
    scroll_state: Rc<RefCell<widgets::VirtualListState>>,
    current_user_id: Option<u64>,
    current_playing_id: Option<i64>,
    content_width: f32,
) -> Element<'a, Message> {
    let header = build_header(artist);
    let controls = playlist::build_controls(
        artist,
        icon_animations,
        search_animation,
        search_expanded,
        search_query,
        locale,
        current_user_id,
    );

    let tabs = build_tabs(artist.artist_tab);
    let header_and_controls = column![header, controls, tabs].spacing(0).width(Fill);

    let palette = artist.palette.clone();
    let primary = palette.primary;
    let gradient_section = container(header_and_controls)
        .width(Fill)
        .style(move |theme| {
            let bottom_color = theme::background(theme);
            iced::widget::container::Style {
                background: Some(iced::Background::Gradient(iced::Gradient::Linear(
                    iced::gradient::Linear::new(iced::Radians(std::f32::consts::PI))
                        .add_stop(
                            0.0,
                            Color::from_rgb(
                                (primary.r * 1.08 + 0.04).min(1.0),
                                (primary.g * 1.06 + 0.03).min(1.0),
                                (primary.b * 1.08 + 0.04).min(1.0),
                            ),
                        )
                        .add_stop(
                            0.58,
                            Color::from_rgb(
                                primary.r * 0.58 + bottom_color.r * 0.42,
                                primary.g * 0.58 + bottom_color.g * 0.42,
                                primary.b * 0.58 + bottom_color.b * 0.42,
                            ),
                        )
                        .add_stop(1.0, bottom_color),
                ))),
                ..Default::default()
            }
        });

    let body: Element<'a, Message> = match artist.artist_tab {
        ArtistPageTab::TopSongs => {
            let filtered_songs = playlist_view::filter_songs(&artist.songs, search_query);
            let columns = PlaylistColumns::online();
            let song_list_header = playlist_view::build_header(locale, columns);
            let song_list = playlist_view::build_list(
                filtered_songs,
                song_animations,
                liked_songs,
                columns,
                scroll_state,
                current_playing_id,
            );

            column![song_list_header, song_list]
                .spacing(0)
                .width(Fill)
                .into()
        }
        ArtistPageTab::Albums => build_albums_view(artist, artist_album_covers, content_width),
    };

    column![gradient_section, body]
        .spacing(0)
        .width(Fill)
        .into()
}

fn build_header(artist: &PlaylistView) -> Element<'static, Message> {
    let avatar_size = 216.0;
    let avatar = circular_avatar(
        artist.cover_path.as_deref(),
        &artist.name,
        avatar_size,
        theme::TEXT_PRIMARY,
    );

    let title = text(artist.name.clone())
        .size(theme::TEXT_SIZE_DISPLAY_LARGE.min(84.0))
        .style(|theme| iced::widget::text::Style {
            color: Some(theme::text_primary(theme)),
        })
        .font(iced::Font {
            weight: BOLD_WEIGHT,
            ..Default::default()
        });

    let stats_text = artist
        .profile_stats
        .clone()
        .unwrap_or_else(|| artist.like_count.clone())
        .trim()
        .to_string();
    let stats = text(if stats_text.is_empty() {
        "热门作品".to_string()
    } else {
        stats_text
    })
    .size(theme::TEXT_SIZE_TITLE)
    .style(|theme| iced::widget::text::Style {
        color: Some(theme::text_secondary(theme)),
    });

    let intro = text(
        artist
            .description
            .clone()
            .filter(|text| !text.trim().is_empty())
            .unwrap_or_else(|| "暂无简介".to_string()),
    )
    .size(theme::TEXT_SIZE_BODY_LARGE)
    .style(|theme| iced::widget::text::Style {
        color: Some(theme::text_muted(theme)),
    })
    .wrapping(iced::widget::text::Wrapping::WordOrGlyph);

    let info = column![
        title,
        Space::new().height(10),
        stats,
        Space::new().height(12),
        container(intro).max_width(720),
    ]
    .align_x(Alignment::Start)
    .width(Fill);

    row![avatar, Space::new().width(28), info]
        .align_y(Alignment::Center)
        .padding(Padding::new(48.0).top(84.0).bottom(28.0))
        .into()
}

fn build_tabs(active_tab: ArtistPageTab) -> Element<'static, Message> {
    let tab = |label: &'static str, tab: ArtistPageTab| {
        let active = active_tab == tab;
        button(
            text(label)
                .size(theme::TEXT_SIZE_BODY_LARGE)
                .style(move |theme| iced::widget::text::Style {
                    color: Some(if active {
                        theme::text_primary(theme)
                    } else {
                        theme::text_secondary(theme)
                    }),
                }),
        )
        .padding(Padding::new(10.0).left(0.0).right(0.0))
        .style(move |theme, _status| button::Style {
            background: Some(iced::Background::Color(Color::TRANSPARENT)),
            border: iced::Border {
                width: 0.0,
                radius: 0.0.into(),
                color: Color::TRANSPARENT,
            },
            text_color: if active {
                theme::text_primary(theme)
            } else {
                theme::text_secondary(theme)
            },
            ..Default::default()
        })
        .on_press(Message::SwitchArtistTab(tab))
    };

    container(
        row![
            tab("热门单曲", ArtistPageTab::TopSongs),
            Space::new().width(28),
            tab("专辑", ArtistPageTab::Albums),
        ]
        .align_y(Alignment::Center),
    )
    .padding(Padding::new(0.0).left(48.0).right(48.0).bottom(18.0))
    .into()
}

fn build_albums_view<'a>(
    artist: &PlaylistView,
    artist_album_covers: &'a std::collections::HashMap<u64, iced::widget::image::Handle>,
    content_width: f32,
) -> Element<'a, Message> {
    if artist.artist_albums.is_empty() {
        return container(
            text("暂无专辑数据")
                .size(theme::TEXT_SIZE_BODY_LARGE)
                .style(|theme| iced::widget::text::Style {
                    color: Some(theme::text_secondary(theme)),
                }),
        )
        .width(Fill)
        .padding(Padding::new(40.0))
        .into();
    }

    let card_width = 208.0;
    let card_spacing = 20.0;
    let columns_per_row = calculate_grid_columns(content_width, card_width, card_spacing);
    let mut rows = column![]
        .spacing(18)
        .padding(Padding::new(0.0).left(48.0).right(48.0));

    for chunk in artist.artist_albums.chunks(columns_per_row) {
        let mut row_items: Vec<Element<'a, Message>> = Vec::new();
        for album in chunk {
            row_items.push(build_album_card(
                album,
                artist_album_covers.get(&album.id),
                card_width,
            ));
            row_items.push(Space::new().width(card_spacing).into());
        }
        if !row_items.is_empty() {
            row_items.pop();
        }
        rows = rows.push(row(row_items).align_y(Alignment::Start));
    }

    scrollable(column![rows, Space::new().height(32)])
        .width(Fill)
        .height(Fill)
        .into()
}

fn build_album_card<'a>(
    album: &crate::api::SongList,
    cover_handle: Option<&'a iced::widget::image::Handle>,
    card_width: f32,
) -> Element<'a, Message> {
    let local_cover = resolve_album_cover_path(album);

    let cover: Element<'a, Message> = if let Some(handle) = cover_handle {
        container(
            image(handle.clone())
                .width(card_width)
                .height(card_width)
                .content_fit(iced::ContentFit::Cover)
                .border_radius(16.0),
        )
        .width(card_width)
        .height(card_width)
        .style(|theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::surface_container(theme))),
            border: iced::Border {
                radius: 16.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
    } else if let Some(path) = local_cover {
        container(
            image(image::Handle::from_path(path))
                .width(card_width)
                .height(card_width)
                .content_fit(iced::ContentFit::Cover)
                .border_radius(16.0),
        )
        .width(card_width)
        .height(card_width)
        .style(|theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::surface_container(theme))),
            border: iced::Border {
                radius: 16.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
    } else {
        container(Space::new().width(card_width).height(card_width))
            .width(card_width)
            .height(card_width)
            .style(|theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(theme::surface_container(theme))),
                border: iced::Border {
                    radius: 16.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    };

    button(
        column![
            cover,
            Space::new().height(10),
            text(album.name.clone())
                .size(theme::TEXT_SIZE_BODY_LARGE)
                .style(|theme| iced::widget::text::Style {
                    color: Some(theme::text_primary(theme)),
                }),
            text(album.author.clone())
                .size(theme::TEXT_SIZE_BODY)
                .style(|theme| iced::widget::text::Style {
                    color: Some(theme::text_secondary(theme)),
                }),
        ]
        .width(card_width),
    )
    .padding(0)
    .style(|_theme, _status| button::Style {
        background: Some(iced::Background::Color(Color::TRANSPARENT)),
        ..Default::default()
    })
    .on_press(Message::OpenAlbum(album.id))
    .into()
}

fn calculate_grid_columns(content_width: f32, card_width: f32, spacing: f32) -> usize {
    let horizontal_padding = 96.0;
    let usable_width = (content_width - horizontal_padding).max(card_width);
    let columns = ((usable_width + spacing) / (card_width + spacing)).floor() as usize;
    columns.clamp(1, 8)
}

fn resolve_album_cover_path(album: &crate::api::SongList) -> Option<String> {
    if !album.cover_img_url.is_empty()
        && !album.cover_img_url.starts_with("http")
        && std::path::Path::new(&album.cover_img_url).exists()
    {
        return Some(album.cover_img_url.clone());
    }

    let covers_dir = crate::utils::covers_cache_dir();
    let stem = format!("search_album_{}", album.id);
    crate::utils::find_cached_image(&covers_dir, &stem).map(|p| p.to_string_lossy().to_string())
}

fn circular_avatar(
    path: Option<&str>,
    fallback_name: &str,
    size: f32,
    fallback_text_color: Color,
) -> Element<'static, Message> {
    if let Some(path) = path.filter(|p| !p.starts_with("http") && std::path::Path::new(p).exists())
    {
        return container(
            image(image::Handle::from_path(path))
                .width(size)
                .height(size)
                .content_fit(iced::ContentFit::Cover)
                .border_radius(size / 2.0),
        )
        .width(size)
        .height(size)
        .style(move |theme| iced::widget::container::Style {
            border: iced::Border {
                radius: (size / 2.0).into(),
                width: 1.0,
                color: theme::border_color(theme),
            },
            shadow: iced::Shadow {
                color: theme::shadow_color(theme),
                offset: iced::Vector::new(0.0, 10.0),
                blur_radius: 28.0,
            },
            ..Default::default()
        })
        .into();
    }

    let initial = fallback_name.chars().next().unwrap_or('?').to_string();
    container(
        text(initial)
            .size(48)
            .style(move |_theme| iced::widget::text::Style {
                color: Some(fallback_text_color),
            })
            .font(iced::Font {
                weight: BOLD_WEIGHT,
                ..Default::default()
            }),
    )
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .center_x(Length::Fixed(size))
    .center_y(Length::Fixed(size))
    .style(move |theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(theme::surface_container(theme))),
        border: iced::Border {
            radius: (size / 2.0).into(),
            width: 1.0,
            color: theme::border_color(theme),
        },
        ..Default::default()
    })
    .into()
}
