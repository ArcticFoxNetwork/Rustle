//! Search bar component
//! Rounded search input with icon and placeholder text

use iced::widget::{Space, container, row, svg, text_input};
use iced::{Alignment, Element, Padding};

use crate::app::Message;
use crate::i18n::{Key, Locale};
use crate::ui::theme;

pub const TOP_BAR_SEARCH_INPUT_ID: &str = "top_bar_search_input";

#[derive(Debug, Clone, Copy)]
pub struct SearchBarStyle {
    pub width: f32,
    pub icon_size: f32,
    pub horizontal_padding: f32,
    pub icon_spacing: f32,
    pub input_padding: f32,
    pub text_size: f32,
    pub background: iced::Color,
    pub radius: f32,
}

impl SearchBarStyle {
    pub const fn top_bar() -> Self {
        Self {
            width: 320.0,
            icon_size: 16.0,
            horizontal_padding: 12.0,
            icon_spacing: 8.0,
            input_padding: 8.0,
            text_size: theme::TEXT_SIZE_LABEL,
            background: iced::Color::from_rgba(1.0, 1.0, 1.0, 0.08),
            radius: 20.0,
        }
    }
}

impl Default for SearchBarStyle {
    fn default() -> Self {
        Self {
            width: 400.0,
            icon_size: 18.0,
            horizontal_padding: 16.0,
            icon_spacing: 12.0,
            input_padding: 12.0,
            text_size: theme::TEXT_SIZE_BODY,
            background: iced::Color::from_rgb(0.1, 0.1, 0.1),
            radius: 24.0,
        }
    }
}

/// Build the search bar component with a custom style
pub fn view(
    search_query: &str,
    locale: Locale,
    style: SearchBarStyle,
) -> Element<'_, Message> {
    let search_icon = svg(svg::Handle::from_memory(
        crate::ui::icons::SEARCH.as_bytes(),
    ))
    .width(style.icon_size)
    .height(style.icon_size)
    .style(|_theme, _status| svg::Style {
        color: Some(theme::TEXT_MUTED),
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
            icon: theme::TEXT_MUTED,
            placeholder: theme::TEXT_MUTED,
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

    container(content)
        .width(style.width)
        .style(move |theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(style.background)),
            border: iced::Border {
                radius: style.radius.into(),
                width: 1.0,
                color: theme::border_color(theme),
            },
            ..Default::default()
        })
        .into()
}
