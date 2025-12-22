//! Window control buttons and navigation bar
//! Positioned at top of the application with navigation on left and controls on right

use iced::widget::{Space, button, container, row, svg, tooltip};
use iced::{Alignment, Element, Fill, Padding};

use crate::app::Message;
use crate::i18n::{Key, Locale};
use crate::ui::theme;

/// Build the complete top bar with navigation buttons on left and window controls on right
pub fn view(locale: Locale, can_go_back: bool, can_go_forward: bool) -> Element<'static, Message> {
    let button_size = 32;
    let icon_size = 14;
    let nav_icon_size = 16;

    // Navigation buttons (left side)
    let back_btn = tooltip(
        button(
            svg(svg::Handle::from_memory(BACK_ICON.as_bytes()))
                .width(nav_icon_size)
                .height(nav_icon_size)
                .style(move |theme, _status| svg::Style {
                    color: Some(if can_go_back {
                        theme::text_secondary(theme)
                    } else {
                        theme::TEXT_DISABLED
                    }),
                }),
        )
        .width(button_size)
        .height(button_size)
        .style(move |theme, status| nav_button_style(theme, status, can_go_back))
        .on_press_maybe(if can_go_back {
            Some(Message::NavigateBack)
        } else {
            None
        }),
        locale.get(Key::Back),
        tooltip::Position::Bottom,
    );

    let forward_btn = tooltip(
        button(
            svg(svg::Handle::from_memory(FORWARD_ICON.as_bytes()))
                .width(nav_icon_size)
                .height(nav_icon_size)
                .style(move |theme, _status| svg::Style {
                    color: Some(if can_go_forward {
                        theme::text_secondary(theme)
                    } else {
                        theme::TEXT_DISABLED
                    }),
                }),
        )
        .width(button_size)
        .height(button_size)
        .style(move |theme, status| nav_button_style(theme, status, can_go_forward))
        .on_press_maybe(if can_go_forward {
            Some(Message::NavigateForward)
        } else {
            None
        }),
        locale.get(Key::Forward),
        tooltip::Position::Bottom,
    );

    // Vertical divider between back and forward buttons (full height)
    let divider = container(Space::new().width(1).height(Fill)).style(|theme: &iced::Theme| {
        container::Style {
            background: Some(iced::Background::Color(theme::border_color(theme))),
            ..Default::default()
        }
    });

    // Navigation buttons group with border container
    let nav_group = container(
        row![back_btn, divider, forward_btn,]
            .align_y(Alignment::Center)
            .height(button_size),
    )
    .style(nav_group_container);

    // Add left margin to move buttons away from edge
    let nav_buttons = container(nav_group).padding(Padding::new(8.0).left(16.0));

    // Window control buttons (right side)
    let settings_btn = tooltip(
        button(
            svg(svg::Handle::from_memory(
                crate::ui::icons::SETTINGS.as_bytes(),
            ))
            .width(icon_size)
            .height(icon_size)
            .style(|_theme, _status| svg::Style {
                color: Some(theme::TEXT_SECONDARY),
            }),
        )
        .width(button_size)
        .height(button_size)
        .style(window_button_style)
        .on_press(Message::OpenSettings),
        locale.get(Key::Settings),
        tooltip::Position::Bottom,
    );

    let minimize_btn = tooltip(
        button(
            svg(svg::Handle::from_memory(MINIMIZE_ICON.as_bytes()))
                .width(icon_size)
                .height(icon_size)
                .style(|_theme, _status| svg::Style {
                    color: Some(theme::TEXT_SECONDARY),
                }),
        )
        .width(button_size)
        .height(button_size)
        .style(window_button_style)
        .on_press(Message::WindowMinimize),
        locale.get(Key::Minimize),
        tooltip::Position::Bottom,
    );

    let maximize_btn = tooltip(
        button(
            svg(svg::Handle::from_memory(MAXIMIZE_ICON.as_bytes()))
                .width(icon_size)
                .height(icon_size)
                .style(|_theme, _status| svg::Style {
                    color: Some(theme::TEXT_SECONDARY),
                }),
        )
        .width(button_size)
        .height(button_size)
        .style(window_button_style)
        .on_press(Message::WindowMaximize),
        locale.get(Key::Maximize),
        tooltip::Position::Bottom,
    );

    let close_btn = tooltip(
        button(
            svg(svg::Handle::from_memory(CLOSE_ICON.as_bytes()))
                .width(icon_size)
                .height(icon_size)
                .style(|_theme, _status| svg::Style {
                    color: Some(theme::TEXT_SECONDARY),
                }),
        )
        .width(button_size)
        .height(button_size)
        .style(close_button_style)
        .on_press(Message::RequestClose),
        locale.get(Key::Close),
        tooltip::Position::Bottom,
    );

    let window_controls = container(
        row![
            settings_btn,
            Space::new().width(4),
            minimize_btn,
            Space::new().width(4),
            maximize_btn,
            Space::new().width(4),
            close_btn,
        ]
        .align_y(Alignment::Center),
    )
    .padding(Padding::new(8.0));

    // Complete top bar layout
    row![nav_buttons, Space::new().width(Fill), window_controls,]
        .align_y(Alignment::Center)
        .into()
}

/// Navigation group container style (rounded border)
fn nav_group_container(theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
        border: iced::Border {
            radius: 8.0.into(),
            width: 1.0,
            color: theme::border_color(theme),
        },
        ..Default::default()
    }
}

/// Navigation button style (back/forward)
fn nav_button_style(theme: &iced::Theme, status: button::Status, enabled: bool) -> button::Style {
    let base = button::Style {
        background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
        text_color: if enabled {
            theme::TEXT_SECONDARY
        } else {
            theme::TEXT_DISABLED
        },
        border: iced::Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow::default(),
        snap: true,
    };

    if !enabled {
        return base;
    }

    match status {
        button::Status::Hovered => button::Style {
            background: Some(iced::Background::Color(theme::surface(theme))),
            text_color: theme::text_primary(theme),
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(iced::Background::Color(theme::border_color(theme))),
            ..base
        },
        _ => base,
    }
}

/// Window button style (settings, minimize, maximize)
fn window_button_style(theme: &iced::Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
        text_color: theme::TEXT_SECONDARY,
        border: iced::Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow::default(),
        snap: true,
    };

    match status {
        button::Status::Hovered => button::Style {
            background: Some(iced::Background::Color(theme::surface(theme))),
            text_color: theme::text_primary(theme),
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(iced::Background::Color(theme::border_color(theme))),
            ..base
        },
        _ => base,
    }
}

/// Close button style (red on hover)
fn close_button_style(theme: &iced::Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
        text_color: theme::TEXT_SECONDARY,
        border: iced::Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow::default(),
        snap: true,
    };

    match status {
        button::Status::Hovered => button::Style {
            background: Some(iced::Background::Color(theme::close_button_hover())),
            text_color: theme::text_primary(theme),
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(iced::Background::Color(theme::close_button_pressed())),
            text_color: theme::text_primary(theme),
            ..base
        },
        _ => base,
    }
}

// Navigation icons - clean chevron style
const BACK_ICON: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
    <polyline points="15 18 9 12 15 6"/>
</svg>"#;

const FORWARD_ICON: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
    <polyline points="9 18 15 12 9 6"/>
</svg>"#;

// Window control icons
const MINIMIZE_ICON: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
    <line x1="5" y1="12" x2="19" y2="12"/>
</svg>"#;

const MAXIMIZE_ICON: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <rect x="4" y="4" width="16" height="16" rx="2"/>
</svg>"#;

const CLOSE_ICON: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <line x1="6" y1="6" x2="18" y2="18"/>
    <line x1="6" y1="18" x2="18" y2="6"/>
</svg>"#;
