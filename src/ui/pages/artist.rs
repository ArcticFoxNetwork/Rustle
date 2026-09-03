//! Artist detail page.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Color, Element, Fill, Length, Padding};

use crate::app::{ContentWidthTarget, ImageState, Message};
use crate::i18n::{Key, Locale};
use crate::image::ImageKind;
use crate::ui::animation::SmoothScrollTarget;
use crate::ui::components::{
    cover_image, detail_card, detail_description,
    playlist_view::{self, PlaylistColumns},
};
use crate::ui::pages::playlist::{self, ArtistPageTab, DetailGradientSnapshot, PlaylistView};
use crate::ui::responsive::{LayoutProfile, ResponsiveContext, TextRole, detail_grid_columns};
use crate::ui::theme::BOLD_WEIGHT;
use crate::ui::{theme, widgets};

pub fn view<'a>(
    artist: &'a PlaylistView,
    image_state: &'a ImageState,
    song_animations: &'a crate::ui::animation::HoverAnimations<i64>,
    icon_animations: &'a crate::ui::animation::HoverAnimations<crate::app::IconId>,
    search_animation: &'a crate::ui::animation::SingleHoverAnimation,
    search_expanded: bool,
    search_query: &'a str,
    liked_songs: Option<&'a HashSet<u64>>,
    locale: Locale,
    scroll_state: Rc<RefCell<widgets::VirtualListState>>,
    current_user_id: Option<u64>,
    current_playing_id: Option<i64>,
    content_width: f32,
    description_expanded: bool,
    gradient_source: Option<DetailGradientSnapshot>,
    gradient_progress: f32,
    context: ResponsiveContext,
) -> Element<'a, Message> {
    view_for_context(
        artist,
        image_state,
        song_animations,
        icon_animations,
        search_animation,
        search_expanded,
        search_query,
        liked_songs,
        locale,
        scroll_state,
        current_user_id,
        current_playing_id,
        content_width,
        description_expanded,
        gradient_source,
        gradient_progress,
        context,
    )
}

fn view_for_context<'a>(
    artist: &'a PlaylistView,
    image_state: &'a ImageState,
    song_animations: &'a crate::ui::animation::HoverAnimations<i64>,
    icon_animations: &'a crate::ui::animation::HoverAnimations<crate::app::IconId>,
    search_animation: &'a crate::ui::animation::SingleHoverAnimation,
    search_expanded: bool,
    search_query: &'a str,
    liked_songs: Option<&'a HashSet<u64>>,
    locale: Locale,
    scroll_state: Rc<RefCell<widgets::VirtualListState>>,
    current_user_id: Option<u64>,
    current_playing_id: Option<i64>,
    content_width: f32,
    description_expanded: bool,
    gradient_source: Option<DetailGradientSnapshot>,
    gradient_progress: f32,
    context: ResponsiveContext,
) -> Element<'a, Message> {
    let header = build_header(artist, image_state, description_expanded, locale, context);
    let controls = playlist::build_controls(
        artist,
        icon_animations,
        search_animation,
        search_expanded,
        search_query,
        locale,
        current_user_id,
        context,
    );

    let tabs = build_tabs(artist.artist_tab, locale, context);
    let header_and_controls = column![header, controls, tabs].spacing(0).width(Fill);

    let gradient_target = artist.gradient_snapshot();
    let gradient_section = container(header_and_controls)
        .width(Fill)
        .style(move |theme| {
            playlist::detail_gradient_style(
                theme,
                gradient_source,
                gradient_target,
                gradient_progress,
            )
        });

    let body: Element<'a, Message> = match artist.artist_tab {
        ArtistPageTab::TopSongs => {
            let filtered_indices = playlist_view::filter_song_indices(&artist.songs, search_query);
            let columns = PlaylistColumns::online().for_context(context);
            let song_list_header = playlist_view::build_header(locale, columns, context);
            let song_list = playlist_view::build_list(
                &artist.songs,
                filtered_indices,
                image_state,
                song_animations,
                liked_songs,
                columns,
                scroll_state,
                current_playing_id,
                context,
            );

            column![song_list_header, song_list]
                .spacing(0)
                .width(Fill)
                .into()
        }
        ArtistPageTab::Albums => {
            build_albums_view(artist, image_state, locale, content_width, context)
        }
    };

    column![gradient_section, body]
        .spacing(0)
        .width(Fill)
        .into()
}

fn build_header(
    artist: &PlaylistView,
    image_state: &ImageState,
    description_expanded: bool,
    locale: Locale,
    context: ResponsiveContext,
) -> Element<'static, Message> {
    let tokens = context.tokens;
    let avatar_size = match context.profile {
        LayoutProfile::Expanded | LayoutProfile::Standard => tokens.size(216.0),
        LayoutProfile::Compact => tokens.size(192.0),
        LayoutProfile::Tablet => tokens.size(176.0),
        LayoutProfile::Narrow => tokens.size(152.0),
    };
    let avatar_handle = artist
        .owner_artist_id
        .and_then(|id| image_state.get(ImageKind::ArtistCover, id));
    let avatar = circular_avatar(
        avatar_handle,
        &artist.name,
        avatar_size,
        theme::TEXT_PRIMARY,
    );

    let title = text(artist.name.clone())
        .size(tokens.text(TextRole::DisplayLarge).min(tokens.size(84.0)))
        .style(|theme| iced::widget::text::Style {
            color: Some(theme::text_primary(theme)),
        })
        .font(iced::Font::DEFAULT.weight(BOLD_WEIGHT));

    let stats_text = artist
        .profile_stats
        .clone()
        .unwrap_or_else(|| artist.like_count.clone())
        .trim()
        .to_string();
    let stats = text(if stats_text.is_empty() {
        locale.get(Key::ArtistPopularWorks).to_string()
    } else {
        stats_text
    })
    .size(tokens.text(TextRole::Title))
    .style(|theme| iced::widget::text::Style {
        color: Some(theme::text_secondary(theme)),
    });

    let intro: Element<'static, Message> = {
        let desc_text = artist
            .description
            .clone()
            .filter(|text| !text.trim().is_empty())
            .unwrap_or_else(|| locale.get(Key::ProfileNoBio).to_string());
        let has_description = artist
            .description
            .as_ref()
            .is_some_and(|d| !d.trim().is_empty());

        let line_count = desc_text.lines().count()
            + desc_text
                .lines()
                .map(|l| l.chars().count().saturating_sub(1) / 55)
                .sum::<usize>();
        let is_long = line_count > 2;

        let desc_widget = text(desc_text)
            .size(tokens.text(TextRole::BodyLarge))
            .style(|theme| iced::widget::text::Style {
                color: Some(theme::text_muted(theme)),
            })
            .wrapping(iced::widget::text::Wrapping::WordOrGlyph);

        if has_description && is_long {
            if description_expanded {
                let scrollable_desc = crate::ui::widgets::smooth_scroll(
                    scrollable(
                        container(desc_widget)
                            .width(Fill)
                            .padding(Padding::new(tokens.space(4.0)).left(0.0)),
                    )
                    .direction(scrollable::Direction::Vertical(
                        iced::widget::scrollable::Scrollbar::new()
                            .width(4)
                            .scroller_width(4),
                    ))
                    .height(tokens.size(150.0))
                    .id(iced::widget::Id::new("artist_description_scroll")),
                    SmoothScrollTarget::Native("artist_description_scroll"),
                    Message::SmoothScroll,
                );

                let collapse_btn = detail_description::toggle_button(
                    locale.get(Key::CollapseDescription),
                    Message::ToggleDescriptionExpand,
                );

                column![scrollable_desc, collapse_btn]
                    .spacing(tokens.space(2.0))
                    .width(Fill)
                    .into()
            } else {
                let clamped_desc = container(desc_widget)
                    .height(detail_description::collapsed_height())
                    .clip(true)
                    .width(Fill);

                let expand_btn = detail_description::toggle_button(
                    locale.get(Key::ExpandDescription),
                    Message::ToggleDescriptionExpand,
                );

                column![clamped_desc, expand_btn]
                    .spacing(tokens.space(2.0))
                    .width(Fill)
                    .into()
            }
        } else {
            container(desc_widget)
                .width(detail_description::text_width())
                .into()
        }
    };

    let info = column![
        title,
        Space::new().height(10),
        stats,
        Space::new().height(12),
        intro,
    ]
    .align_x(Alignment::Start)
    .width(Fill);

    if matches!(
        context.profile,
        LayoutProfile::Expanded | LayoutProfile::Standard | LayoutProfile::Compact
    ) {
        row![avatar, Space::new().width(tokens.space(28.0)), info]
            .align_y(Alignment::Center)
            .padding(
                Padding::new(if context.profile == LayoutProfile::Compact {
                    tokens.space(28.0)
                } else {
                    tokens.space(48.0)
                })
                .top(if context.profile == LayoutProfile::Compact {
                    tokens.space(48.0)
                } else {
                    tokens.space(84.0)
                })
                .bottom(tokens.space(28.0)),
            )
            .into()
    } else {
        column![avatar, Space::new().height(tokens.space(20.0)), info]
            .align_x(Alignment::Start)
            .width(Fill)
            .padding(
                Padding::new(tokens.space(24.0))
                    .top(tokens.space(48.0))
                    .bottom(tokens.space(24.0)),
            )
            .into()
    }
}

fn build_tabs(
    active_tab: ArtistPageTab,
    locale: Locale,
    context: ResponsiveContext,
) -> Element<'static, Message> {
    let tokens = context.tokens;
    let tab = |label: &'static str, tab: ArtistPageTab| {
        let active = active_tab == tab;
        button(
            text(label)
                .size(tokens.text(TextRole::BodyLarge))
                .style(move |theme| iced::widget::text::Style {
                    color: Some(if active {
                        theme::text_primary(theme)
                    } else {
                        theme::text_secondary(theme)
                    }),
                }),
        )
        .padding(Padding::new(tokens.space(10.0)).left(0.0).right(0.0))
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

    scrollable(
        container(
            row![
                tab(locale.get(Key::ArtistTopSongs), ArtistPageTab::TopSongs),
                Space::new().width(tokens.space(28.0)),
                tab(locale.get(Key::ArtistAlbums), ArtistPageTab::Albums),
            ]
            .align_y(Alignment::Center),
        )
        .padding(
            Padding::new(0.0)
                .left(tokens.space(48.0))
                .right(tokens.space(48.0))
                .bottom(tokens.space(18.0)),
        ),
    )
    .direction(iced::widget::scrollable::Direction::Horizontal(
        iced::widget::scrollable::Scrollbar::new()
            .width(0)
            .scroller_width(0),
    ))
    .id(iced::widget::Id::new("artist_tabs_scroll"))
    .width(Fill)
    .into()
}

fn build_albums_view<'a>(
    artist: &PlaylistView,
    image_state: &'a ImageState,
    locale: Locale,
    content_width: f32,
    context: ResponsiveContext,
) -> Element<'a, Message> {
    let tokens = context.tokens;
    if artist.artist_albums.is_empty() {
        return container(
            text(locale.get(Key::ArtistNoAlbums).to_string())
                .size(tokens.text(TextRole::BodyLarge))
                .style(|theme| iced::widget::text::Style {
                    color: Some(theme::text_secondary(theme)),
                }),
        )
        .width(Fill)
        .padding(Padding::new(tokens.space(40.0)))
        .into();
    }

    let card_spacing = context.tokens.space(detail_card::CARD_SPACING);
    let columns_per_row = detail_grid_columns(content_width, context);
    let mut rows = column![].spacing(tokens.space(18.0)).padding(
        Padding::new(0.0)
            .left(tokens.space(48.0))
            .right(tokens.space(48.0)),
    );

    for chunk in artist.artist_albums.chunks(columns_per_row) {
        let mut row_items: Vec<Element<'a, Message>> = Vec::new();
        for album in chunk {
            let cover_handle = image_state.get(ImageKind::AlbumCover, album.id);
            row_items.push(detail_card::view_with_context(
                album.name.clone(),
                album.artist_names(),
                cover_handle,
                ImageKind::AlbumCover,
                Message::OpenAlbum(album.id),
                context,
            ));
            row_items.push(Space::new().width(card_spacing).into());
        }
        if !row_items.is_empty() {
            row_items.pop();
        }
        rows = rows.push(row(row_items).align_y(Alignment::Start));
    }

    widgets::measured_scrollable(
        column![rows, Space::new().height(tokens.space(32.0))],
        "playlist_scroll",
        |size| Message::ContentWidthResized(ContentWidthTarget::PlaylistDetail, size),
        Message::SmoothScroll,
    )
}

fn circular_avatar(
    handle: Option<&iced::widget::image::Handle>,
    fallback_name: &str,
    size: f32,
    fallback_text_color: Color,
) -> Element<'static, Message> {
    if handle.is_some() {
        return cover_image::circle(handle, ImageKind::ArtistCover, size);
    }

    let initial = fallback_name.chars().next().unwrap_or('?').to_string();
    container(
        text(initial)
            .size((size * 0.22).max(24.0))
            .style(move |_theme| iced::widget::text::Style {
                color: Some(fallback_text_color),
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
