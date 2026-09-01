//! Search bar component
//! Rounded search input with icon and placeholder text

use iced::widget::{Space, container, row, svg, text_input};
use iced::{Alignment, Element, Fill, Padding};

use crate::app::Message;
use crate::i18n::{Key, Locale};
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
}

impl SearchBarStyle {
    pub const fn top_bar() -> Self {
        Self {
            width: 0.0, // signal: Fill + max_width(240)
            height: 36.0,
            icon_size: 16.0,
            horizontal_padding: 12.0,
            icon_spacing: 8.0,
            input_padding: 8.0,
            text_size: theme::TEXT_SIZE_LABEL,
            radius: 18.0,
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
            icon: theme::text_muted(theme),
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
        // Web-like: outer Fill fills flex space, inner max_width caps visual size
        container(
            container(content)
                .height(style.height)
                .align_y(Alignment::Center)
                .max_width(320.0)
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
