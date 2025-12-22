//! Search bar component
//! Rounded search input with icon and placeholder text

use iced::widget::{Space, container, row, svg, text_input};
use iced::{Alignment, Element, Padding};

use crate::app::Message;
use crate::i18n::{Key, Locale};
use crate::ui::theme;

/// Build the search bar component
pub fn view(search_query: &str, locale: Locale) -> Element<'_, Message> {
    let search_icon = svg(svg::Handle::from_memory(
        crate::ui::icons::SEARCH.as_bytes(),
    ))
    .width(18)
    .height(18)
    .style(|_theme, _status| svg::Style {
        color: Some(theme::TEXT_MUTED),
    });

    let input = text_input(locale.get(Key::SearchPlaceholder), search_query)
        .on_input(Message::SearchChanged)
        .padding(Padding::new(12.0).left(0.0))
        .size(14)
        .font(iced::Font::with_name("Inter"))
        .style(|theme, _status| iced::widget::text_input::Style {
            background: iced::Background::Color(iced::Color::TRANSPARENT),
            border: iced::Border::default(),
            icon: theme::TEXT_MUTED,
            placeholder: theme::TEXT_MUTED,
            value: theme::text_primary(theme),
            selection: theme::ACCENT_PINK,
        });

    let content = row![
        Space::new().width(16),
        search_icon,
        Space::new().width(12),
        input,
        Space::new().width(16),
    ]
    .align_y(Alignment::Center);

    container(content)
        .width(400)
        .style(|theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::surface(theme))),
            border: iced::Border {
                radius: 24.0.into(),
                width: 1.0,
                color: theme::border_color(theme),
            },
            ..Default::default()
        })
        .into()
}
