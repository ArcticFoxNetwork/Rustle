//! Reusable playlist view component
//!
//! A generic song list component with virtual scrolling, hover animations,
//! and customizable columns. Can be used for playlists, albums, search results, etc.
//!
//! Performance optimizations:
//! - Pre-computed display strings (no format! in render loop)
//! - Pre-loaded image handles (no disk IO in render loop)
//! - Cached SVG handles (no repeated parsing)

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::LazyLock;

use iced::widget::{Space, button, column, container, mouse_area, row, svg, text};
use iced::{Alignment, Color, Element, Fill, Length, Padding};

use crate::app::{ImageState, Message};
use crate::i18n::{Key, Locale};
use crate::ui::animation::{SmoothScrollEvent, SmoothScrollTarget};
use crate::ui::theme::BOLD_WEIGHT;
use crate::ui::widgets::{VirtualList, VirtualListState};
use crate::ui::{icons, theme};
use crate::utils::Source;

/// Song row height constant for virtual list
pub const SONG_ROW_HEIGHT: f32 = 62.0;

/// Pre-cached SVG handles to avoid repeated parsing in render loop
static PLAY_ICON_HANDLE: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(icons::PLAY.as_bytes()));
static HEART_ICON_HANDLE: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(icons::HEART.as_bytes()));
static HEART_OUTLINE_ICON_HANDLE: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(icons::HEART_OUTLINE.as_bytes()));
static CLOCK_ICON_HANDLE: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(icons::CLOCK.as_bytes()));

/// Maximum lengths for display text truncation
const MAX_TITLE_LEN: usize = 28;
const MAX_ARTIST_LEN: usize = 25;

/// Song item data for display in the list
/// All display strings and image handles are pre-computed for performance
#[derive(Debug, Clone)]
pub struct SongItem {
    pub id: i64,
    /// Remote cover URL when the row represents an online song.
    /// Local songs resolve their cover through the local image cache.
    pub cover_url: Option<String>,
    /// Pre-formatted index string to avoid format! in render loop
    pub index_str: String,
    /// Original title for search/filter
    pub title: String,
    /// Original artist for search/filter
    pub artist: String,
    /// Pre-normalized searchable text to avoid lowercasing on every view rebuild.
    search_text: String,
    /// Pre-truncated display title
    pub display_title: String,
    /// Pre-truncated display artist
    pub display_artist: String,
    /// Pre-truncated display album
    pub display_album: String,
    pub duration: String,
    pub added_date: String,
    /// Song source for display badge
    pub source: Source,
}

impl SongItem {
    /// Create a new SongItem with pre-computed display values.
    /// Cover resolution is deferred to `image::resolve`.
    pub fn new(
        id: i64,
        cover_url: Option<String>,
        index: usize,
        title: String,
        artist: String,
        album: String,
        duration: String,
        added_date: String,
        source: Source,
    ) -> Self {
        let display_title = truncate_string(&title, MAX_TITLE_LEN);
        let display_artist = truncate_string(&artist, MAX_ARTIST_LEN);
        let display_album = album.clone();
        let index_str = index.to_string();
        let search_text = format!(
            "{}\n{}\n{}",
            title.to_lowercase(),
            artist.to_lowercase(),
            album.to_lowercase()
        );

        Self {
            id,
            cover_url,
            index_str,
            title,
            artist,
            search_text,
            display_title,
            display_artist,
            display_album,
            duration,
            added_date,
            source,
        }
    }
}

/// Truncate string with ellipsis if too long
#[inline]
fn truncate_string(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count > max_len {
        let mut result: String = s.chars().take(max_len).collect();
        result.push('…');
        result
    } else {
        s.to_string()
    }
}

/// Configuration for playlist view columns
#[derive(Debug, Clone, Copy)]
pub struct PlaylistColumns {
    /// Show the like button column (for online playlists)
    pub show_like: bool,
    /// Show the added date column (for local playlists)
    pub show_added_date: bool,
    /// Show album column
    pub show_album: bool,
}

impl Default for PlaylistColumns {
    fn default() -> Self {
        Self {
            show_like: true,
            show_added_date: false,
            show_album: true,
        }
    }
}

impl PlaylistColumns {
    /// Configuration for local playlists (with added date, no like button)
    pub fn local() -> Self {
        Self {
            show_like: false,
            show_added_date: true,
            show_album: true,
        }
    }

    /// Configuration for online/cloud playlists (with like button, no added date)
    pub fn online() -> Self {
        Self {
            show_like: true,
            show_added_date: false,
            show_album: true,
        }
    }
}

/// Build the song list header row
pub fn build_header(locale: Locale, columns: PlaylistColumns) -> Element<'static, Message> {
    let mut header_items: Vec<Element<'static, Message>> = vec![
        container(
            text(locale.get(Key::PlaylistHeaderNumber))
                .size(theme::TEXT_SIZE_LABEL)
                .style(|theme| text::Style {
                    color: Some(theme::header_text(theme)),
                }),
        )
        .width(48)
        .center_x(48)
        .into(),
        Space::new().width(44).into(), // Cover space
        Space::new().width(14).into(),
        text(locale.get(Key::PlaylistHeaderTitle))
            .size(theme::TEXT_SIZE_LABEL)
            .style(|theme| text::Style {
                color: Some(theme::header_text(theme)),
            })
            .into(),
        Space::new().width(Fill).into(),
    ];

    if columns.show_album {
        header_items.push(
            container(
                text(locale.get(Key::PlaylistHeaderAlbum))
                    .size(theme::TEXT_SIZE_LABEL)
                    .style(|theme| text::Style {
                        color: Some(theme::header_text(theme)),
                    }),
            )
            .width(200)
            .into(),
        );
    }

    if columns.show_added_date {
        header_items.push(
            container(
                text(locale.get(Key::PlaylistHeaderAddedDate))
                    .size(theme::TEXT_SIZE_LABEL)
                    .style(|theme| text::Style {
                        color: Some(theme::header_text(theme)),
                    }),
            )
            .width(90)
            .into(),
        );
    }

    // Duration/clock icon column - use cached handle
    header_items.push(
        container(
            svg(CLOCK_ICON_HANDLE.clone())
                .width(16)
                .height(16)
                .style(|theme, _status| svg::Style {
                    color: Some(theme::header_text(theme)),
                }),
        )
        .width(50)
        .center_x(50)
        .into(),
    );

    let header = row(header_items)
        .align_y(Alignment::Center)
        .padding(Padding::new(14.0).left(20.0).right(24.0));

    let header_container = container(header).width(Fill);

    // Divider line
    let divider = container(Space::new().height(1))
        .width(Fill)
        .padding(Padding::new(0.0).left(20.0).right(20.0))
        .style(|theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::divider(theme))),
            ..Default::default()
        });

    column![header_container, divider, Space::new().height(8),]
        .spacing(0)
        .width(Fill)
        .into()
}

/// Build the virtual song list
pub fn build_list<'a>(
    songs: &'a [SongItem],
    filtered_indices: Option<Vec<usize>>,
    image_state: &'a ImageState,
    song_animations: &'a crate::ui::animation::HoverAnimations<i64>,
    liked_songs: Option<&'a HashSet<u64>>,
    columns: PlaylistColumns,
    scroll_state: Rc<RefCell<VirtualListState>>,
    current_playing_id: Option<i64>,
) -> Element<'a, Message> {
    let filtered_indices = filtered_indices.map(Rc::new);
    let song_count = filtered_indices
        .as_ref()
        .map_or(songs.len(), |idx| idx.len());

    if song_count == 0 {
        return container(text("暂无歌曲").size(theme::TEXT_SIZE_BODY).style(|theme| {
            text::Style {
                color: Some(theme::dimmed_text(theme)),
            }
        }))
        .width(Fill)
        .padding(Padding::new(32.0))
        .center_x(Fill)
        .into();
    }

    let indices_for_builder = filtered_indices.clone();
    let indices_for_hover = filtered_indices.clone();
    let item_builder = move |index: usize| -> Element<'a, Message> {
        let song_index = indices_for_builder
            .as_ref()
            .and_then(|indices| indices.get(index).copied())
            .unwrap_or(index);

        let Some(song) = songs.get(song_index) else {
            return Space::new().height(SONG_ROW_HEIGHT).into();
        };

        let is_playing = current_playing_id == Some(song.id);
        let animation_progress = song_animations.get_progress(&song.id);
        let is_hovered = animation_progress > 0.5;

        container(build_song_row(
            song,
            image_state,
            is_playing,
            is_hovered,
            animation_progress,
            liked_songs,
            columns,
        ))
        .padding(Padding::new(1.0).left(12.0).right(12.0))
        .into()
    };

    let indices_for_key = filtered_indices.clone();
    let indices_for_visible = filtered_indices.clone();
    let songs_for_visible = songs;
    let image_generation = image_state.generation;

    VirtualList::new(song_count, SONG_ROW_HEIGHT, item_builder)
        .keyed_by(move |index| {
            let song_index = indices_for_key
                .as_ref()
                .and_then(|indices| indices.get(index).copied())
                .unwrap_or(index);

            songs
                .get(song_index)
                .map(|song| (song.id, song_index))
                .unwrap_or((i64::MIN, index))
        })
        .state(scroll_state)
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(0.0)
        .on_empty_area(Message::HoverSong(None))
        .on_item_hover(move |index| {
            let song_index = indices_for_hover
                .as_ref()
                .and_then(|indices| indices.get(index).copied())
                .unwrap_or(index);
            let song_id = songs.get(song_index).map(|s| s.id);
            Message::HoverSong(song_id)
        })
        .on_smooth_scroll(|delta| {
            Message::SmoothScroll(SmoothScrollEvent::Requested {
                target: SmoothScrollTarget::PlaylistSongs,
                delta,
            })
        })
        .on_smooth_scroll_cancel(Message::SmoothScroll(SmoothScrollEvent::Cancelled {
            target: SmoothScrollTarget::PlaylistSongs,
        }))
        .on_visible_range(move |(start, end)| {
            let mut images = Vec::new();
            for index in start..end {
                let song_index = indices_for_visible
                    .as_ref()
                    .and_then(|indices| indices.get(index).copied())
                    .unwrap_or(index);
                let Some(song) = songs_for_visible.get(song_index) else {
                    continue;
                };
                let Some((kind, id)) = crate::image::song_cover_key(song.id) else {
                    continue;
                };
                let Some(url) = song.cover_url.as_deref() else {
                    continue;
                };
                if !url.is_empty() {
                    images.push((kind, id, url.to_string()));
                }
            }
            Message::ImageViewportChanged(image_generation, images)
        })
        .visible_range_token(image_generation)
        .into()
}

/// Build a single song row with hover effect
/// Optimized: No disk IO, no string allocations, uses pre-cached handles
fn build_song_row(
    song: &SongItem,
    image_state: &ImageState,
    is_playing: bool,
    is_hovered: bool,
    animation_progress: f32,
    liked_songs: Option<&HashSet<u64>>,
    columns: PlaylistColumns,
) -> Element<'static, Message> {
    let song_id = song.id;

    // Clone strings for 'static lifetime (these are pre-computed, so cheap)
    let index_str = song.index_str.clone();
    let display_title = song.display_title.clone();
    let display_artist = song.display_artist.clone();
    let display_album = song.display_album.clone();
    let duration = song.duration.clone();
    let added_date = song.added_date.clone();

    // --- Index or play icon (fixed slot width; content is re-diffed by VirtualList) ---
    let index_content: Element<'static, Message> = if is_hovered {
        svg(PLAY_ICON_HANDLE.clone())
            .width(16)
            .height(16)
            .style(|theme, _status| svg::Style {
                color: Some(theme::text_primary(theme)),
            })
            .into()
    } else if is_playing {
        svg(PLAY_ICON_HANDLE.clone())
            .width(16)
            .height(16)
            .style(|_theme, _status| svg::Style {
                color: Some(theme::ACCENT_PINK),
            })
            .into()
    } else {
        text(index_str)
            .size(theme::TEXT_SIZE_BODY_LARGE)
            .style(|theme| text::Style {
                color: Some(theme::dimmed_text(theme)),
            })
            .into()
    };

    // --- Song cover (use pre-loaded handle, no disk IO) ---
    let cover_handle =
        crate::image::song_cover_key(song.id).and_then(|(kind, id)| image_state.get(kind, id));
    let cover = crate::ui::components::cover_image::custom(
        cover_handle,
        crate::image::ImageKind::SongCover,
        44.0,
        4.0,
    );

    // --- Title info (use pre-truncated strings) ---
    let title_info = column![
        text(display_title)
            .size(theme::TEXT_SIZE_BODY_LARGE)
            .style(move |theme| text::Style {
                color: Some(if is_playing {
                    theme::ACCENT_PINK
                } else {
                    theme::text_primary(theme)
                })
            })
            .font(iced::Font::DEFAULT.weight(BOLD_WEIGHT)),
        row![
            super::source_badge::source_badge(song.source),
            text(display_artist)
                .size(theme::TEXT_SIZE_LABEL)
                .style(move |theme| text::Style {
                    color: Some(theme::animated_text(theme, animation_progress))
                }),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(6),
    ]
    .spacing(3);

    // --- Like button handling ---
    let ncm_song_id = if song_id < 0 {
        (-song_id) as u64
    } else {
        song_id as u64
    };
    let is_liked = liked_songs.is_some_and(|songs| songs.contains(&ncm_song_id));

    // Duration or like button
    let duration_or_like: Element<'static, Message> = if columns.show_like && is_hovered {
        let heart_handle = if is_liked {
            HEART_ICON_HANDLE.clone()
        } else {
            HEART_OUTLINE_ICON_HANDLE.clone()
        };
        button(
            svg(heart_handle)
                .width(18)
                .height(18)
                .style(move |theme, _status| svg::Style {
                    color: Some(if is_liked {
                        theme::ACCENT_PINK
                    } else {
                        theme::text_primary(theme)
                    }),
                }),
        )
        .padding(0)
        .style(transparent_button)
        .on_press(Message::ToggleFavorite(ncm_song_id))
        .into()
    } else {
        text(duration)
            .size(theme::TEXT_SIZE_BODY)
            .style(move |theme| text::Style {
                color: Some(theme::animated_text(theme, animation_progress * 0.8)),
            })
            .into()
    };

    // --- Build row content (flattened structure) ---
    let mut row_items: Vec<Element<'static, Message>> = vec![
        container(index_content).width(48).center_x(48).into(),
        cover,
        Space::new().width(14).into(),
        title_info.width(Fill).into(),
    ];

    if columns.show_album {
        row_items.push(
            text(display_album)
                .size(theme::TEXT_SIZE_BODY)
                .style(move |theme| text::Style {
                    color: Some(theme::animated_text(theme, animation_progress)),
                })
                .width(200)
                .into(),
        );
    }

    if columns.show_added_date {
        row_items.push(
            text(added_date)
                .size(theme::TEXT_SIZE_BODY)
                .style(move |theme| text::Style {
                    color: Some(theme::animated_text(theme, animation_progress)),
                })
                .width(90)
                .into(),
        );
    }

    row_items.push(container(duration_or_like).width(50).center_x(50).into());

    let row_content = row(row_items)
        .align_y(Alignment::Center)
        .padding(Padding::new(8.0).left(8.0).right(12.0));

    // --- Outer button with animated background ---
    let btn = button(row_content)
        .width(Fill)
        .padding(0)
        .style(move |theme, _status| {
            let bg_color = if animation_progress > 0.001 {
                theme::hover_bg_alpha(theme, 0.12 * animation_progress)
            } else {
                Color::TRANSPARENT
            };
            button::Style {
                background: Some(iced::Background::Color(bg_color)),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                text_color: theme::text_primary(theme),
                ..Default::default()
            }
        })
        .on_press(Message::PlaySong(song_id));

    // Hover is now handled by VirtualList's on_item_hover for reliable tracking
    mouse_area(btn)
        .on_right_press(Message::RightClickSong(song_id))
        .into()
}

fn transparent_button(
    _theme: &iced::Theme,
    _status: button::Status,
) -> iced::widget::button::Style {
    iced::widget::button::Style {
        background: None,
        ..Default::default()
    }
}

/// Filter songs by search query (title, artist, album)
pub fn filter_song_indices(songs: &[SongItem], query: &str) -> Option<Vec<usize>> {
    if query.is_empty() {
        return None;
    }

    let query_lower = query.to_lowercase();
    Some(
        songs
            .iter()
            .enumerate()
            .filter_map(|(index, song)| song.search_text.contains(&query_lower).then_some(index))
            .collect(),
    )
}
