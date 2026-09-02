//! User detail page.

use std::cell::RefCell;
use std::rc::Rc;

use iced::widget::{Space, column, container, row, scrollable, text};
use iced::{Alignment, Element, Fill, Length, Padding};

use crate::app::{ContentWidthTarget, ImageState, Message};
use crate::i18n::{Key, Locale};
use crate::image::ImageKind;
use crate::ui::animation::SmoothScrollTarget;
use crate::ui::components::{cover_image, detail_card, detail_description};
use crate::ui::pages::playlist::{self, DetailGradientSnapshot, PlaylistView};
use crate::ui::theme::BOLD_WEIGHT;
use crate::ui::{theme, widgets};

pub fn view<'a>(
    user: &PlaylistView,
    image_state: &'a ImageState,
    _song_animations: &'a crate::ui::animation::HoverAnimations<i64>,
    _icon_animations: &crate::ui::animation::HoverAnimations<crate::app::IconId>,
    _search_animation: &crate::ui::animation::SingleHoverAnimation,
    _search_expanded: bool,
    _search_query: &str,
    _liked_songs: Option<&std::collections::HashSet<u64>>,
    locale: Locale,
    _scroll_state: Rc<RefCell<widgets::VirtualListState>>,
    _current_user_id: Option<u64>,
    _current_playing_id: Option<i64>,
    content_width: f32,
    description_expanded: bool,
    gradient_source: Option<DetailGradientSnapshot>,
    gradient_progress: f32,
) -> Element<'a, Message> {
    let header = build_header(user, image_state, description_expanded, locale);
    let body = build_playlist_grid(user, image_state, content_width);

    let gradient_target = user.gradient_snapshot();
    let gradient_section = container(header).width(Fill).style(move |theme| {
        playlist::detail_gradient_style(theme, gradient_source, gradient_target, gradient_progress)
    });

    column![gradient_section, body]
        .spacing(0)
        .width(Fill)
        .into()
}

fn build_header(
    user: &PlaylistView,
    image_state: &ImageState,
    description_expanded: bool,
    locale: Locale,
) -> Element<'static, Message> {
    let avatar_size = 216.0;
    let avatar = circular_avatar(
        image_state.get(ImageKind::UserAvatar, user.creator_id),
        &user.name,
        avatar_size,
    );

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

    let intro: Element<'static, Message> = {
        let desc_text = user
            .description
            .clone()
            .filter(|text| !text.trim().is_empty())
            .unwrap_or_else(|| "暂无简介".to_string());
        let has_description = user
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
            .size(theme::TEXT_SIZE_BODY_LARGE)
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
                            .padding(Padding::new(4.0).left(0.0)),
                    )
                    .direction(scrollable::Direction::Vertical(
                        iced::widget::scrollable::Scrollbar::new()
                            .width(4)
                            .scroller_width(4),
                    ))
                    .height(150)
                    .id(iced::widget::Id::new("user_description_scroll")),
                    SmoothScrollTarget::Native("user_description_scroll"),
                    Message::SmoothScroll,
                );

                let collapse_btn = detail_description::toggle_button(
                    locale.get(Key::CollapseDescription),
                    Message::ToggleDescriptionExpand,
                );

                column![scrollable_desc, collapse_btn]
                    .spacing(2)
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
                    .spacing(2)
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

    row![avatar, Space::new().width(28), info]
        .align_y(Alignment::Center)
        .padding(Padding::new(48.0).top(84.0).bottom(28.0))
        .into()
}

fn build_playlist_grid<'a>(
    user: &PlaylistView,
    image_state: &'a ImageState,
    content_width: f32,
) -> Element<'a, Message> {
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

    let card_width = detail_card::CARD_WIDTH;
    let card_spacing = detail_card::CARD_SPACING;
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
            let cover_handle = image_state.get(ImageKind::PlaylistCover, playlist.id);
            row_items.push(detail_card::view(
                playlist.name.clone(),
                playlist.creator.nickname.clone(),
                cover_handle,
                ImageKind::PlaylistCover,
                Message::OpenNcmPlaylist(playlist.id),
            ));
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
        Message::SmoothScroll,
    )
}

fn circular_avatar(
    handle: Option<&iced::widget::image::Handle>,
    fallback_name: &str,
    size: f32,
) -> Element<'static, Message> {
    if handle.is_some() {
        return cover_image::circle(handle, ImageKind::UserAvatar, size);
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
