//! Source badge component for displaying song origin
//!
//! Renders a small colored badge with icon and label before the artist name.

use iced::widget::{container, row, svg, text};
use iced::{Color, Element, Padding};

use crate::app::Message;
use crate::utils::Source;

/// Colors for source badges
const EMERALD_400: Color = Color::from_rgb(52.0 / 255.0, 211.0 / 255.0, 153.0 / 255.0);
const BLUE_400: Color = Color::from_rgb(96.0 / 255.0, 165.0 / 255.0, 250.0 / 255.0);
const PURPLE_400: Color = Color::from_rgb(192.0 / 255.0, 132.0 / 255.0, 252.0 / 255.0);

/// Build a source badge element (icon + label)
pub fn source_badge(source: Source) -> Element<'static, Message> {
    let (icon_data, label, color) = match source {
        Source::Local => (crate::ui::icons::HARD_DRIVE, "本地", EMERALD_400),
        Source::Cached => (crate::ui::icons::DOWNLOAD_CLOUD, "缓存", BLUE_400),
        Source::Online => (crate::ui::icons::CLOUD, "在线", PURPLE_400),
    };

    let r = color.r;
    let g = color.g;
    let b = color.b;

    container(
        row![
            svg(svg::Handle::from_memory(icon_data.as_bytes()))
                .width(10)
                .height(10)
                .style(move |_theme, _status| svg::Style {
                    color: Some(Color::from_rgb(r, g, b)),
                }),
            text(label).size(9).color(color),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(4),
    )
    .padding(Padding::new(1.0).left(6.0).right(6.0))
    .style(move |_theme| container::Style {
        background: Some(iced::Background::Color(Color::from_rgba(r, g, b, 0.2))),
        border: iced::Border {
            color: Color::from_rgba(r, g, b, 0.2),
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    })
    .into()
}
