//! Window control buttons and navigation bar
//! Positioned at top of the application with navigation on left, search in center, and controls on right

use iced::widget::{Space, button, container, mouse_area, row, stack, svg, text, tooltip};
use iced::{Alignment, Color, ContentFit, Element, Fill, Length, Padding};

use crate::app::{ImageState, Message, UserInfo};
use crate::i18n::{Key, Locale};
use crate::image::ImageKind;
use crate::ui::components::search_bar::{self, SearchBarStyle};
use crate::ui::{theme, widgets};

/// Build the complete top bar with navigation buttons on left, search bar in center, user info and window controls on right
pub fn view<'a>(
    locale: Locale,
    can_go_back: bool,
    can_go_forward: bool,
    search_query: &'a str,
    is_logged_in: bool,
    user_info: Option<&UserInfo>,
    image_state: &ImageState,
    show_background: bool,
    is_maximized: bool,
) -> Element<'a, Message> {
    let button_size = 36;
    let icon_size = 16;
    let nav_icon_size = 18;
    let back_icon_opacity: f32 = if can_go_back { 1.0 } else { 0.5 };
    let forward_icon_opacity: f32 = if can_go_forward { 1.0 } else { 0.5 };

    // Navigation buttons (left side)
    let back_button = button(
        svg(svg::Handle::from_memory(BACK_ICON.as_bytes()))
            .width(nav_icon_size)
            .height(nav_icon_size)
            .style(move |theme, _status| svg::Style {
                color: Some(if can_go_back {
                    theme::text_secondary(theme)
                } else {
                    theme::opaque_color(theme::TEXT_DISABLED)
                }),
            })
            .opacity(back_icon_opacity),
    )
    .width(button_size)
    .height(button_size)
    .style(move |theme, status| nav_button_style(theme, status, can_go_back))
    .on_press_maybe(if can_go_back {
        Some(Message::NavigateBack)
    } else {
        None
    });
    let back_btn = tooltip(
        animated_nav_button(back_button.into(), can_go_back),
        locale.get(Key::Back),
        tooltip::Position::Bottom,
    );

    let forward_button = button(
        svg(svg::Handle::from_memory(FORWARD_ICON.as_bytes()))
            .width(nav_icon_size)
            .height(nav_icon_size)
            .style(move |theme, _status| svg::Style {
                color: Some(if can_go_forward {
                    theme::text_secondary(theme)
                } else {
                    theme::opaque_color(theme::TEXT_DISABLED)
                }),
            })
            .opacity(forward_icon_opacity),
    )
    .width(button_size)
    .height(button_size)
    .style(move |theme, status| nav_button_style(theme, status, can_go_forward))
    .on_press_maybe(if can_go_forward {
        Some(Message::NavigateForward)
    } else {
        None
    });
    let forward_btn = tooltip(
        animated_nav_button(forward_button.into(), can_go_forward),
        locale.get(Key::Forward),
        tooltip::Position::Bottom,
    );

    // Keep back and forward as separate circular controls.
    let nav_buttons = container(
        row![back_btn, forward_btn]
            .spacing(8)
            .align_y(Alignment::Center),
    )
    .padding(Padding::new(12.0).left(16.0));

    // User info (avatar + username + API-backed membership badge)
    let avatar_size = 28.0;
    let avatar_radius = avatar_size / 2.0;
    let avatar_icon_size = 14.0;

    let avatar_elem: Element<'_, Message> = if is_logged_in {
        if let Some(info) = user_info {
            if let Some(handle) = image_state.get(ImageKind::UserAvatar, info.user_id) {
                container(
                    widgets::crossfade_image(Some(handle.clone()))
                        .width(Fill)
                        .height(Fill)
                        .content_fit(ContentFit::Cover)
                        .border_radius(avatar_radius),
                )
                .width(avatar_size)
                .height(avatar_size)
                .into()
            } else {
                avatar_placeholder(avatar_size, avatar_radius, avatar_icon_size)
            }
        } else {
            avatar_placeholder(avatar_size, avatar_radius, avatar_icon_size)
        }
    } else {
        avatar_placeholder(avatar_size, avatar_radius, avatar_icon_size)
    };

    let user_text: Element<'_, Message> = if is_logged_in {
        if let Some(info) = user_info {
            if let Some(icon_url) = info.vip.badge_url()
                && let Some(handle) = image_state.get(
                    ImageKind::VipBadge,
                    crate::image::vip_badge_key(info.user_id, info.vip.tier(), icon_url),
                )
            {
                let vip_badge = widgets::crossfade_image(Some(handle.clone()))
                    // Match the adjacent 16px nickname. Width remains Shrink,
                    // so Iced derives it from the official image aspect ratio.
                    .height(theme::TEXT_SIZE_BODY_LARGE)
                    .content_fit(ContentFit::Contain);
                row![
                    text(info.nickname.clone())
                        .size(theme::TEXT_SIZE_BODY_LARGE)
                        .font(iced::Font::DEFAULT.weight(theme::BOLD_WEIGHT))
                        .style(|theme| text::Style {
                            color: Some(theme::text_primary(theme))
                        }),
                    Space::new().width(6),
                    vip_badge,
                ]
                .align_y(Alignment::Center)
                .into()
            } else {
                text(info.nickname.clone())
                    .size(theme::TEXT_SIZE_BODY_LARGE)
                    .font(iced::Font::DEFAULT.weight(theme::BOLD_WEIGHT))
                    .style(|theme| text::Style {
                        color: Some(theme::text_primary(theme)),
                    })
                    .into()
            }
        } else {
            text("未登录")
                .size(theme::TEXT_SIZE_BODY)
                .style(|theme| text::Style {
                    color: Some(theme::text_muted(theme)),
                })
                .into()
        }
    } else {
        text("未登录")
            .size(theme::TEXT_SIZE_BODY)
            .style(|theme| text::Style {
                color: Some(theme::text_muted(theme)),
            })
            .into()
    };

    let user_info_widget =
        button(row![avatar_elem, Space::new().width(6), user_text,].align_y(Alignment::Center))
            .style(|_theme, _status| button::Style {
                background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
                text_color: iced::Color::TRANSPARENT,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: false,
            })
            .on_press(if is_logged_in {
                user_info.map_or(Message::OpenSettings, |info| {
                    Message::OpenUser(info.user_id)
                })
            } else {
                Message::ToggleLoginPopup
            });

    // Window control buttons (right side)
    let settings_button = button(
        svg(svg::Handle::from_memory(
            crate::ui::icons::SETTINGS.as_bytes(),
        ))
        .width(icon_size)
        .height(icon_size)
        .style(|theme, _status| svg::Style {
            color: Some(theme::text_secondary(theme)),
        }),
    )
    .width(button_size)
    .height(button_size)
    .style(window_button_style)
    .on_press(Message::OpenSettings);
    let settings_btn = tooltip(
        animated_window_button(settings_button.into()),
        locale.get(Key::Settings),
        tooltip::Position::Bottom,
    );

    let minimize_button = button(
        svg(svg::Handle::from_memory(MINIMIZE_ICON.as_bytes()))
            .width(icon_size)
            .height(icon_size)
            .style(|theme, _status| svg::Style {
                color: Some(theme::text_secondary(theme)),
            }),
    )
    .width(button_size)
    .height(button_size)
    .style(window_button_style)
    .on_press(Message::WindowMinimize);
    let minimize_btn = tooltip(
        animated_window_button(minimize_button.into()),
        locale.get(Key::Minimize),
        tooltip::Position::Bottom,
    );

    let maximize_button = button(
        svg(svg::Handle::from_memory(
            crate::ui::icons::maximize_restore(is_maximized).as_bytes(),
        ))
        .width(icon_size)
        .height(icon_size)
        .style(|theme, _status| svg::Style {
            color: Some(theme::text_secondary(theme)),
        }),
    )
    .width(button_size)
    .height(button_size)
    .style(window_button_style)
    .on_press(Message::WindowMaximize);
    let maximize_btn = tooltip(
        animated_window_button(maximize_button.into()),
        locale.get(if is_maximized {
            Key::Restore
        } else {
            Key::Maximize
        }),
        tooltip::Position::Bottom,
    );

    let close_button = button(
        svg(svg::Handle::from_memory(CLOSE_ICON.as_bytes()))
            .width(icon_size)
            .height(icon_size)
            .style(|theme, _status| svg::Style {
                color: Some(theme::text_secondary(theme)),
            }),
    )
    .width(button_size)
    .height(button_size)
    .style(close_button_style)
    .on_press(Message::RequestClose);
    let close_btn = tooltip(
        animated_close_button(close_button.into()),
        locale.get(Key::Close),
        tooltip::Position::Bottom,
    );

    // Search bar (left, after nav buttons)
    let search_bar = search_bar::view(search_query, locale, SearchBarStyle::top_bar());

    let drag_region = mouse_area(
        container(Space::new())
            .width(Fill)
            .height(Length::Fixed(theme::TOP_BAR_HEIGHT)),
    )
    .on_press(Message::WindowDrag);

    let controls = container(
        row![
            nav_buttons,
            Space::new().width(16),
            search_bar,
            Space::new().width(12),
            user_info_widget,
            Space::new().width(6),
            settings_btn,
            Space::new().width(6),
            minimize_btn,
            Space::new().width(6),
            maximize_btn,
            Space::new().width(6),
            close_btn,
        ]
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .height(Length::Fixed(theme::TOP_BAR_HEIGHT))
    .padding(Padding::new(0.0).right(12.0));

    // Native Iced scrollbars do not support a top-only track inset. Cover the
    // scrollbar gutter inside the window chrome so the scroll thumb starts
    // visually below the top bar while page content can still pass underneath.
    let scrollbar_gutter: Element<'a, Message> = if show_background {
        container(
            container(Space::new())
                .width(theme::TOP_BAR_SCROLLBAR_GUTTER_WIDTH)
                .height(Fill)
                .style(|active_theme| container::Style {
                    background: Some(iced::Background::Color(theme::background(active_theme))),
                    ..Default::default()
                }),
        )
        .width(Fill)
        .height(Length::Fixed(theme::TOP_BAR_HEIGHT))
        .align_x(Alignment::End)
        .into()
    } else {
        Space::new().into()
    };

    // Complete top bar layout: nav + search on left, user info + window controls on right.
    // The bottom layer provides dragging only in empty space; controls stay interactive above it.
    container(
        stack![drag_region, controls, scrollbar_gutter]
            .width(Fill)
            .height(Length::Fixed(theme::TOP_BAR_HEIGHT)),
    )
    .width(Fill)
    .height(Length::Fixed(theme::TOP_BAR_HEIGHT))
    .style(move |active_theme| container::Style {
        background: show_background
            .then(|| iced::Background::Color(theme::top_bar_background(active_theme))),
        ..Default::default()
    })
    .into()
}

/// Navigation button style (back/forward)
fn nav_button_style(theme: &iced::Theme, status: button::Status, enabled: bool) -> button::Style {
    let base = button::Style {
        background: Some(iced::Background::Color(Color::TRANSPARENT)),
        text_color: if enabled {
            theme::text_secondary(theme)
        } else {
            theme::TEXT_DISABLED
        },
        border: iced::Border {
            radius: 18.0.into(),
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
            text_color: theme::text_primary(theme),
            ..base
        },
        _ => base,
    }
}

/// Window button style (settings, minimize, maximize)
fn window_button_style(theme: &iced::Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
        text_color: theme::text_secondary(theme),
        border: iced::Border {
            radius: 18.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow::default(),
        snap: true,
    };

    match status {
        button::Status::Hovered => button::Style {
            text_color: theme::text_primary(theme),
            ..base
        },
        _ => base,
    }
}

/// Close button style (red on hover)
fn close_button_style(theme: &iced::Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
        text_color: theme::text_secondary(theme),
        border: iced::Border {
            radius: 18.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow::default(),
        snap: true,
    };

    match status {
        button::Status::Hovered => button::Style {
            text_color: theme::text_primary(theme),
            ..base
        },
        _ => base,
    }
}

fn animated_nav_button<'a>(content: Element<'a, Message>, enabled: bool) -> Element<'a, Message> {
    widgets::hover_surface(content)
        .enabled(enabled)
        .style(move |theme, progress| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::hover_bg_alpha(
                theme,
                0.08 + 0.06 * progress,
            ))),
            border: iced::Border {
                radius: 18.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn animated_window_button<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    widgets::hover_surface(content)
        .style(|theme, progress| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::hover_bg_alpha(
                theme,
                0.14 * progress,
            ))),
            border: iced::Border {
                radius: 18.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn animated_close_button<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    widgets::hover_surface(content)
        .style(|theme, progress| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::lerp_color(
                Color::TRANSPARENT,
                theme::close_button_hover(theme),
                progress,
            ))),
            border: iced::Border {
                radius: 18.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
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

const CLOSE_ICON: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <line x1="6" y1="6" x2="18" y2="18"/>
    <line x1="6" y1="18" x2="18" y2="6"/>
</svg>"#;

/// Circular avatar placeholder with a centered user icon
fn avatar_placeholder(size: f32, radius: f32, icon_size: f32) -> Element<'static, Message> {
    container(
        svg(svg::Handle::from_memory(crate::ui::icons::USER.as_bytes()))
            .width(icon_size)
            .height(icon_size)
            .style(|theme, _status| svg::Style {
                color: Some(theme::text_secondary(theme)),
            }),
    )
    .width(size)
    .height(size)
    .center_x(size)
    .center_y(size)
    .style(move |theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(theme::border_color(theme))),
        border: iced::Border {
            radius: radius.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}
