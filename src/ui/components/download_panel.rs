//! Download management panel — matching settings page layout conventions

use iced::widget::{Space, button, column, container, row, scrollable, svg, text};
use iced::{Alignment, Background, Color, Element, Length, Padding};

use crate::app::{DownloadTab, Message};
use crate::download::{DownloadManager, DownloadStatus, DownloadTask};
use crate::i18n::{Key, Locale};
use crate::ui::theme;

pub fn download_panel(
    locale: Locale,
    manager: &DownloadManager,
    tab: DownloadTab,
) -> Element<'static, Message> {
    let header = column![
        text(locale.get(Key::DownloadPanelTitle))
            .size(theme::TEXT_SIZE_HERO)
            .style(|theme| text::Style {
                color: Some(theme::text_primary(theme))
            }),
        text("管理你的本地离线音乐库")
            .size(theme::TEXT_SIZE_LABEL)
            .style(|theme| text::Style {
                color: Some(theme::text_muted(theme))
            }),
    ]
    .spacing(6);

    let active_count = manager.active.len();
    let completed_count = manager.completed.len();
    let is_active = tab == DownloadTab::Active;
    let tab_active = make_tab_button(
        format!("{} ({})", locale.get(Key::DownloadActive), active_count),
        is_active,
        Message::SwitchDownloadTab(DownloadTab::Active),
    );
    let tab_done = make_tab_button(
        format!(
            "{} ({})",
            locale.get(Key::DownloadCompleted),
            completed_count
        ),
        !is_active,
        Message::SwitchDownloadTab(DownloadTab::Completed),
    );

    let body: Element<'static, Message> = match tab {
        DownloadTab::Active => {
            if manager.active.is_empty() {
                empty_placeholder(locale.get(Key::DownloadNoActive).to_string())
            } else {
                column(
                    manager
                        .active
                        .iter()
                        .map(|t| build_active_card(t))
                        .collect::<Vec<_>>(),
                )
                .spacing(16)
                .into()
            }
        }
        DownloadTab::Completed => {
            if manager.completed.is_empty() {
                empty_placeholder(locale.get(Key::DownloadNoCompleted).to_string())
            } else {
                column(
                    manager
                        .completed
                        .iter()
                        .take(100)
                        .map(|t| build_completed_card(t))
                        .collect::<Vec<_>>(),
                )
                .spacing(8)
                .into()
            }
        }
    };

    // Fixed header — matches settings header_container
    let header_container = container(
        column![
            header,
            Space::new().height(24),
            row![tab_active, tab_done].spacing(0)
        ]
        .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding(
        Padding::new(40.0)
            .top(70.0)
            .right(32.0)
            .bottom(20.0)
            .left(32.0),
    )
    .style(|theme| container::Style {
        background: Some(Background::Color(theme::background(theme))),
        ..Default::default()
    });

    // Scrollable body — matches settings scrollable_content
    let scrollable_content = scrollable(
        container(body)
            .width(Length::Fill)
            .padding(Padding::new(20.0).right(32.0).bottom(60.0).left(32.0)),
    )
    .width(Length::Fill)
    .height(Length::Fill);

    container(
        column![header_container, scrollable_content]
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(theme::main_content)
    .into()
}

fn empty_placeholder(msg: String) -> Element<'static, Message> {
    container(
        text(msg)
            .size(theme::TEXT_SIZE_BODY)
            .style(|theme| text::Style {
                color: Some(theme::text_muted(theme)),
            }),
    )
    .width(Length::Fill)
    .center_x(Length::Fill)
    .padding(60)
    .into()
}

// ── Tabs (matches settings ACCENT_PINK style) ────────────────────────────────

fn make_tab_button(label: String, active: bool, msg: Message) -> Element<'static, Message> {
    let underline: Element<'static, Message> = if active {
        container(Space::new().height(2))
            .width(Length::Fill)
            .style(|_theme: &iced::Theme| container::Style {
                background: Some(Background::Color(theme::ACCENT_PINK)),
                ..Default::default()
            })
            .into()
    } else {
        Space::new().height(2).into()
    };
    column![
        button(
            text(label)
                .size(theme::TEXT_SIZE_BODY)
                .style(move |theme| text::Style {
                    color: Some(if active {
                        theme::ACCENT_PINK
                    } else {
                        theme::text_muted(theme)
                    }),
                }),
        )
        .on_press(msg)
        .padding([12, 0])
        .style(|theme, status| {
            let bg = if status == button::Status::Hovered {
                Background::Color(Color::from_rgba(1.0, 0.08, 0.55, 0.05))
            } else {
                Background::Color(Color::TRANSPARENT)
            };
            button::Style {
                background: Some(bg),
                ..Default::default()
            }
        }),
        underline,
    ]
    .width(90)
    .align_x(Alignment::Center)
    .into()
}

// ── Active download card ──────────────────────────────────────────────────

fn build_active_card(task: &DownloadTask) -> Element<'static, Message> {
    let (progress, _speed) = match &task.status {
        DownloadStatus::Active { progress, speed } => (*progress, speed.clone()),
        _ => (0.0, String::new()),
    };
    let title = task.metadata.title.clone();
    let artist = task.metadata.artist.clone();
    let song_id = task.song_id;

    let s = crate::ui::components::cover_thumb::CoverSize::Medium;
    let cover_handle = crate::ui::components::cover_thumb::resolve_song_cover(task.song_id);
    let cover = crate::ui::components::cover_thumb::thumb(
        cover_handle.as_ref(), s.px(), s.radius(),
    );

    let info = column![
        text(title)
            .size(theme::TEXT_SIZE_BODY_LARGE)
            .style(|theme| text::Style {
                color: Some(theme::text_primary(theme))
            }),
        text(artist)
            .size(theme::TEXT_SIZE_LABEL)
            .style(|theme| text::Style {
                color: Some(theme::text_secondary(theme))
            }),
    ]
    .spacing(2);

    // Responsive progress bar with FillPortion
    let pct = (progress * 100.0) as u32;
    let fill_portion = ((progress.max(0.01).min(1.0)) * 1000.0) as u16;
    let empty_portion = 1000u16.saturating_sub(fill_portion);
    let bar = container(
        row![
            container(Space::new().height(4))
                .width(Length::FillPortion(fill_portion))
                .style(|_theme| container::Style {
                    background: Some(Background::Color(theme::ACCENT_PINK)),
                    ..Default::default()
                }),
            Space::new().width(Length::FillPortion(empty_portion)),
        ]
        .width(Length::Fill),
    )
    .style(|_theme| container::Style {
        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.06))),
        border: iced::Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    let downloaded_mb = progress * task.file_size as f32 / 1_048_576.0;
    let total_mb = task.file_size as f32 / 1_048_576.0;
    let progress_info = column![
        row![
            text(format!("{:.1} MB / {:.1} MB", downloaded_mb, total_mb))
                .size(theme::TEXT_SIZE_CAPTION)
                .style(|theme| text::Style {
                    color: Some(theme::text_muted(theme))
                }),
            Space::new().width(Length::Fill),
            text(format!("{}%", pct))
                .size(theme::TEXT_SIZE_CAPTION)
                .style(|theme| text::Style {
                    color: Some(theme::text_muted(theme))
                }),
        ],
        Space::new().height(4),
        bar,
    ]
    .spacing(2);

    let cancel_btn = icon_danger_btn(song_id);

    container(
        row![
            cover,
            column![info, progress_info].spacing(10).width(Length::Fill),
            cancel_btn,
        ]
        .align_y(Alignment::Center)
        .spacing(16),
    )
    .padding(16)
    .style(|theme| container::Style {
        background: Some(Background::Color(theme::panel_bg(theme))),
        border: iced::Border {
            radius: 12.0.into(),
            width: 1.0,
            color: theme::panel_border(theme),
        },
        ..Default::default()
    })
    .into()
}

// ── Completed download card ───────────────────────────────────────────────

fn build_completed_card(task: &DownloadTask) -> Element<'static, Message> {
    let title = task.metadata.title.clone();
    let artist = task.metadata.artist.clone();
    let quality = task.quality;
    let size_str = format_size(task.file_size);
    let song_id = task.song_id;

    // For completed downloads, the file exists on disk — pass the path
    // so cover can be extracted from embedded tags if not cached.
    let audio_path = match &task.status {
        DownloadStatus::Completed(p) => Some(p.as_path()),
        _ => None,
    };

    let quality_badge: Option<Element<'static, Message>> = match quality {
        crate::features::settings::MusicQuality::Lossless => Some(quality_badge_el(
            "FLAC",
            Color::from_rgb(168.0 / 255.0, 85.0 / 255.0, 247.0 / 255.0),
        )),
        crate::features::settings::MusicQuality::HiRes => {
            Some(quality_badge_el("Hi-Res", Color::from_rgb(1.0, 0.76, 0.03)))
        }
        _ => None,
    };

    let title_row = if let Some(badge) = quality_badge {
        row![
            text(title)
                .size(theme::TEXT_SIZE_BODY)
                .style(|theme| text::Style {
                    color: Some(theme::text_primary(theme))
                }),
            badge,
        ]
        .spacing(8)
        .align_y(Alignment::Center)
    } else {
        row![
            text(title)
                .size(theme::TEXT_SIZE_BODY)
                .style(|theme| text::Style {
                    color: Some(theme::text_primary(theme))
                })
        ]
    };

    // Action buttons using SVG icons
    let actions = row![
        svg_btn(crate::ui::icons::PLAY, Message::PlaySong(song_id)),
        svg_btn(crate::ui::icons::EDIT, Message::EditSongTags(song_id)),
        svg_btn(crate::ui::icons::TRASH, Message::DeleteDownloadHistory(song_id)),
    ]
    .spacing(4);

    let s = crate::ui::components::cover_thumb::CoverSize::Medium;
    let cover_handle = crate::ui::components::cover_thumb::resolve_song_cover(task.song_id);
    let cover = crate::ui::components::cover_thumb::thumb(
        cover_handle.as_ref(), s.px(), s.radius(),
    );

    container(
        row![
            cover,
            column![
                title_row,
                row![
                    text(artist)
                        .size(theme::TEXT_SIZE_LABEL)
                        .style(|theme| text::Style {
                            color: Some(theme::text_muted(theme))
                        }),
                    text("·")
                        .size(theme::TEXT_SIZE_CAPTION)
                        .style(|theme| text::Style {
                            color: Some(theme::text_muted(theme))
                        }),
                    text(size_str)
                        .size(theme::TEXT_SIZE_CAPTION)
                        .style(|theme| text::Style {
                            color: Some(theme::text_muted(theme))
                        }),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            ]
            .spacing(4)
            .width(Length::Fill),
            actions,
        ]
        .align_y(Alignment::Center)
        .spacing(16),
    )
    .padding(12)
    .style(|theme| container::Style {
        background: Some(Background::Color(theme::panel_bg(theme))),
        border: iced::Border {
            radius: 12.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    })
    .into()
}

fn quality_badge_el(label: &'static str, color: Color) -> Element<'static, Message> {
    let (r, g, b) = (color.r, color.g, color.b);
    container(text(label).size(theme::TEXT_SIZE_MICRO).color(color))
        .padding(Padding::new(1.0).left(6.0).right(6.0))
        .style(move |_theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(r, g, b, 0.15))),
            border: iced::Border {
                radius: 4.0.into(),
                width: 1.0,
                color: Color::from_rgba(r, g, b, 0.25),
            },
            ..Default::default()
        })
        .into()
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f32 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f32 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

// ── Button helpers (SVG icons, no emoji) ─────────────────────────────────

fn svg_btn(icon_svg: &'static str, msg: Message) -> Element<'static, Message> {
    button(
        svg(svg::Handle::from_memory(icon_svg.as_bytes()))
            .width(14)
            .height(14)
            .style(|theme, _status| svg::Style {
                color: Some(theme::text_secondary(theme)),
            }),
    )
    .on_press(msg)
    .padding([6, 10])
    .style(|theme, status| {
        let bg = if status == button::Status::Hovered {
            Background::Color(theme::surface(theme))
        } else {
            Background::Color(Color::TRANSPARENT)
        };
        button::Style {
            background: Some(bg),
            border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .into()
}

fn icon_danger_btn(song_id: i64) -> Element<'static, Message> {
    button(
        svg(svg::Handle::from_memory(crate::ui::icons::CLOSE.as_bytes()))
            .width(16)
            .height(16)
            .style(|_theme, status| svg::Style {
                color: Some(if status == svg::Status::Hovered {
                    Color::from_rgb(0.94, 0.34, 0.34)
                } else {
                    Color::from_rgba(0.5, 0.5, 0.5, 0.8)
                }),
            }),
    )
    .on_press(Message::DownloadCancel(song_id))
    .width(40)
    .height(40)
    .style(|_theme, status| {
        let bg = if status == button::Status::Hovered {
            Background::Color(Color::from_rgba(0.94, 0.34, 0.34, 0.1))
        } else {
            Background::Color(Color::TRANSPARENT)
        };
        button::Style {
            background: Some(bg),
            border: iced::Border {
                radius: 100.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .into()
}
