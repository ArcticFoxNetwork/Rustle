//! Exit confirmation dialog component

use iced::Element;
use iced::Size;
use iced::widget::{Space, checkbox, column, text};

use crate::app::Message;
use crate::i18n::{Key, Locale};
use crate::ui::responsive::{IconRole, ResponsiveContext, TextRole};
use crate::ui::theme;

/// Content-only body for unified modal layout (message + checkbox, no backdrop/title/buttons).
pub fn view_body(remember_choice: bool, locale: Locale) -> Element<'static, Message> {
    view_body_responsive(
        remember_choice,
        locale,
        ResponsiveContext::new(Size::new(1_920.0, 1_080.0)),
    )
}

/// Render the exit confirmation body with density-aware typography and controls.
pub fn view_body_responsive(
    remember_choice: bool,
    locale: Locale,
    context: ResponsiveContext,
) -> Element<'static, Message> {
    let tokens = context.tokens;
    let message = text(locale.get(Key::ExitDialogMessage).to_string())
        .size(tokens.text(TextRole::Body))
        .style(|theme| text::Style {
            color: Some(theme::text_secondary(theme)),
        });

    let remember_checkbox = checkbox(remember_choice)
        .label("记住我的选择")
        .on_toggle(Message::ExitDialogRememberChanged)
        .size(tokens.icon(IconRole::Small))
        .text_size(tokens.text(TextRole::Label))
        .spacing(tokens.space(8.0))
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

    column![
        message,
        Space::new().height(tokens.space(16.0)),
        remember_checkbox
    ]
    .into()
}
