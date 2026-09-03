//! Search bar component
//! Rounded search input with icon and placeholder text

use iced::widget::{Space, container, row, svg, text_input};
use iced::{Alignment, Element, Fill, Length, Padding};

use crate::app::Message;
use crate::i18n::{Key, Locale};
use crate::ui::responsive::{IconRole, ResponsiveContext, TextRole};
use crate::ui::theme;

pub const TOP_BAR_SEARCH_INPUT_ID: &str = "top_bar_search_input";

#[derive(Debug, Clone, Copy)]
pub struct SearchBarStyle {
    pub width: f32,
    pub height: f32,
    pub icon_size: f32,
    pub horizontal_padding: f32,
    pub icon_spacing: f32,
    pub input_padding: f32,
    pub text_size: f32,
    pub radius: f32,
    /// Maximum visual width for fluid top-bar usage. The outer lane remains
    /// flexible so the field can shrink instead of compressing the chrome.
    pub max_width: f32,
}

impl SearchBarStyle {
    pub const fn top_bar() -> Self {
        Self {
            width: 0.0, // signal: fluid search content with a visual width cap
            height: 36.0,
            icon_size: 16.0,
            horizontal_padding: 12.0,
            icon_spacing: 8.0,
            input_padding: 8.0,
            text_size: theme::TEXT_SIZE_LABEL,
            radius: 18.0,
            max_width: 320.0,
        }
    }

    /// Build a top-bar style from the shared density tokens.
    pub fn top_bar_scaled(context: &ResponsiveContext, width: f32) -> Self {
        let tokens = &context.tokens;
        Self {
            width,
            height: tokens.size(36.0),
            icon_size: tokens.icon(IconRole::Small),
            horizontal_padding: tokens.space(12.0),
            icon_spacing: tokens.space(8.0),
            input_padding: tokens.space(8.0),
            text_size: tokens.text(TextRole::Label),
            radius: tokens.size(18.0),
            max_width: tokens.size(320.0),
        }
    }
}

impl Default for SearchBarStyle {
    fn default() -> Self {
        Self {
            width: 400.0,
            height: 48.0,
            icon_size: 18.0,
            horizontal_padding: 16.0,
            icon_spacing: 12.0,
            input_padding: 12.0,
            text_size: theme::TEXT_SIZE_BODY,
            radius: 24.0,
            max_width: 400.0,
        }
    }
}

/// Build the search bar component with a custom style
pub fn view(search_query: &str, locale: Locale, style: SearchBarStyle) -> Element<'_, Message> {
    let search_icon = svg(svg::Handle::from_memory(
        crate::ui::icons::SEARCH.as_bytes(),
    ))
    .width(style.icon_size)
    .height(style.icon_size)
    .style(|_theme, _status| svg::Style {
        color: Some(theme::text_muted(_theme)),
    });

    let input = text_input(locale.get(Key::SearchPlaceholder), search_query)
        .id(iced::widget::Id::new(TOP_BAR_SEARCH_INPUT_ID))
        .on_input(Message::SearchChanged)
        .on_submit(Message::SearchSubmit)
        .padding(Padding::new(style.input_padding).left(0.0))
        .size(style.text_size)
        .style(|theme, _status| iced::widget::text_input::Style {
            background: iced::Background::Color(iced::Color::TRANSPARENT),
            border: iced::Border::default(),
            placeholder: theme::text_muted(theme),
            value: theme::text_primary(theme),
            selection: theme::ACCENT_PINK,
        });

    let content = row![
        Space::new().width(style.horizontal_padding),
        search_icon,
        Space::new().width(style.icon_spacing),
        input,
        Space::new().width(style.horizontal_padding),
    ]
    .align_y(Alignment::Center);

    if style.width > 0.0 {
        container(content)
            .width(style.width)
            .height(style.height)
            .align_y(Alignment::Center)
            .style(move |theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(theme::hover_bg_alpha(theme, 0.08))),
                border: iced::Border {
                    radius: style.radius.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    } else {
        // The outer lane fills the flexible row slot while the visual field is
        // bounded only by a maximum. `Length::max` lets it shrink with the
        // right panel instead of forcing the top bar to overflow.
        container(
            container(content)
                .height(style.height)
                .align_y(Alignment::Center)
                .width(Length::Fill.max(style.max_width))
                .style(move |theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(theme::hover_bg_alpha(theme, 0.08))),
                    border: iced::Border {
                        radius: style.radius.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        )
        .width(Fill)
        .align_x(Alignment::Start)
        .into()
    }
}
