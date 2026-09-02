//! Bottom player bar component

use iced::widget::{Space, button, column, container, opaque, row, svg, text};
use iced::{Alignment, Color, Element, Fill, Length, Padding};

use crate::app::Message;
use crate::database::DbSong;
use crate::features::PlayMode;
use crate::ui::theme::BOLD_WEIGHT;
use crate::ui::widgets::{self, ControlSize, SliderSize};
use crate::ui::{icons, theme};
use crate::utils;

/// Player bar height
pub const PLAYER_BAR_HEIGHT: f32 = 80.0;

const COVER_EXPAND_CHEVRON: &str = r#"<svg viewBox="0 0 24 12" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
    <path d="M4 8.5L12 5L20 8.5"/>
</svg>"#;

/// Build the player bar
pub fn view(
    current_song: Option<&DbSong>,
    current_artist_id: Option<u64>,
    is_playing: bool,
    position: f32, // 0.0 to 1.0
    duration_secs: f32,
    volume: f32, // 0.0 to 1.0
    play_mode: PlayMode,
    current_favorite: Option<(u64, bool)>,
    progress_colors: Option<[Color; 3]>,
    is_buffering: bool,             // Whether streaming is buffering
    download_progress: Option<f32>, // Download progress 0.0 to 1.0 (None if not streaming)
    is_fm_mode: bool,               // Whether in Personal FM mode
    is_first_song: bool,            // Whether at first song in queue
    current_song_cover: Option<&iced::widget::image::Handle>,
    current_quality: Option<&crate::app::ResolvedAudioQuality>,
) -> Element<'static, Message> {
    let current_time = utils::format_time(position * duration_secs);
    let total_time = utils::format_time(duration_secs);

    // Left section: Song info or placeholder (fixed width to prevent layout issues)
    const LEFT_SECTION_WIDTH: f32 = 260.0;
    const TEXT_MAX_WIDTH: f32 = 180.0;

    let song_info: Element<'static, Message> = if let Some(song) = current_song {
        let song_clone = song.clone();

        // Cover - clickable to open lyrics page
        let s = crate::image::CoverSize::Medium;
        let cover_content: Element<'static, Message> = crate::ui::components::cover_image::cover(
            current_song_cover,
            crate::image::ImageKind::SongCover,
            s,
        );

        let cover_size = s.px();
        let cover_radius = s.radius();
        let expand_overlay =
            widgets::hover_surface(Space::new().width(cover_size).height(cover_size))
                .style(move |_theme, progress| iced::widget::container::Style {
                    background: Some(iced::Background::Color(Color::from_rgba(
                        0.0,
                        0.0,
                        0.0,
                        0.58 * progress,
                    ))),
                    border: iced::Border {
                        radius: cover_radius.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .svg_overlay(
                    svg::Handle::from_memory(COVER_EXPAND_CHEVRON.as_bytes()),
                    iced::Size::new(24.0, 12.0),
                    Color::WHITE,
                );

        let cover_btn = button(iced::widget::stack![cover_content, expand_overlay])
            .padding(0)
            .style(|_theme, _status| button::Style {
                background: Some(iced::Background::Color(Color::TRANSPARENT)),
                ..Default::default()
            })
            .on_press(Message::OpenLyricsPage);

        // Title - clickable to open lyrics page
        const TITLE_LINE_HEIGHT: f32 = 17.0;
        const TITLE_MAX_LINES: f32 = 2.0;
        const TITLE_MAX_HEIGHT: f32 = TITLE_LINE_HEIGHT * TITLE_MAX_LINES;

        let title_btn = button(
            container(
                text(song_clone.title.clone())
                    .size(theme::TEXT_SIZE_BODY)
                    .style(|theme| text::Style {
                        color: Some(theme::text_primary(theme)),
                    })
                    .font(iced::Font::DEFAULT.weight(BOLD_WEIGHT))
                    .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
            )
            .max_width(TEXT_MAX_WIDTH)
            .max_height(TITLE_MAX_HEIGHT)
            .clip(true),
        )
        .padding(0)
        .style(|_theme, _status| button::Style {
            background: Some(iced::Background::Color(Color::TRANSPARENT)),
            ..Default::default()
        })
        .on_press(Message::OpenLyricsPage);

        // Artist
        let artist_action = current_artist_id
            .map(Message::OpenArtist)
            .or_else(|| Some(Message::OpenArtistByName(song_clone.artist.clone())));
        let artist_btn = button(
            container(
                text(song_clone.artist.clone())
                    .size(theme::TEXT_SIZE_CAPTION)
                    .style(|theme| text::Style {
                        color: Some(theme::text_secondary(theme)),
                    }),
            )
            .max_width(TEXT_MAX_WIDTH)
            .clip(true),
        )
        .padding(0)
        .style(|_theme, _status| button::Style {
            background: Some(iced::Background::Color(Color::TRANSPARENT)),
            ..Default::default()
        })
        .on_press_maybe(artist_action);

        let quality_badge: Element<'static, Message> = if let Some(quality) = current_quality {
            // The configured preference is internal negotiation context. The
            // player bar describes this song, so only show its actual quality.
            text(quality.actual.short_name())
                .size(theme::TEXT_SIZE_CAPTION)
                .style(|_theme| text::Style {
                    color: Some(theme::ACCENT),
                })
                .into()
        } else {
            Space::new().height(0).into()
        };

        let artist_and_quality = row![artist_btn, quality_badge]
            .spacing(6)
            .align_y(Alignment::Center);
        let song_details = column![title_btn, artist_and_quality].spacing(2);

        row![cover_btn, Space::new().width(12), song_details]
            .align_y(Alignment::Center)
            .into()
    } else {
        // Show placeholder when no song
        let placeholder = column![
            text("No song playing")
                .size(theme::TEXT_SIZE_BODY)
                .style(|theme| text::Style {
                    color: Some(theme::text_muted(theme))
                }),
            text("Select a song to play")
                .size(theme::TEXT_SIZE_CAPTION)
                .style(|theme| text::Style {
                    color: Some(theme::text_muted(theme))
                }),
        ]
        .spacing(2);

        row![
            container(
                svg(svg::Handle::from_memory(icons::MUSIC.as_bytes()))
                    .width(24)
                    .height(24)
                    .style(|theme, _status| svg::Style {
                        color: Some(theme::opaque_color(theme::icon_muted(theme))),
                    })
                    .opacity(0.4),
            )
            .width(56)
            .height(56)
            .center_x(56)
            .center_y(56)
            .style(|theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(theme::surface_container(theme))),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
            Space::new().width(12),
            placeholder
        ]
        .align_y(Alignment::Center)
        .into()
    };

    let left_section = container(song_info)
        .width(LEFT_SECTION_WIDTH)
        .align_y(Alignment::Center);

    // Center section: Playback controls (using unified widgets)
    let controls = widgets::playback_controls::view_player_bar(
        is_playing,
        is_buffering,
        ControlSize::Small,
        is_fm_mode,
        is_first_song,
        play_mode,
        current_favorite,
    );

    let progress_slider = widgets::progress_slider::view_with_gradient(
        position,
        download_progress,
        SliderSize::Edge,
        progress_colors,
    );

    let center_section = container(controls)
        .width(Length::Fill)
        .align_x(Alignment::Center);

    // Right section: Volume control (using unified widgets)
    let volume_icon = svg(svg::Handle::from_memory(icons::VOLUME.as_bytes()))
        .width(20)
        .height(20)
        .style(|_theme, _status| svg::Style {
            color: Some(theme::text_secondary(_theme)),
        });

    let volume_slider = widgets::progress_slider::volume_slider(volume);

    // Queue button
    let queue_btn = button(
        svg(svg::Handle::from_memory(icons::QUEUE.as_bytes()))
            .width(20)
            .height(20)
            .style(|_theme, _status| svg::Style {
                color: Some(theme::text_secondary(_theme)),
            }),
    )
    .padding(8)
    .style(|_theme, _status| button::Style {
        background: Some(iced::Background::Color(Color::TRANSPARENT)),
        ..Default::default()
    })
    .on_press(Message::ToggleQueue);
    let queue_btn =
        widgets::hover_surface(queue_btn).style(|theme, progress| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::hover_bg_alpha(
                theme,
                0.12 * progress,
            ))),
            border: iced::Border {
                radius: 18.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    let volume_area = iced::widget::mouse_area(
        row![volume_icon, Space::new().width(8), volume_slider,]
            .align_y(Alignment::Center)
            .width(Length::Shrink),
    )
    .on_enter(Message::VolumeSliderHovered(true))
    .on_exit(Message::VolumeSliderHovered(false));

    let right_section = row![
        text(format!("{current_time} / {total_time}"))
            .size(theme::TEXT_SIZE_CAPTION)
            .style(|theme| text::Style {
                color: Some(theme::text_muted(theme))
            }),
        Space::new().width(8),
        volume_area,
        Space::new().width(12),
        queue_btn,
    ]
    .align_y(Alignment::Center)
    .width(Length::Shrink);

    // Combine all sections
    let content = row![left_section, center_section, right_section,]
        .spacing(16)
        .align_y(Alignment::Center)
        .padding(Padding::new(0.0).left(16.0).right(16.0));

    const PROGRESS_BAR_HEIGHT: f32 = 8.0;
    let top_progress = container(progress_slider)
        .width(Fill)
        .height(PROGRESS_BAR_HEIGHT)
        .style(|theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::player_bar_bg(theme))),
            ..Default::default()
        });

    let main_content = container(content)
        .width(Fill)
        .height(PLAYER_BAR_HEIGHT)
        .padding(Padding::new(0.0).top(PROGRESS_BAR_HEIGHT))
        .align_y(Alignment::Center)
        .style(|theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::player_bar_bg(theme))),
            ..Default::default()
        });

    // Draw the progress slider after the player bar body so its hover handle
    // can extend below the rail without being covered by the body background.
    let bar = iced::widget::stack![main_content, top_progress]
        .width(Fill)
        .height(PLAYER_BAR_HEIGHT);

    // Use opaque to block events from reaching underlying widgets without swallowing
    // interactions inside the player bar itself.
    opaque(bar)
}
