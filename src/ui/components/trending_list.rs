//! Trending songs list component with hover animations
//!
//! Displays the NCM trending chart (飙升榜) with hover effects.

use iced::widget::{Space, button, column, container, image, row, svg, text};
use iced::{Alignment, Color, Element, Fill, Padding};

use crate::api::SongInfo;
use crate::app::Message;
use crate::i18n::{Key, Locale};
use crate::ui::animation::HoverAnimations;
use crate::ui::theme::{self, BOLD_WEIGHT, MEDIUM_WEIGHT};

const ITEM_HEIGHT: f32 = 64.0;
const COVER_SIZE: f32 = 48.0;

/// Build the trending songs list view
pub fn view<'a>(
    songs: &'a [SongInfo],
    song_covers: &'a std::collections::HashMap<u64, iced::widget::image::Handle>,
    hover_animations: &'a HoverAnimations<u64>,
    locale: Locale,
    is_logged_in: bool,
) -> Element<'a, Message> {
    let title = locale.get(Key::TrendingSongs);

    let header = row![
        text(title)
            .size(20)
            .font(iced::Font {
                weight: BOLD_WEIGHT,
                ..Default::default()
            })
            .style(|theme| text::Style {
                color: Some(theme::text_primary(theme))
            }),
        Space::new().width(Fill),
        button(text(locale.get(Key::SeeAll)).size(14).color(theme::ACCENT))
            .style(theme::text_button)
            .on_press(Message::OpenTrendingSongs),
    ]
    .align_y(Alignment::Center)
    .padding(Padding::new(0.0).bottom(16.0));

    if songs.is_empty() {
        return column![
            header,
            container(
                text(locale.get(Key::Loading).to_string())
                    .size(14)
                    .color(theme::TEXT_SECONDARY),
            )
            .width(Fill)
            .height(200)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center),
        ]
        .into();
    }

    // Only show first 10 songs
    let visible_songs: Vec<_> = songs.iter().take(10).enumerate().collect();

    let song_items: Vec<Element<'_, Message>> = visible_songs
        .into_iter()
        .map(|(index, song)| {
            let hover_progress = hover_animations.get_progress(&song.id);
            let cover_handle = song_covers.get(&song.id);
            let is_hovered = hover_progress > 0.01; // Lower threshold to fix timing issue
            view_song_item(
                song,
                index + 1,
                is_hovered,
                hover_progress,
                cover_handle,
                is_logged_in,
            )
        })
        .collect();

    column![header, column(song_items).spacing(4),].into()
}

/// Build a single song item with hover effect
fn view_song_item<'a>(
    song: &'a SongInfo,
    rank: usize,
    is_hovered: bool,
    hover_progress: f32,
    cover_handle: Option<&'a iced::widget::image::Handle>,
    is_logged_in: bool,
) -> Element<'a, Message> {
    let song_id = song.id;

    // Song info
    let song_name = text(&song.name)
        .size(14)
        .font(iced::Font {
            weight: MEDIUM_WEIGHT,
            ..Default::default()
        })
        .style(|theme| text::Style {
            color: Some(theme::text_primary(theme)),
        });

    let artist_text = text(&song.singer).size(12).color(theme::TEXT_SECONDARY);

    let song_info = column![song_name, artist_text,].spacing(2);

    // Duration text and favorite button - show favorite on hover if logged in
    let (duration_or_favorite, duration_width): (Element<'_, Message>, f32) =
        if is_logged_in && is_hovered {
            let favorite_btn = button(
                svg(svg::Handle::from_memory(crate::ui::icons::HEART.as_bytes()))
                    .width(18)
                    .height(18)
                    .style(move |theme, _status| svg::Style {
                        color: Some(theme::text_primary(theme)),
                    }),
            )
            .style(theme::icon_button)
            .on_press(Message::ToggleFavorite(song.id))
            .into();
            (favorite_btn, 32.0)
        } else {
            let duration = text(format_duration(song.duration / 1000))
                .size(12)
                .color(theme::TEXT_SECONDARY)
                .into();
            (duration, 50.0)
        };

    // Song cover image - use pre-loaded handle for instant rendering
    let song_cover: Element<'_, Message> = if let Some(handle) = cover_handle {
        container(
            image(handle.clone())
                .width(COVER_SIZE)
                .height(COVER_SIZE)
                .content_fit(iced::ContentFit::Cover),
        )
        .width(COVER_SIZE)
        .height(COVER_SIZE)
        .clip(true)
        .style(|_theme| container::Style {
            border: iced::Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
    } else {
        // Fallback colored square with music note
        container(
            svg(svg::Handle::from_memory(crate::ui::icons::MUSIC.as_bytes()))
                .width(20)
                .height(20)
                .style(move |_theme, _status| svg::Style {
                    color: Some(theme::TEXT_MUTED),
                }),
        )
        .width(COVER_SIZE)
        .height(COVER_SIZE)
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(theme::BACKGROUND_DARK)),
            border: iced::Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .center_x(COVER_SIZE)
        .center_y(COVER_SIZE)
        .into()
    };

    // Create rank display directly to avoid move issues
    let rank_display = if is_hovered {
        container(
            svg(svg::Handle::from_memory(crate::ui::icons::PLAY.as_bytes()))
                .width(16)
                .height(16)
                .style(|theme, _status| svg::Style {
                    color: Some(theme::text_primary(theme)),
                }),
        )
        .width(48)
        .center_x(48)
    } else {
        container(
            text(format!("{}", rank))
                .size(15)
                .style(move |theme| text::Style {
                    color: Some(theme::rank_color(rank, theme)),
                })
                .font(iced::Font {
                    weight: BOLD_WEIGHT,
                    ..Default::default()
                }),
        )
        .width(48)
        .center_x(48)
    };

    // Row layout matching playlist page
    let content = row![
        rank_display,
        song_cover,
        Space::new().width(14),
        song_info,
        Space::new().width(Fill),
        container(duration_or_favorite)
            .width(duration_width)
            .center_x(duration_width),
    ]
    .align_y(Alignment::Center)
    .padding(Padding::new(8.0).left(8.0).right(12.0))
    .height(ITEM_HEIGHT);

    // Wrap in mouse_area for hover tracking
    // Use button for cursor pointer, wrapped in mouse_area for hover animations
    iced::widget::mouse_area(
        button(content)
            .width(Fill)
            .padding(0)
            .style(move |theme, _status| {
                // Interpolate background color based on hover
                let bg_color = interpolate_color(
                    Color::TRANSPARENT,
                    theme::surface_hover(theme),
                    hover_progress,
                );
                button::Style {
                    background: Some(bg_color.into()),
                    border: iced::Border {
                        radius: 8.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })
            .on_press(Message::PlayNcmSong(song.clone())),
    )
    .on_enter(Message::HoverTrendingSong(Some(song_id)))
    .on_exit(Message::HoverTrendingSong(None))
    .into()
}

/// Format duration in mm:ss format
fn format_duration(secs: u64) -> String {
    let mins = secs / 60;
    let secs = secs % 60;
    format!("{:02}:{:02}", mins, secs)
}

/// Interpolate between two colors
fn interpolate_color(from: Color, to: Color, t: f32) -> Color {
    Color::from_rgba(
        from.r + (to.r - from.r) * t,
        from.g + (to.g - from.g) * t,
        from.b + (to.b - from.b) * t,
        from.a + (to.a - from.a) * t,
    )
}
