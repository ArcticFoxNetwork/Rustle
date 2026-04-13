//! User detail page.

use std::cell::RefCell;
use std::rc::Rc;

use iced::widget::{Space, button, column, container, image, row, text};
use iced::{Alignment, Color, Element, Fill, Length, Padding};

use crate::app::{ContentWidthTarget, Message};
use crate::i18n::Locale;
use crate::ui::pages::playlist::PlaylistView;
use crate::ui::theme::BOLD_WEIGHT;
use crate::ui::{theme, widgets};

pub fn view<'a>(
    user: &PlaylistView,
    _song_animations: &'a crate::ui::animation::HoverAnimations<i64>,
    _icon_animations: &crate::ui::animation::HoverAnimations<crate::app::IconId>,
    _search_animation: &crate::ui::animation::SingleHoverAnimation,
    _search_expanded: bool,
    _search_query: &str,
    _liked_songs: std::collections::HashSet<u64>,
    _locale: Locale,
    _scroll_state: Rc<RefCell<widgets::VirtualListState>>,
    _current_user_id: Option<u64>,
    _current_playing_id: Option<i64>,
    content_width: f32,
) -> Element<'a, Message> {
    let header = build_header(user);
    let body = build_playlist_grid(user, content_width);

    let palette = user.palette.clone();
    let primary = palette.primary;
    let gradient_section = container(header).width(Fill).style(move |theme| {
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

    column![gradient_section, body]
        .spacing(0)
        .width(Fill)
        .into()
}

fn build_header(user: &PlaylistView) -> Element<'static, Message> {
    let avatar_size = 216.0;
    let avatar = circular_avatar(user.cover_path.as_deref(), &user.name, avatar_size);

    let title = text(user.name.clone())
        .size(theme::TEXT_SIZE_DISPLAY_LARGE.min(84.0))
        .style(|theme| iced::widget::text::Style {
            color: Some(theme::text_primary(theme)),
        })
        .font(iced::Font::DEFAULT.weight(BOLD_WEIGHT));

    let stats = text(
        user.profile_stats
            .clone()
            .unwrap_or_else(|| "关注 0 · 粉丝 0".to_string()),
    )
    .size(theme::TEXT_SIZE_TITLE)
    .style(|theme| iced::widget::text::Style {
        color: Some(theme::text_secondary(theme)),
    });

    let intro = text(
        user.description
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

fn build_playlist_grid<'a>(user: &PlaylistView, content_width: f32) -> Element<'a, Message> {
    if user.user_playlists.is_empty() {
        return container(
            text("暂无歌单")
                .size(theme::TEXT_SIZE_BODY_LARGE)
                .style(|theme| iced::widget::text::Style {
                    color: Some(theme::text_secondary(theme)),
                }),
        )
        .padding(48)
        .width(Fill)
        .into();
    }

    let card_width = 208.0;
    let card_spacing = 20.0;
    let columns_per_row =
        widgets::calculate_grid_columns_clamped(content_width, card_width, card_spacing, 8);

    let title = text("歌单")
        .size(theme::TEXT_SIZE_TITLE_LARGE)
        .style(|theme| iced::widget::text::Style {
            color: Some(theme::text_primary(theme)),
        })
        .font(iced::Font::DEFAULT.weight(BOLD_WEIGHT));

    let mut rows = column![title, Space::new().height(20)]
        .spacing(18)
        .padding(Padding::new(40.0).left(48.0).right(48.0));

    for chunk in user.user_playlists.chunks(columns_per_row) {
        let mut row_items: Vec<Element<'a, Message>> = Vec::new();
        for playlist in chunk {
            row_items.push(build_playlist_card(playlist, card_width));
            row_items.push(Space::new().width(card_spacing).into());
        }
        if !row_items.is_empty() {
            row_items.pop();
        }
        rows = rows.push(row(row_items).align_y(Alignment::Start));
    }

    widgets::measured_scrollable(
        column![rows, Space::new().height(32)],
        "playlist_scroll",
        |size| Message::ContentWidthResized(ContentWidthTarget::PlaylistDetail, size),
    )
}

fn build_playlist_card<'a>(
    playlist: &crate::api::SongList,
    card_width: f32,
) -> Element<'a, Message> {
    let local_cover = resolve_playlist_cover_path(playlist);

    let cover: Element<'a, Message> = if let Some(path) = local_cover {
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
            text(playlist.name.clone())
                .size(theme::TEXT_SIZE_BODY_LARGE)
                .style(|theme| iced::widget::text::Style {
                    color: Some(theme::text_primary(theme)),
                }),
            text(playlist.author.clone())
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
    .on_press(Message::OpenNcmPlaylist(playlist.id))
    .into()
}

fn resolve_playlist_cover_path(playlist: &crate::api::SongList) -> Option<String> {
    if !playlist.cover_img_url.is_empty()
        && !playlist.cover_img_url.starts_with("http")
        && std::path::Path::new(&playlist.cover_img_url).exists()
    {
        return Some(playlist.cover_img_url.clone());
    }

    let covers_dir = crate::utils::covers_cache_dir();
    let stem = format!("playlist_{}", playlist.id);
    crate::utils::find_cached_image(&covers_dir, &stem).map(|p| p.to_string_lossy().to_string())
}

fn circular_avatar(
    path: Option<&str>,
    fallback_name: &str,
    size: f32,
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
            .style(|theme| iced::widget::text::Style {
                color: Some(theme::text_primary(theme)),
            })
            .font(iced::Font::DEFAULT.weight(BOLD_WEIGHT)),
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
