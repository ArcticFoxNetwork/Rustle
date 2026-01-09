//! Bottom player bar component

use iced::widget::{Space, button, column, container, image, mouse_area, opaque, row, svg, text};
use iced::{Alignment, Color, Element, Fill, Length, Padding};

use crate::app::Message;
use crate::database::DbSong;
use crate::features::PlayMode;
use crate::ui::theme::MEDIUM_WEIGHT;
use crate::ui::widgets::{self, ControlSize, PlayModeButtonSize, SliderSize};
use crate::ui::{icons, theme};

/// Player bar height
pub const PLAYER_BAR_HEIGHT: f32 = 80.0;

/// Build the player bar
pub fn view(
    current_song: Option<&DbSong>,
    is_playing: bool,
    position: f32, // 0.0 to 1.0
    duration_secs: f32,
    volume: f32,       // 0.0 to 1.0
    _is_seeking: bool, // Whether user is dragging the slider (reserved for future use)
    play_mode: PlayMode,
    is_buffering: bool,             // Whether streaming is buffering
    download_progress: Option<f32>, // Download progress 0.0 to 1.0 (None if not streaming)
) -> Element<'static, Message> {
    // Format time as mm:ss
    let format_time = |secs: f32| -> String {
        let mins = (secs / 60.0) as u32;
        let secs = (secs % 60.0) as u32;
        format!("{}:{:02}", mins, secs)
    };

    let current_time = format_time(position * duration_secs);
    let total_time = format_time(duration_secs);

    // Left section: Song info or placeholder (fixed width to prevent layout issues)
    const LEFT_SECTION_WIDTH: f32 = 240.0;
    const TEXT_MAX_WIDTH: f32 = 160.0;

    let song_info: Element<'static, Message> = if let Some(song) = current_song {
        let song_clone = song.clone();

        // Cover - clickable to open lyrics page
        let cover_content: Element<'static, Message> =
            if let Some(cover_path) = song.cover_path.clone() {
                // Skip URL covers - they need to be downloaded first
                if cover_path.starts_with("http://") || cover_path.starts_with("https://") {
                    // Show placeholder for URL covers (waiting for download)
                    container(
                        svg(svg::Handle::from_memory(icons::MUSIC.as_bytes()))
                            .width(24)
                            .height(24)
                            .style(|theme, _status| svg::Style {
                                color: Some(theme::icon_muted(theme)),
                            }),
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
                    })
                    .into()
                } else {
                    image(image::Handle::from_path(&cover_path))
                        .width(56)
                        .height(56)
                        .content_fit(iced::ContentFit::Cover)
                        .border_radius(4.0)
                        .into()
                }
            } else {
                container(
                    svg(svg::Handle::from_memory(icons::MUSIC.as_bytes()))
                        .width(24)
                        .height(24)
                        .style(|theme, _status| svg::Style {
                            color: Some(theme::icon_muted(theme)),
                        }),
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
                })
                .into()
            };

        let cover_btn = button(cover_content)
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
                    .size(14)
                    .style(|theme| text::Style {
                        color: Some(theme::text_primary(theme)),
                    })
                    .font(iced::Font {
                        weight: MEDIUM_WEIGHT,
                        ..Default::default()
                    })
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
        let artist_btn = button(
            container(
                text(song_clone.artist.clone())
                    .size(12)
                    .color(theme::TEXT_SECONDARY),
            )
            .max_width(TEXT_MAX_WIDTH)
            .clip(true),
        )
        .padding(0)
        .style(|_theme, _status| button::Style {
            background: Some(iced::Background::Color(Color::TRANSPARENT)),
            ..Default::default()
        })
        .on_press(Message::Noop);

        let song_details = column![title_btn, artist_btn].spacing(2);

        row![cover_btn, Space::new().width(12), song_details]
            .align_y(Alignment::Center)
            .into()
    } else {
        // Show placeholder when no song
        let placeholder = column![
            text("No song playing").size(14).color(theme::TEXT_MUTED),
            text("Select a song to play")
                .size(12)
                .color(theme::TEXT_MUTED),
        ]
        .spacing(2);

        row![
            container(
                svg(svg::Handle::from_memory(icons::MUSIC.as_bytes()))
                    .width(24)
                    .height(24)
                    .style(|theme, _status| svg::Style {
                        color: Some(theme::icon_muted(theme)),
                    })
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

    // Center section: Playback controls + progress (using unified widgets)
    let controls = widgets::playback_controls::view_with_buffering(
        is_playing,
        is_buffering,
        ControlSize::Small,
    );

    let progress_slider = widgets::progress_slider::view_with_download(
        position,
        download_progress,
        SliderSize::Standard,
    );

    let progress_row = row![
        text(current_time).size(12).color(theme::TEXT_MUTED),
        Space::new().width(8),
        progress_slider,
        Space::new().width(8),
        text(total_time).size(12).color(theme::TEXT_MUTED),
    ]
    .align_y(Alignment::Center);

    let center_section = column![controls, Space::new().height(4), progress_row,]
        .align_x(Alignment::Center)
        .width(Length::Fill);

    // Right section: Volume control (using unified widgets)
    let volume_icon = svg(svg::Handle::from_memory(icons::VOLUME.as_bytes()))
        .width(18)
        .height(18)
        .style(|_theme, _status| svg::Style {
            color: Some(theme::TEXT_SECONDARY),
        });

    let volume_slider = widgets::progress_slider::volume_slider(volume);

    // Play mode button (using unified widget)
    let play_mode_btn = widgets::play_mode_button::view(play_mode, PlayModeButtonSize::Small);

    // Queue button
    let queue_btn = button(
        svg(svg::Handle::from_memory(icons::QUEUE.as_bytes()))
            .width(18)
            .height(18)
            .style(|_theme, _status| svg::Style {
                color: Some(theme::TEXT_SECONDARY),
            }),
    )
    .padding(8)
    .style(|theme, status| {
        let bg = match status {
            button::Status::Hovered => theme::hover_bg(theme),
            _ => Color::TRANSPARENT,
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            border: iced::Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .on_press(Message::ToggleQueue);

    let right_section = row![
        play_mode_btn,
        Space::new().width(8),
        volume_icon,
        Space::new().width(8),
        volume_slider,
        Space::new().width(12),
        queue_btn,
    ]
    .align_y(Alignment::Center)
    .width(Length::Shrink);

    // Combine all sections
    let content = row![left_section, center_section, right_section,]
        .spacing(16)
        .align_y(Alignment::Center)
        .padding(Padding::new(12.0).left(16.0).right(16.0));

    // Top border line
    let top_border = container(Space::new().height(0))
        .width(Fill)
        .height(1)
        .style(|theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::divider(theme))),
            ..Default::default()
        });

    let main_content = container(content)
        .width(Fill)
        .height(PLAYER_BAR_HEIGHT - 1.0)
        .style(|theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::player_bar_bg(theme))),
            ..Default::default()
        });

    let bar = column![top_border, main_content]
        .width(Fill)
        .height(PLAYER_BAR_HEIGHT);

    // Use opaque + mouse_area to block all events from reaching underlying widgets
    let event_blocker = mouse_area(bar)
        .on_press(Message::Noop)
        .on_release(Message::Noop)
        .on_enter(Message::Noop)
        .on_exit(Message::Noop)
        .on_move(|_| Message::Noop);

    opaque(event_blocker).into()
}
