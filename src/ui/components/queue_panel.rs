//! Queue popup component
//!
//! Shows the current play queue as a popup bubble above the player bar.

use iced::widget::{Space, button, column, container, row, scrollable, svg, text};
use iced::{Alignment, Color, Element, Fill, Length, Padding};

use crate::app::Message;
use crate::database::DbSong;
use crate::i18n::{Key, Locale};
use crate::ui::{icons, theme};

/// Queue popup width
pub const QUEUE_PANEL_WIDTH: f32 = 360.0;
/// Queue popup max height
pub const QUEUE_PANEL_HEIGHT: f32 = 400.0;
/// Height of each queue item (padding 8*2 + content ~36)
const QUEUE_ITEM_HEIGHT: f32 = 54.0;
/// Scrollable ID for queue panel
pub const QUEUE_SCROLLABLE_ID: &str = "queue_panel_scroll";

/// Calculate the scroll offset to center the current song in the queue panel
/// Returns a relative offset (0.0 to 1.0) for use with scrollable::snap_to
pub fn calculate_scroll_offset(queue_len: usize, queue_index: Option<usize>) -> f32 {
    let Some(idx) = queue_index else {
        return 0.0;
    };

    if queue_len == 0 {
        return 0.0;
    }

    let visible_height = QUEUE_PANEL_HEIGHT - 60.0;
    let total_height = queue_len as f32 * QUEUE_ITEM_HEIGHT;

    if total_height <= visible_height {
        return 0.0;
    }

    let item_center = idx as f32 * QUEUE_ITEM_HEIGHT + QUEUE_ITEM_HEIGHT / 2.0;
    let target_scroll = item_center - visible_height / 2.0;
    let max_scroll = total_height - visible_height;
    let clamped_scroll = target_scroll.clamp(0.0, max_scroll);
    clamped_scroll / max_scroll
}

/// Build the queue popup bubble
pub fn view(
    queue: &[DbSong],
    queue_index: Option<usize>,
    locale: Locale,
    is_fm_mode: bool,
) -> Element<'static, Message> {
    let header_title = if is_fm_mode {
        "私人FM".to_string()
    } else {
        locale.get(Key::QueueTitle).to_string()
    };

    let header = row![
        text(header_title).size(16).style(move |theme| text::Style {
            color: Some(theme::text_primary(theme))
        }),
        Space::new().width(Fill),
        text(format!("{}", queue.len()))
            .size(12)
            .style(|theme| text::Style {
                color: Some(theme::text_muted(theme))
            }),
        Space::new().width(8),
        button(
            svg(svg::Handle::from_memory(icons::TRASH.as_bytes()))
                .width(14)
                .height(14)
                .style(|theme, _status| svg::Style {
                    color: Some(theme::text_muted(theme)),
                })
        )
        .padding(6)
        .style(theme::transparent_btn)
        .on_press(Message::ClearQueue),
        button(
            svg(svg::Handle::from_memory(icons::CLOSE.as_bytes()))
                .width(14)
                .height(14)
                .style(|theme, _status| svg::Style {
                    color: Some(theme::text_muted(theme)),
                })
        )
        .padding(6)
        .style(theme::transparent_btn)
        .on_press(Message::ToggleQueue),
    ]
    .align_y(Alignment::Center)
    .padding(Padding::new(12.0).left(16.0).right(12.0));

    let song_items: Vec<Element<'static, Message>> = queue
        .iter()
        .enumerate()
        .map(|(idx, song)| {
            let is_current = queue_index == Some(idx);
            build_queue_item(song.clone(), idx, is_current)
        })
        .collect();

    let song_list: Element<'static, Message> = if song_items.is_empty() {
        container(
            text(locale.get(Key::QueueEmpty).to_string())
                .size(14)
                .style(|theme| text::Style {
                    color: Some(theme::text_muted(theme)),
                }),
        )
        .width(Fill)
        .padding(32)
        .center_x(Fill)
        .into()
    } else {
        scrollable(
            column(song_items)
                .spacing(2)
                .padding(Padding::new(0.0).left(8.0).right(8.0).bottom(8.0)),
        )
        .id(iced::widget::Id::new(QUEUE_SCROLLABLE_ID))
        .height(Length::Fixed(QUEUE_PANEL_HEIGHT - 60.0))
        .into()
    };

    let content = column![header, song_list,].width(QUEUE_PANEL_WIDTH);

    container(content)
        .width(QUEUE_PANEL_WIDTH)
        .max_height(QUEUE_PANEL_HEIGHT)
        .style(|theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::surface_elevated(theme))),
            border: iced::Border {
                color: theme::divider(theme),
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow: iced::Shadow {
                color: theme::overlay_backdrop(theme, 0.5),
                offset: iced::Vector::new(0.0, -4.0),
                blur_radius: 20.0,
            },
            ..Default::default()
        })
        .into()
}

/// Build a single queue item
fn build_queue_item(song: DbSong, index: usize, is_current: bool) -> Element<'static, Message> {
    let duration_secs = song.duration_secs as u64;
    let mins = duration_secs / 60;
    let secs = duration_secs % 60;
    let duration_str = format!("{}:{:02}", mins, secs);

    let indicator: Element<'static, Message> = if is_current {
        svg(svg::Handle::from_memory(icons::PLAYING.as_bytes()))
            .width(14)
            .height(14)
            .style(|_theme, _status| svg::Style {
                color: Some(theme::ACCENT_PINK),
            })
            .into()
    } else {
        text(format!("{}", index + 1))
            .size(12)
            .style(|theme| text::Style {
                color: Some(theme::text_muted(theme)),
            })
            .into()
    };

    let indicator_container = container(indicator).width(24).center_x(24);

    let title = text(song.title.clone())
        .size(13)
        .style(move |theme| text::Style {
            color: Some(if is_current {
                theme::ACCENT_PINK
            } else {
                theme::text_primary(theme)
            }),
        });

    let artist_text = if song.artist.is_empty() {
        "未知艺术家".to_string()
    } else {
        song.artist.clone()
    };
    let artist = text(artist_text).size(11).style(move |theme| text::Style {
        color: Some(if is_current {
            Color::from_rgba(
                theme::ACCENT_PINK.r,
                theme::ACCENT_PINK.g,
                theme::ACCENT_PINK.b,
                0.7,
            )
        } else {
            theme::text_muted(theme)
        }),
    });

    let info = column![title, artist].spacing(2).width(Fill);

    let duration = text(duration_str).size(11).style(|theme| text::Style {
        color: Some(theme::text_muted(theme)),
    });

    let remove_btn = button(
        svg(svg::Handle::from_memory(icons::CLOSE.as_bytes()))
            .width(12)
            .height(12)
            .style(|theme, _status| svg::Style {
                color: Some(theme::text_muted(theme)),
            }),
    )
    .padding(4)
    .style(theme::transparent_btn)
    .on_press(Message::RemoveFromQueue(index));

    let item_row = row![
        indicator_container,
        Space::new().width(8),
        info,
        duration,
        Space::new().width(4),
        remove_btn,
    ]
    .align_y(Alignment::Center)
    .padding(Padding::new(8.0).left(8.0).right(8.0));

    button(item_row)
        .width(Fill)
        .padding(0)
        .style(move |theme, status| {
            let bg_color = if is_current {
                theme::hover_bg(theme)
            } else {
                Color::TRANSPARENT
            };
            let hover_bg = match status {
                button::Status::Hovered | button::Status::Pressed => theme::hover_bg(theme),
                _ => bg_color,
            };
            button::Style {
                background: Some(iced::Background::Color(hover_bg)),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .on_press(Message::PlayQueueIndex(index))
        .into()
}
