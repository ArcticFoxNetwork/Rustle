//! Exit confirmation dialog component

use iced::mouse::Interaction;
use iced::widget::{Space, button, checkbox, column, container, mouse_area, opaque, row, text};
use iced::{Alignment, Color, Element, Fill};

use crate::app::Message;
use crate::i18n::{Key, Locale};
use crate::ui::theme;

/// Build the exit confirmation dialog
pub fn view(
    remember_choice: bool,
    animation_progress: f32,
    locale: Locale,
) -> Element<'static, Message> {
    if animation_progress < 0.01 {
        return Space::new().height(0).into();
    }

    // Animate opacity (scale animation removed - not currently used)
    let opacity = animation_progress;

    // Dialog content
    let title = text(locale.get(Key::ExitDialogTitle).to_string())
        .size(18)
        .color(theme::TEXT_PRIMARY)
        .font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        });

    let message = text(locale.get(Key::ExitDialogMessage).to_string())
        .size(14)
        .color(theme::TEXT_SECONDARY);

    let remember_checkbox = checkbox(remember_choice)
        .label("记住我的选择")
        .on_toggle(Message::ExitDialogRememberChanged)
        .text_size(13)
        .spacing(8)
        .style(|theme, status| {
            let is_checked = matches!(
                status,
                checkbox::Status::Active { is_checked: true }
                    | checkbox::Status::Hovered { is_checked: true }
            );
            checkbox::Style {
                background: iced::Background::Color(if is_checked {
                    theme::ACCENT_PINK
                } else {
                    theme::hover_bg_alpha(theme, 0.1)
                }),
                icon_color: theme::BLACK,
                border: iced::Border {
                    radius: 4.0.into(),
                    width: if is_checked { 0.0 } else { 1.0 },
                    color: theme::hover_bg_alpha(theme, 0.3),
                },
                text_color: Some(theme::text_secondary(theme)),
            }
        });

    // Buttons
    let exit_btn = button(
        text(locale.get(Key::ExitDialogExit).to_string())
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
    .on_press(Message::ConfirmExit);

    let minimize_btn = button(
        text(locale.get(Key::ExitDialogMinimize).to_string())
            .size(14)
            .color(theme::BLACK),
    )
    .padding([10, 24])
    .style(|theme, status| {
        let bg = match status {
            button::Status::Hovered => theme::hover_bg(theme),
            _ => theme::TEXT_PRIMARY,
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            text_color: theme::BLACK,
            border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .on_press(Message::MinimizeToTray);

    let cancel_btn = button(
        text(locale.get(Key::Cancel).to_string())
            .size(14)
            .color(theme::TEXT_SECONDARY),
    )
    .padding([10, 16])
    .style(|theme, status| {
        let bg = match status {
            button::Status::Hovered => theme::hover_bg(theme),
            _ => Color::TRANSPARENT,
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            text_color: theme::TEXT_SECONDARY,
            border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .on_press(Message::CancelExit);

    let buttons = row![
        cancel_btn,
        Space::new().width(Fill),
        exit_btn,
        Space::new().width(12),
        minimize_btn,
    ]
    .align_y(Alignment::Center);

    let dialog_content = column![
        title,
        Space::new().height(8),
        message,
        Space::new().height(16),
        remember_checkbox,
        Space::new().height(20),
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
    // Use opaque + mouse_area to block all events from reaching underlying widgets
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

    // mouse_area with interaction set to Idle to reset cursor
    // on_press to capture click events (clicking backdrop cancels dialog)
    let event_blocker = mouse_area(backdrop_content)
        .interaction(Interaction::Idle)
        .on_press(Message::CancelExit);

    // opaque to block all mouse button events from propagating
    opaque(event_blocker).into()
}
