//! Overlay system — unified abstraction for modals, popups, and floating panels.
//!
//! # Architecture
//!
//! The overlay module provides:
//! - **backdrop**: Semi-transparent full-screen click-to-dismiss layer with event isolation
//! - **modal**: Unified centered modal dialog component (backdrop + panel + opaque)
//!
//! # Event Isolation
//!
//! All overlays use `iced::widget::opaque()` to prevent cursor, click, and scroll events
//! from penetrating through to widgets beneath.

use iced::border::Radius;
use iced::widget::{button, column, container, mouse_area, opaque, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Shadow};

use crate::app::Message;
use crate::ui::theme;

// ============================================================================
// ModalKind — Enum covering all modal dialog types with typed payloads
// ============================================================================

/// Each variant holds the specific data needed to render and handle that modal.
#[derive(Debug, Clone)]
pub enum ModalKind {
    /// Editable song metadata form (title, artist, album, year, etc.).
    SongEdit(crate::app::SongEditDialogState),
    /// Edit playlist form (name, description, cover, watch folder).
    PlaylistEdit {
        playlist_id: i64,
        name: String,
        description: String,
        cover_path: Option<String>,
        watch_enabled: bool,
        watch_available: bool,
        watch_path: Option<String>,
    },
    /// Delete playlist confirmation prompt.
    DeleteConfirm {
        playlist_id: i64,
        playlist_name: String,
    },
    /// Batch download confirmation prompt.
    DownloadConfirm {
        playlist_id: i64,
        playlist_name: String,
        song_count: u32,
    },
    /// Exit application confirmation prompt.
    ExitConfirm { remember_choice: bool },
    /// Pick a target playlist for adding a song.
    PlaylistPicker {
        song_id: i64,
        /// NCM online playlists (user's created + subscribed); None for local songs
        ncm_playlists: Option<Vec<crate::api::PlaylistSummary>>,
    },
}

// ============================================================================
// OverlayKind + OverlayEntry — Stack-based overlay management
// ============================================================================

/// The kind of overlay currently active.
#[derive(Debug, Clone)]
pub enum OverlayKind {
    Modal(ModalKind, ModalConfig),
}

/// A single entry in the overlay stack (LIFO: last = topmost).
#[derive(Debug, Clone)]
pub struct OverlayEntry {
    pub kind: OverlayKind,
}

impl OverlayEntry {
    pub fn new(kind: OverlayKind) -> Self {
        Self { kind }
    }
}

// ============================================================================
// Backdrop — Semi-transparent event-blocking layer
// ============================================================================

fn backdrop_color() -> Color {
    Color::from_rgba(0.0, 0.0, 0.0, 0.6)
}

// ============================================================================
// Modal — Unified centered dialog component
// ============================================================================

/// Configuration for a modal dialog.
#[derive(Debug, Clone)]
pub struct ModalConfig {
    /// Width of the content panel in logical pixels.
    pub width: f32,
    /// Whether clicking the backdrop dismisses the modal.
    pub backdrop_dismiss: bool,
    /// Whether pressing Escape dismisses the modal.
    pub escape_close: bool,
    /// Border radius of the content panel.
    pub border_radius: f32,
}

impl Default for ModalConfig {
    fn default() -> Self {
        Self {
            width: 480.0,
            backdrop_dismiss: true,
            escape_close: true,
            border_radius: 16.0,
        }
    }
}

impl ModalConfig {
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn no_backdrop_dismiss(mut self) -> Self {
        self.backdrop_dismiss = false;
        self
    }
}

/// Render a unified modal dialog.
pub fn modal_view<'a>(
    config: &ModalConfig,
    content: Element<'a, Message>,
    on_dismiss: Message,
) -> Element<'a, Message> {
    let bg = backdrop_color();
    let border_radius = config.border_radius;

    let panel = opaque(
        container(content)
            .width(Length::Fixed(config.width))
            .style(move |t| container::Style {
                background: Some(Background::Color(theme::surface(t))),
                border: Border {
                    color: theme::divider(t),
                    width: 1.0,
                    radius: Radius::new(border_radius),
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
                    offset: iced::Vector::new(0.0, 8.0),
                    blur_radius: 32.0,
                },
                ..Default::default()
            }),
    );

    let backdrop_container = container(panel)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(bg)),
            ..Default::default()
        });

    let inner: Element<'a, Message> = if config.backdrop_dismiss {
        mouse_area(backdrop_container).on_press(on_dismiss).into()
    } else {
        backdrop_container.into()
    };

    opaque(inner)
}

// ============================================================================
// Unified Modal Layout — Header / Body / Footer with dividers
// ============================================================================

/// Header bar: title left, close button (✕) right.
pub fn modal_header(title: String, on_close: Message) -> Element<'static, Message> {
    container(
        row![
            text(title).size(14.0).style(|theme| text::Style {
                color: Some(theme::text_primary(theme))
            }),
            iced::widget::Space::new().width(Length::Fill),
            button(text("✕").size(16.0).style(|theme| text::Style {
                color: Some(theme::text_muted(theme))
            }))
            .style(close_btn_style)
            .padding([6, 10])
            .on_press(on_close),
        ]
        .align_y(Alignment::Center)
        .padding([14, 20]),
    )
    .into()
}

/// Footer bar: divider line + right-aligned buttons.
pub fn modal_footer<'a>(buttons: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let mut row_children: Vec<Element<'a, Message>> =
        vec![iced::widget::Space::new().width(Length::Fill).into()];
    row_children.extend(buttons);
    let btn_row = row(row_children)
        .align_y(Alignment::Center)
        .spacing(12)
        .padding([14, 20]);
    column![divider(), btn_row].into()
}

/// Content area with standard padding.
pub fn modal_body<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    container(content).padding(24).into()
}

/// Full modal layout: header + divider + body + footer.
pub fn modal_section<'a>(
    title: String,
    on_close: Message,
    body: Element<'a, Message>,
    footer: Element<'a, Message>,
) -> Element<'a, Message> {
    column![modal_header(title, on_close), divider(), body, footer].into()
}

fn divider() -> Element<'static, Message> {
    container(iced::widget::Space::new().width(Length::Fill).height(1))
        .style(|theme| container::Style {
            background: Some(Background::Color(theme::divider(theme))),
            ..Default::default()
        })
        .into()
}

fn close_btn_style(_: &iced::Theme, s: button::Status) -> button::Style {
    button::Style {
        background: matches!(s, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.08))),
        border: Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
