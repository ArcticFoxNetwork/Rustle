//! Login popup component with QR code display
//!
//! Displays QR code for NCM login with status messages.

use iced::widget::{Space, button, column, container, image, row, svg, text};
use iced::{Alignment, Color, Element, Fill, Padding};
use std::path::PathBuf;

use crate::app::Message;
use crate::app::UserInfo;
use crate::i18n::{Key, Locale};
use crate::ui::theme;

const POPUP_WIDTH: f32 = 320.0;
const POPUP_HEIGHT: f32 = 400.0;
const QR_SIZE: f32 = 200.0;

/// Build the login popup view
pub fn view<'a>(
    is_open: bool,
    qr_code_path: Option<&'a PathBuf>,
    qr_status: Option<&'a str>,
    user_info: Option<&'a UserInfo>,
    is_logged_in: bool,
    locale: Locale,
) -> Element<'a, Message> {
    if !is_open {
        return Space::new().width(0).height(0).into();
    }

    let content: Element<'_, Message> = if is_logged_in {
        // Show user info and logout button
        if let Some(user) = user_info {
            view_logged_in(user, locale)
        } else {
            view_qr_login(qr_code_path, qr_status, locale)
        }
    } else {
        view_qr_login(qr_code_path, qr_status, locale)
    };

    // Popup container with mouse_area to prevent click-through to backdrop
    let popup = iced::widget::mouse_area(
        container(content)
            .width(POPUP_WIDTH)
            .height(POPUP_HEIGHT)
            .padding(24)
            .style(theme::login_popup),
    );

    // Backdrop - clicking outside popup closes it
    let backdrop = iced::widget::mouse_area(
        container(Space::new().width(Fill).height(Fill))
            .width(Fill)
            .height(Fill)
            .style(|theme| container::Style {
                background: Some(theme::overlay_backdrop(theme, 0.5).into()),
                ..Default::default()
            }),
    )
    .on_press(Message::ToggleLoginPopup);

    // Stack popup on backdrop
    iced::widget::stack![
        backdrop,
        container(popup)
            .width(Fill)
            .height(Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center),
    ]
    .width(Fill)
    .height(Fill)
    .into()
}

/// View for QR code login
fn view_qr_login<'a>(
    qr_code_path: Option<&'a PathBuf>,
    qr_status: Option<&'a str>,
    locale: Locale,
) -> Element<'a, Message> {
    let title = text(locale.get(Key::LoginScanQr).to_string())
        .size(24)
        .font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..iced::Font::with_name("Inter")
        })
        .style(|theme| text::Style {
            color: Some(theme::text_primary(theme)),
        });

    let qr_display: Element<'_, Message> = if let Some(path) = qr_code_path {
        container(
            image(path.to_string_lossy().to_string())
                .width(QR_SIZE)
                .height(QR_SIZE),
        )
        .style(|_theme| container::Style {
            background: Some(Color::WHITE.into()),
            border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .padding(8)
        .into()
    } else {
        container(
            text(locale.get(Key::LoginGeneratingQr).to_string())
                .size(14)
                .color(theme::TEXT_SECONDARY),
        )
        .width(QR_SIZE)
        .height(QR_SIZE)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(|_theme| container::Style {
            background: Some(theme::SURFACE_SECONDARY.into()),
            border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
    };

    let status_text = text(qr_status.unwrap_or("请使用网易云音乐App扫码"))
        .size(14)
        .color(theme::TEXT_SECONDARY);

    let refresh_button = button(
        row![
            svg(svg::Handle::from_memory(
                crate::ui::icons::REFRESH.as_bytes()
            ))
            .width(16)
            .height(16)
            .style(|theme, _status| svg::Style {
                color: Some(theme::text_primary(theme)),
            }),
            Space::new().width(8),
            text(locale.get(Key::LoginRefreshQr).to_string())
                .size(14)
                .style(|theme| text::Style {
                    color: Some(theme::text_primary(theme))
                }),
        ]
        .align_y(Alignment::Center),
    )
    .padding(Padding::new(8.0).left(16.0).right(16.0))
    .style(theme::secondary_button)
    .on_press(Message::RequestQrCode);

    column![
        title,
        Space::new().height(24),
        qr_display,
        Space::new().height(16),
        status_text,
        Space::new().height(16),
        refresh_button,
    ]
    .spacing(0)
    .align_x(Alignment::Center)
    .width(Fill)
    .into()
}

/// View for logged in user
fn view_logged_in(user: &UserInfo, locale: Locale) -> Element<'_, Message> {
    let title = text(locale.get(Key::LoginLoggedIn).to_string())
        .size(24)
        .font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..iced::Font::with_name("Inter")
        })
        .style(|theme| text::Style {
            color: Some(theme::text_primary(theme)),
        });

    let username = text(&user.nickname).size(18).style(|theme| text::Style {
        color: Some(theme::text_primary(theme)),
    });

    let uid_text = text(format!("UID: {}", user.user_id))
        .size(14)
        .color(theme::TEXT_SECONDARY);

    let logout_button = button(
        row![
            svg(svg::Handle::from_memory(
                crate::ui::icons::LOGOUT.as_bytes()
            ))
            .width(16)
            .height(16)
            .style(|_theme, _status| svg::Style {
                color: Some(Color::WHITE),
            }),
            Space::new().width(8),
            text(locale.get(Key::LoginLogout).to_string())
                .size(14)
                .color(Color::WHITE),
        ]
        .align_y(Alignment::Center),
    )
    .padding(Padding::new(12.0).left(20.0).right(20.0))
    .style(theme::danger_button)
    .on_press(Message::Logout);

    column![
        title,
        Space::new().height(32),
        username,
        Space::new().height(8),
        uid_text,
        Space::new().height(32),
        logout_button,
    ]
    .spacing(0)
    .align_x(Alignment::Center)
    .width(Fill)
    .into()
}
