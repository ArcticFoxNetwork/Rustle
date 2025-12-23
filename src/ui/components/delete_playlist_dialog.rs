//! Delete playlist confirmation dialog component

use iced::mouse::Interaction;
use iced::widget::{Space, button, column, container, mouse_area, opaque, row, text};
use iced::{Alignment, Color, Element, Fill};

use crate::app::Message;
use crate::i18n::{Key, Locale};
use crate::ui::theme::{self, BOLD_WEIGHT};

/// Build the delete playlist confirmation dialog
pub fn view(playlist_name: &str, animation_progress: f32, locale: Locale) -> Element<'_, Message> {
    if animation_progress < 0.01 {
        return Space::new().height(0).into();
    }

    // Animate opacity
    let opacity = animation_progress;

    // Dialog content
    let title = text(locale.get(Key::DeletePlaylistTitle).to_string())
        .size(18)
        .style(|theme| text::Style {
            color: Some(theme::text_primary(theme)),
        })
        .font(iced::Font {
            weight: BOLD_WEIGHT,
            ..Default::default()
        });

    let message_text = format!("确定要删除歌单「{}」吗？此操作无法撤销。", playlist_name);
    let message = text(message_text).size(14).color(theme::TEXT_SECONDARY);

    // Buttons
    let delete_btn = button(
        text(locale.get(Key::Delete).to_string())
            .size(14)
            .color(Color::WHITE),
    )
    .padding([10, 24])
    .style(|theme, status| {
        let bg = match status {
            button::Status::Hovered => theme::danger_hover(theme),
            _ => theme::danger(theme),
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            text_color: Color::WHITE,
            border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .on_press(Message::ConfirmDeletePlaylist);

    let cancel_btn = button(
        text(locale.get(Key::Cancel).to_string())
            .size(14)
            .color(theme::TEXT_PRIMARY),
    )
    .padding([10, 24])
    .style(|theme, status| {
        let bg = match status {
            button::Status::Hovered => theme::hover_bg(theme),
            _ => theme::surface_container(theme),
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            text_color: theme::TEXT_PRIMARY,
            border: iced::Border {
                radius: 8.0.into(),
                width: 1.0,
                color: theme::divider(theme),
            },
            ..Default::default()
        }
    })
    .on_press(Message::CancelDeletePlaylist);

    let buttons = row![
        Space::new().width(Fill),
        cancel_btn,
        Space::new().width(12),
        delete_btn,
    ]
    .align_y(Alignment::Center);

    let dialog_content = column![
        title,
        Space::new().height(12),
        message,
        Space::new().height(24),
        buttons,
    ]
    .width(380)
    .padding(24);

    // Dialog box with animation
    let dialog_box = container(dialog_content).style(move |theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(Color::from_rgba(
            if theme::is_dark_theme(theme) {
                0.12
            } else {
                0.96
            },
            if theme::is_dark_theme(theme) {
                0.12
            } else {
                0.96
            },
            if theme::is_dark_theme(theme) {
                0.12
            } else {
                0.96
            },
            opacity,
        ))),
        border: iced::Border {
            radius: 12.0.into(),
            width: 1.0,
            color: Color::from_rgba(
                if theme::is_dark_theme(theme) {
                    1.0
                } else {
                    0.0
                },
                if theme::is_dark_theme(theme) {
                    1.0
                } else {
                    0.0
                },
                if theme::is_dark_theme(theme) {
                    1.0
                } else {
                    0.0
                },
                0.1 * opacity,
            ),
        },
        ..Default::default()
    });

    // Backdrop with event interception
    let backdrop_content = container(dialog_box)
        .width(Fill)
        .height(Fill)
        .center_x(Fill)
        .center_y(Fill)
        .style(move |_theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(Color::from_rgba(
                0.0,
                0.0,
                0.0,
                0.5 * opacity,
            ))),
            ..Default::default()
        });

    // mouse_area to capture click events (clicking backdrop cancels dialog)
    let event_blocker = mouse_area(backdrop_content)
        .interaction(Interaction::Idle)
        .on_press(Message::CancelDeletePlaylist);

    // opaque to block all mouse button events from propagating
    opaque(event_blocker).into()
}
