//! Toast notification widget
//!
//! Modern dark minimalist toast notifications.
//! Follows Shadcn UI / Spotify style: dark surface with accent color accents.

use iced::widget::{Space, container, row, text};
use iced::{Alignment, Element, Padding};

use crate::ui::theme;

/// Toast notification style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastStyle {
    Success,
    Error,
    Warning,
    Info,
}

impl ToastStyle {
    /// Get the accent color for this style (used for icon/indicator only)
    pub fn accent_color(&self) -> iced::Color {
        match self {
            ToastStyle::Success => theme::success(&iced::Theme::Dark),
            ToastStyle::Error => theme::danger(&iced::Theme::Dark),
            ToastStyle::Warning => theme::warning(&iced::Theme::Dark),
            ToastStyle::Info => theme::info(&iced::Theme::Dark),
        }
    }

    /// Get the icon for this style
    pub fn icon(&self) -> &'static str {
        match self {
            ToastStyle::Success => "✓",
            ToastStyle::Error => "✗",
            ToastStyle::Warning => "⚠",
            ToastStyle::Info => "ℹ",
        }
    }
}

/// Toast notification data
#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub style: ToastStyle,
    pub visible: bool,
}

impl Toast {
    pub fn new(message: impl Into<String>, style: ToastStyle) -> Self {
        Self {
            message: message.into(),
            style,
            visible: true,
        }
    }

    pub fn success(message: impl Into<String>) -> Self {
        Self::new(message, ToastStyle::Success)
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(message, ToastStyle::Error)
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(message, ToastStyle::Warning)
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self::new(message, ToastStyle::Info)
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn show(&mut self) {
        self.visible = true;
    }
}

/// Build a toast notification widget
///
/// Modern dark minimalist style:
/// - Dark gray background (blends with dark mode)
/// - Subtle border for depth
/// - Accent color only on icon (not background)
/// - Soft shadow for floating effect
pub fn view_toast<'a, Message: 'a>(toast: &Toast) -> Element<'a, Message> {
    if !toast.visible {
        return Space::new().width(0).height(0).into();
    }

    let accent_color = toast.style.accent_color();
    let icon = toast.style.icon();
    let message = toast.message.clone();

    // Left accent bar (thin vertical line)
    let accent_bar = container(Space::new().width(3).height(20)).style(move |_theme| {
        iced::widget::container::Style {
            background: Some(iced::Background::Color(accent_color)),
            border: iced::Border {
                radius: 2.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    });

    // Icon with accent color
    let icon_widget = text(icon).size(14).color(accent_color);

    // Message text
    let message_widget = text(message).size(13).style(|theme| text::Style {
        color: Some(theme::text_primary(theme)),
    });

    // Toast content
    let content = row![
        accent_bar,
        Space::new().width(12),
        icon_widget,
        Space::new().width(10),
        message_widget,
    ]
    .align_y(Alignment::Center)
    .padding(Padding::new(14.0).left(12.0).right(20.0));

    // Toast container with dark surface style
    container(content)
        .style(|theme| iced::widget::container::Style {
            // Surface elevated background
            background: Some(iced::Background::Color(theme::surface_elevated(theme))),
            // Subtle border for depth
            border: iced::Border {
                radius: 8.0.into(),
                width: 1.0,
                color: theme::border_color(theme),
            },
            // Soft shadow for floating effect
            shadow: iced::Shadow {
                color: theme::shadow_color(theme),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            ..Default::default()
        })
        .into()
}
