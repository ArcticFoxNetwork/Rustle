//! Importing playlist card component
//!
//! Shows a playlist card during import with circular progress indicator.
//! This is a business-specific component that uses the generic ProgressRing widget.

use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Fill, Padding};

use crate::app::Message;
use crate::ui::theme;
use crate::ui::widgets::{ProgressRing, view_progress_ring_styled};

/// State for an importing playlist
#[derive(Debug, Clone)]
pub struct ImportingPlaylist {
    /// Playlist name (folder name)
    pub name: String,
    /// Cover image path (first song's cover, if available)
    pub cover_path: Option<String>,
    /// Current progress (0.0 - 1.0)
    pub progress: f32,
    /// Current file count
    pub current: u64,
    /// Total file count
    pub total: u64,
    /// Is import complete
    pub completed: bool,
    /// Database ID of created playlist (set after completion)
    pub playlist_id: Option<i64>,
}

impl ImportingPlaylist {
    pub fn new(name: String) -> Self {
        Self {
            name,
            cover_path: None,
            progress: 0.0,
            current: 0,
            total: 0,
            completed: false,
            playlist_id: None,
        }
    }

    pub fn update_progress(&mut self, current: u64, total: u64) {
        self.current = current;
        self.total = total;
        self.progress = if total > 0 {
            current as f32 / total as f32
        } else {
            0.0
        };
    }

    pub fn set_cover(&mut self, path: String) {
        if self.cover_path.is_none() {
            self.cover_path = Some(path);
        }
    }

    pub fn complete(&mut self) {
        self.completed = true;
        self.progress = 1.0;
    }

    pub fn is_in_progress(&self) -> bool {
        !self.completed && self.total > 0
    }
}

/// Build an importing playlist card for the sidebar
pub fn view(playlist: &ImportingPlaylist) -> Element<'static, Message> {
    let name = playlist.name.clone();
    let progress = playlist.progress;
    let percentage = (progress * 100.0) as u32;

    // Progress indicator - show checkmark when completed, progress ring otherwise
    let progress_indicator: Element<'static, Message> = if playlist.completed {
        // Show checkmark icon when completed
        container(
            iced::widget::svg(iced::widget::svg::Handle::from_memory(
                crate::ui::icons::CHECK.as_bytes(),
            ))
            .width(18)
            .height(18)
            .style(|_theme, _status| iced::widget::svg::Style {
                color: Some(theme::ACCENT_PINK),
            }),
        )
        .width(18)
        .height(18)
        .center_x(18)
        .center_y(18)
        .into()
    } else {
        // Show progress ring with percentage during import
        let progress_ring = ProgressRing::new(progress)
            .stroke_width(2.5)
            .background_color(theme::SURFACE_LIGHT)
            .progress_color(theme::ACCENT_PINK);

        container(
            column![
                view_progress_ring_styled(progress_ring, 28.0),
                text(format!("{}%", percentage))
                    .size(8)
                    .color(theme::TEXT_MUTED)
            ]
            .align_x(Alignment::Center)
            .spacing(1),
        )
        .width(32)
        .center_x(32)
        .center_y(32)
        .into()
    };

    // Playlist info
    let status_text = if playlist.completed {
        "导入完成".to_string()
    } else if playlist.total > 0 {
        format!("{}/{}", playlist.current, playlist.total)
    } else {
        "扫描中...".to_string()
    };

    let completed = playlist.completed;
    let info = column![
        text(name)
            .size(13)
            .style(move |theme| text::Style {
                color: Some(if completed {
                    theme::text_primary(theme)
                } else {
                    theme::TEXT_SECONDARY
                })
            })
            .font(iced::Font::with_name("Inter")),
        text(status_text).size(11).color(theme::TEXT_MUTED)
    ]
    .spacing(2);

    let content = row![progress_indicator, Space::new().width(12), info,]
        .align_y(Alignment::Center)
        .padding(Padding::new(10.0).left(14.0).right(14.0));

    // Make it a button only if completed
    if playlist.completed {
        button(content)
            .width(Fill)
            .style(theme::nav_item)
            .on_press(Message::PlayHero)
            .into()
    } else {
        // Non-clickable during import
        container(content)
            .width(Fill)
            .style(|_theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
                ..Default::default()
            })
            .into()
    }
}
