//! Lyrics page - full screen lyrics display with Apple Music-like styling
//!
//! Layout:
//! - Left panel: Cover art, song title, artist, progress bar, playback controls
//! - Right panel: Scrollable lyrics with current line highlighted

use std::sync::Arc;

use iced::widget::{Space, button, column, container, mouse_area, opaque, row, shader, svg, text};
use iced::{Alignment, Color, Element, Fill, Length, Padding};

use crate::app::Message;
use crate::database::DbSong;
use crate::features::PlayMode;
use crate::features::lyrics::engine::{LyricLineData, LyricsEngine};
use crate::ui::effects::textured_background::TexturedBackgroundProgram;
use crate::ui::icons;
use crate::ui::theme::{self, BOLD_WEIGHT};
use crate::ui::widgets::{self, ControlSize, PlayModeButtonSize, SliderSize};

/// Build the lyrics page view
///
/// `animation_progress`: 0.0 = hidden at bottom, 1.0 = fully visible
/// `cached_engine_lines`: Pre-computed engine lines (Arc for O(1) clone, thread-safe)
/// `power_saving_mode`: When true, use simple text rendering instead of SDF engine
/// `is_liked`: Whether the current song is in user's favorites
/// `download_progress`: Download progress for streaming songs (0.0 to 1.0)
/// `is_fm_mode`: Whether in Personal FM mode
pub fn view<'a>(
    song: &'a DbSong,
    is_playing: bool,
    position: f32, // 0.0 to 1.0
    duration_secs: f32,
    cached_engine_lines: Option<&Arc<Vec<LyricLineData>>>,
    _current_line_index: Option<usize>,
    play_mode: PlayMode,
    animation_progress: f32,
    _bg_colors: &crate::utils::DominantColors,
    _bg_shader: &'a crate::ui::effects::background::LyricsBackgroundProgram,
    textured_bg_shader: &'a TexturedBackgroundProgram,
    lyrics_engine: Option<&'a std::cell::RefCell<LyricsEngine>>,
    power_saving_mode: bool,
    is_liked: bool,
    download_progress: Option<f32>,
    is_fm_mode: bool,
) -> Element<'a, Message> {
    let left_panel = build_left_panel(
        song,
        is_playing,
        position,
        duration_secs,
        play_mode,
        is_liked,
        download_progress,
        is_fm_mode,
    );
    let right_panel = if power_saving_mode {
        // Power saving mode: use simple text rendering
        build_simple_lyrics_panel(cached_engine_lines, position * duration_secs * 1000.0)
    } else {
        build_right_panel_engine(
            cached_engine_lines,
            lyrics_engine,
            position * duration_secs * 1000.0,
        )
    };

    // Main layout: left panel (40%) + right panel (60%)
    let content_row = row![
        container(left_panel)
            .width(Length::FillPortion(4))
            .height(Fill)
            .padding(40),
        container(right_panel)
            .width(Length::FillPortion(6))
            .height(Fill)
            .padding(Padding::new(40.0).left(20.0)),
    ]
    .width(Fill)
    .height(Fill);

    let content = container(content_row).width(Fill).height(Fill);

    // Back button overlay in top-left corner
    let back_btn = button(
        svg(svg::Handle::from_memory(icons::CHEVRON_DOWN.as_bytes()))
            .width(24)
            .height(24)
            .style(|_theme, _status| svg::Style {
                color: Some(theme::TEXT_PRIMARY),
            }),
    )
    .padding(12)
    .style(|theme, status| {
        let bg = match status {
            button::Status::Hovered => theme::hover_bg(theme),
            _ => Color::TRANSPARENT,
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            border: iced::Border {
                radius: 24.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .on_press(Message::CloseLyricsPage);

    // Top-right buttons - using unified window control styles
    let icon_btn_style = |_theme: &iced::Theme, status: button::Status| {
        let base = button::Style {
            background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
            text_color: theme::TEXT_PRIMARY,
            border: iced::Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            shadow: iced::Shadow::default(),
            snap: true,
        };

        match status {
            button::Status::Hovered => button::Style {
                background: Some(iced::Background::Color(Color::from_rgba(
                    1.0, 1.0, 1.0, 0.1,
                ))),
                text_color: theme::TEXT_PRIMARY,
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(iced::Background::Color(Color::from_rgba(
                    1.0, 1.0, 1.0, 0.2,
                ))),
                ..base
            },
            _ => base,
        }
    };

    let close_btn_style = |_theme: &iced::Theme, status: button::Status| {
        let base = button::Style {
            background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
            text_color: theme::TEXT_PRIMARY,
            border: iced::Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            shadow: iced::Shadow::default(),
            snap: true,
        };

        match status {
            button::Status::Hovered => button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.8, 0.2, 0.2,
                ))),
                text_color: theme::TEXT_PRIMARY,
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.6, 0.15, 0.15,
                ))),
                text_color: theme::TEXT_PRIMARY,
                ..base
            },
            _ => base,
        }
    };

    let settings_btn = button(
        svg(svg::Handle::from_memory(icons::SETTINGS.as_bytes()))
            .width(14)
            .height(14)
            .style(|_theme, _status| svg::Style {
                color: Some(theme::TEXT_PRIMARY),
            }),
    )
    .width(32)
    .height(32)
    .style(icon_btn_style)
    .on_press(Message::OpenSettingsWithCloseLyrics);

    let minimize_btn = button(
        svg(svg::Handle::from_memory(icons::MINIMIZE.as_bytes()))
            .width(14)
            .height(14)
            .style(|_theme, _status| svg::Style {
                color: Some(theme::TEXT_PRIMARY),
            }),
    )
    .width(32)
    .height(32)
    .style(icon_btn_style)
    .on_press(Message::WindowMinimize);

    let maximize_btn = button(
        svg(svg::Handle::from_memory(icons::MAXIMIZE.as_bytes()))
            .width(14)
            .height(14)
            .style(|_theme, _status| svg::Style {
                color: Some(theme::TEXT_PRIMARY),
            }),
    )
    .width(32)
    .height(32)
    .style(icon_btn_style)
    .on_press(Message::WindowMaximize);

    let close_btn = button(
        svg(svg::Handle::from_memory(icons::CLOSE.as_bytes()))
            .width(14)
            .height(14)
            .style(|_theme, _status| svg::Style {
                color: Some(theme::TEXT_PRIMARY),
            }),
    )
    .width(32)
    .height(32)
    .style(close_btn_style)
    .on_press(Message::RequestClose);

    let top_right_buttons = row![
        settings_btn,
        Space::new().width(4),
        minimize_btn,
        Space::new().width(4),
        maximize_btn,
        Space::new().width(4),
        close_btn,
    ]
    .align_y(Alignment::Center);

    let top_bar = row![back_btn, Space::new().width(Fill), top_right_buttons,]
        .align_y(Alignment::Center)
        .padding(20);

    let content_with_overlay = iced::widget::stack![content, container(top_bar).width(Fill),]
        .width(Fill)
        .height(Fill);

    // Apply slide animation
    let slide_offset = (1.0 - animation_progress) * 30.0;

    // Use WGPU textured shader for animated background
    let shader_bg = shader(textured_bg_shader).width(Fill).height(Fill);

    let content_with_shader = iced::widget::stack![
        shader_bg,
        container(content_with_overlay)
            .width(Fill)
            .height(Fill)
            .padding(Padding::new(0.0).top(slide_offset)),
    ]
    .width(Fill)
    .height(Fill);

    // Use opaque to block events from reaching underlying widgets (main content behind lyrics page)
    // opaque only blocks events from propagating to widgets BELOW it in the stack,
    // it does NOT block events to widgets INSIDE it (like our buttons)
    opaque(container(content_with_shader).width(Fill).height(Fill)).into()
}

/// Build the left panel with cover, song info, and controls
fn build_left_panel<'a>(
    song: &'a DbSong,
    is_playing: bool,
    position: f32,
    duration_secs: f32,
    play_mode: PlayMode,
    is_liked: bool,
    download_progress: Option<f32>,
    is_fm_mode: bool,
) -> Element<'a, Message> {
    // Format time as mm:ss
    let format_time = |secs: f32| -> String {
        let mins = (secs / 60.0) as u32;
        let secs = (secs % 60.0) as u32;
        format!("{}:{:02}", mins, secs)
    };

    let current_time = format_time(position * duration_secs);
    let total_time = format_time(duration_secs);

    // Cover image - square aspect ratio using custom widget
    let cover = widgets::square_cover::view(song.cover_path.as_deref());

    // Song title
    let title = text(&song.title)
        .size(28)
        .color(theme::TEXT_PRIMARY)
        .font(iced::Font {
            weight: BOLD_WEIGHT,
            ..Default::default()
        });

    // Artist name
    let artist = text(&song.artist).size(18).color(theme::TEXT_SECONDARY);

    // Progress bar - using unified widget with download progress
    let progress_slider =
        widgets::progress_slider::view_with_download(position, download_progress, SliderSize::Full);

    let time_row = row![
        text(current_time).size(12).color(theme::TEXT_MUTED),
        Space::new().width(Fill),
        text(total_time).size(12).color(theme::TEXT_MUTED),
    ]
    .width(Fill);

    // Playback controls - using unified widgets
    let playback_controls = widgets::playback_controls::view(is_playing, ControlSize::Large);

    // Play mode button - using unified widget
    let play_mode_btn =
        widgets::play_mode_button::view(play_mode, PlayModeButtonSize::Large, is_fm_mode);

    // Like button - only for NCM songs (negative ID)
    let like_btn: Element<'a, Message> = if song.id < 0 {
        let ncm_id = (-song.id) as u64;
        let heart_icon = if is_liked {
            icons::HEART
        } else {
            icons::HEART_OUTLINE
        };
        let heart_color = if is_liked {
            theme::ACCENT_PINK
        } else {
            theme::TEXT_SECONDARY
        };
        button(
            svg(svg::Handle::from_memory(heart_icon.as_bytes()))
                .width(22)
                .height(22)
                .style(move |_theme, _status| svg::Style {
                    color: Some(heart_color),
                }),
        )
        .padding(10)
        .style(|theme, status| {
            let bg = match status {
                button::Status::Hovered => theme::hover_bg(theme),
                _ => Color::TRANSPARENT,
            };
            button::Style {
                background: Some(iced::Background::Color(bg)),
                border: iced::Border {
                    radius: 21.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .on_press(Message::ToggleFavorite(ncm_id))
        .into()
    } else {
        // Local songs - show disabled like button
        button(
            svg(svg::Handle::from_memory(icons::HEART.as_bytes()))
                .width(22)
                .height(22)
                .style(|_theme, _status| svg::Style {
                    color: Some(theme::icon_muted(&iced::Theme::Dark)),
                }),
        )
        .padding(10)
        .style(|_theme, _status| button::Style {
            background: Some(iced::Background::Color(Color::TRANSPARENT)),
            border: iced::Border {
                radius: 21.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
    };

    let controls = row![
        play_mode_btn,
        Space::new().width(Fill),
        playback_controls,
        Space::new().width(Fill),
        like_btn,
    ]
    .align_y(Alignment::Center)
    .width(Fill);

    // Content container with max width
    let content = column![
        cover,
        Space::new().height(24),
        title,
        Space::new().height(4),
        artist,
        Space::new().height(24),
        progress_slider,
        Space::new().height(4),
        time_row,
        Space::new().height(20),
        controls,
    ]
    .width(Fill)
    .max_width(400);

    // Assemble left panel - center the content vertically
    column![
        Space::new().height(Fill),
        content,
        Space::new().height(Fill),
    ]
    .align_x(Alignment::Center)
    .width(Fill)
    .height(Fill)
    .into()
}

/// Build the right panel with the Apple Music-style engine
/// Uses pre-computed cached_engine_lines to avoid per-frame conversion
fn build_right_panel_engine<'a>(
    cached_engine_lines: Option<&Arc<Vec<LyricLineData>>>,
    lyrics_engine: Option<&'a std::cell::RefCell<LyricsEngine>>,
    current_time_ms: f32,
) -> Element<'a, Message> {
    // Check if we have cached engine lines
    let engine_lines = match cached_engine_lines {
        Some(arc) if !arc.is_empty() => arc,
        _ => {
            // No lyrics or empty - show placeholder
            return container(
                column![
                    svg(svg::Handle::from_memory(icons::MUSIC.as_bytes()))
                        .width(64)
                        .height(64)
                        .style(|_theme: &iced::Theme, _status| svg::Style {
                            color: Some(theme::icon_muted(&iced::Theme::Dark)),
                        }),
                    Space::new().height(16),
                    text("纯音乐，请欣赏").size(18).color(theme::TEXT_MUTED),
                ]
                .align_x(Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
        }
    };

    // Use provided engine - if none, show a simple fallback
    if let Some(engine_cell) = lyrics_engine {
        // Build primitive with current animation state
        // Arc clone is O(1) - no data copying needed
        let primitive =
            crate::features::lyrics::engine::pipeline::LyricsEnginePrimitive::from_engine(
                &mut engine_cell.borrow_mut(),
                engine_lines.clone(), // Arc clone is O(1)
                current_time_ms,
            );

        let content =
            shader(crate::features::lyrics::engine::program::LyricsEngineProgram::new(primitive))
                .width(Length::Fill)
                .height(Length::Fill);

        mouse_area(content)
            .on_scroll(|delta| {
                let scroll_amount = match delta {
                    iced::mouse::ScrollDelta::Lines { y, .. } => y * 40.0,
                    iced::mouse::ScrollDelta::Pixels { y, .. } => y,
                };
                Message::LyricsScroll(-scroll_amount)
            })
            .into()
    } else {
        // Fallback: show simple text-based lyrics without engine
        build_simple_lyrics_panel_from_engine_lines(engine_lines, current_time_ms)
    }
}

/// Simple fallback lyrics panel when engine is not available
/// Uses pre-computed engine lines
fn build_simple_lyrics_panel_from_engine_lines(
    engine_lines: &[LyricLineData],
    current_time_ms: f32,
) -> Element<'static, Message> {
    let current_time = current_time_ms as u64;

    // Find current line
    let current_idx = engine_lines
        .iter()
        .position(|line| line.start_ms <= current_time && line.end_ms > current_time);

    // Clone text to owned strings for the UI elements
    let lines_data: Vec<(bool, String)> = engine_lines
        .iter()
        .enumerate()
        .map(|(idx, line)| {
            let is_active = Some(idx) == current_idx;
            (is_active, line.text.clone())
        })
        .collect();

    let lines_column: Element<'static, Message> = column(
        lines_data
            .into_iter()
            .map(|(is_active, line_text)| {
                let opacity = if is_active { 1.0 } else { 0.5 };
                let size = if is_active { 28 } else { 22 };

                text(line_text)
                    .size(size)
                    .color(Color::from_rgba(1.0, 1.0, 1.0, opacity))
                    .into()
            })
            .collect::<Vec<_>>(),
    )
    .spacing(12)
    .align_x(Alignment::Start)
    .into();

    container(lines_column)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(20)
        .into()
}

/// Simple lyrics panel for power saving mode
/// Uses plain text rendering instead of SDF engine
fn build_simple_lyrics_panel(
    cached_engine_lines: Option<&Arc<Vec<LyricLineData>>>,
    current_time_ms: f32,
) -> Element<'static, Message> {
    // Check if we have cached engine lines
    let engine_lines = match cached_engine_lines {
        Some(arc) if !arc.is_empty() => arc,
        _ => {
            // No lyrics or empty - show placeholder
            return container(
                column![
                    svg(svg::Handle::from_memory(icons::MUSIC.as_bytes()))
                        .width(64)
                        .height(64)
                        .style(|_theme: &iced::Theme, _status| svg::Style {
                            color: Some(theme::icon_muted(&iced::Theme::Dark)),
                        }),
                    Space::new().height(16),
                    text("暂无歌词").size(18).color(theme::TEXT_MUTED),
                ]
                .align_x(Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
        }
    };

    let current_time = current_time_ms as u64;

    // Find current line index
    let current_idx = engine_lines
        .iter()
        .position(|line| line.start_ms <= current_time && line.end_ms > current_time);

    // Build scrollable lyrics with current line highlighted
    let lines_data: Vec<(bool, String, Option<String>)> = engine_lines
        .iter()
        .enumerate()
        .map(|(idx, line)| {
            let is_active = Some(idx) == current_idx;
            (is_active, line.text.clone(), line.translated.clone())
        })
        .collect();

    // Power saving mode: no scrollable, just a simple column with larger fonts
    let lines_column: Element<'static, Message> = column(
        lines_data
            .into_iter()
            .map(|(is_active, line_text, translated)| {
                let opacity = if is_active { 1.0 } else { 0.35 };
                let size = if is_active { 36 } else { 28 };
                let weight = if is_active {
                    BOLD_WEIGHT
                } else {
                    iced::font::Weight::Normal
                };

                let main_text = text(line_text)
                    .size(size)
                    .color(Color::from_rgba(1.0, 1.0, 1.0, opacity))
                    .font(iced::Font {
                        weight,
                        ..Default::default()
                    });

                if let Some(trans) = translated {
                    column![
                        main_text,
                        text(trans)
                            .size(18)
                            .color(Color::from_rgba(1.0, 1.0, 1.0, opacity * 0.7))
                    ]
                    .spacing(6)
                    .into()
                } else {
                    main_text.into()
                }
            })
            .collect::<Vec<_>>(),
    )
    .spacing(24)
    .padding(iced::Padding::new(40.0).top(200.0).bottom(300.0))
    .align_x(Alignment::Start)
    .into();

    // No background, no scrollbar - just the lyrics
    container(lines_column)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// A single word in a lyric line (for word-by-word sync)
#[derive(Debug, Clone)]
pub struct LyricWord {
    pub start_ms: u64,
    pub end_ms: u64,
    pub word: String,
}

/// A single line of lyrics with timestamp
#[derive(Debug, Clone)]
pub struct LyricLine {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub words: Vec<LyricWord>,
    pub translated: Option<String>,
    pub romanized: Option<String>,
    pub is_background: bool,
    pub is_duet: bool,
}

impl LyricLine {
    pub fn with_end(start_ms: u64, end_ms: u64, text: String) -> Self {
        Self {
            start_ms,
            end_ms,
            text,
            words: Vec::new(),
            translated: None,
            romanized: None,
            is_background: false,
            is_duet: false,
        }
    }
}

/// Find the current lyric line index based on playback position
pub fn find_current_line(lyrics: &[LyricLine], position_ms: u64) -> Option<usize> {
    if lyrics.is_empty() {
        return None;
    }

    let mut current = None;
    for (idx, line) in lyrics.iter().enumerate() {
        if line.start_ms <= position_ms {
            current = Some(idx);
        } else {
            break;
        }
    }

    current
}
