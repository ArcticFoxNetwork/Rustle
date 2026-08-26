//! Trending songs list component with hover animations
//!
//! Displays the NCM trending chart (飙升榜) with hover effects.

use iced::widget::{Space, button, column, container, row, svg, text};
use iced::{Alignment, Color, Element, Fill, Padding};

use crate::api::Track;
use crate::app::{ImageState, Message};
use crate::i18n::{Key, Locale};
use crate::image::ImageKind;
use crate::ui::animation::HoverAnimations;
use crate::ui::components::cover_image;
use crate::ui::theme::{self, BOLD_WEIGHT, MEDIUM_WEIGHT};

const ITEM_HEIGHT: f32 = 64.0;
const COVER_SIZE: f32 = 48.0;

/// Build the trending songs list view
pub fn view<'a>(
    songs: &'a [Track],
    image_state: &'a ImageState,
    hover_animations: &'a HoverAnimations<u64>,
    locale: Locale,
    is_logged_in: bool,
) -> Element<'a, Message> {
    let title = locale.get(Key::TrendingSongs);

    let header = row![
        text(title)
            .size(theme::TEXT_SIZE_TITLE)
            .font(iced::Font::DEFAULT.weight(BOLD_WEIGHT))
            .style(|theme| text::Style {
                color: Some(theme::text_primary(theme))
            }),
        Space::new().width(Fill),
        button(
            text(locale.get(Key::SeeAll))
                .size(theme::TEXT_SIZE_BODY)
                .color(theme::ACCENT),
        )
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
                    .size(theme::TEXT_SIZE_BODY)
                    .style(|theme| text::Style {
                        color: Some(theme::text_secondary(theme))
                    }),
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
            let cover_handle = image_state.get(ImageKind::SongCover, song.id);
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
    song: &'a Track,
    rank: usize,
    is_hovered: bool,
    hover_progress: f32,
    cover_handle: Option<&'a iced::widget::image::Handle>,
    is_logged_in: bool,
) -> Element<'a, Message> {
    let song_id = song.id;

    // Song info
    let song_name = text(&song.title)
        .size(theme::TEXT_SIZE_BODY)
        .font(iced::Font::DEFAULT.weight(MEDIUM_WEIGHT))
        .style(|theme| text::Style {
            color: Some(theme::text_primary(theme)),
        });

    let artist_text = text(song.artist_names())
        .size(theme::TEXT_SIZE_CAPTION)
        .style(|theme| text::Style {
            color: Some(theme::text_secondary(theme)),
        });

    let quality_badge: Element<'a, Message> = match song
        .quality_options
        .iter()
        .max_by_key(|option| option.level.priority())
    {
        Some(option) => text(option.level.short_name())
            .size(theme::TEXT_SIZE_CAPTION)
            .style(|_theme| text::Style {
                color: Some(theme::ACCENT),
            })
            .into(),
        None => Space::new().width(0).into(),
    };
    let availability_badge: Element<'a, Message> = if song.availability.label().is_empty() {
        Space::new().width(0).into()
    } else {
        text(song.availability.label())
            .size(theme::TEXT_SIZE_CAPTION)
            .style(|theme| text::Style {
                color: Some(if song.availability.is_restricted() {
                    theme::ACCENT_PINK
                } else {
                    theme::text_muted(theme)
                }),
            })
            .into()
    };

    let song_info = column![song_name, artist_text, row![quality_badge, availability_badge].spacing(6),].spacing(2);

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
            let duration = text(format_duration(song.duration_ms / 1000))
                .size(theme::TEXT_SIZE_CAPTION)
                .style(|theme| text::Style {
                    color: Some(theme::text_secondary(theme)),
                })
                .into();
            (duration, 50.0)
        };

    // Song cover image - use pre-loaded handle for instant rendering
    let song_cover = cover_image::custom(cover_handle, ImageKind::SongCover, COVER_SIZE, 4.0);

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
                .size(theme::TEXT_SIZE_BODY_LARGE)
                .style(move |theme| text::Style {
                    color: Some(theme::rank_color(rank, theme)),
                })
                .font(iced::Font::DEFAULT.weight(BOLD_WEIGHT)),
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
                // Only interpolate alpha; keep RGB fixed to surface_hover.
                // Avoids dirty-looking mid-tones that RGB+alpha joint interpolation produces
                // over a white background in light mode.
                let target = theme::surface_hover(theme);
                let bg_color =
                    Color::from_rgba(target.r, target.g, target.b, target.a * hover_progress);
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
    .on_right_press(Message::RightClickNcmSong(song.clone()))
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
