//! Edit playlist dialog component

use iced::widget::{Space, button, column, container, image, row, svg, text, text_input, toggler};
use iced::{Alignment, Color, Element, Fill};

use crate::app::Message;
use crate::i18n::{Key, Locale};
use crate::ui::{icons, theme};

/// Content-only body for unified modal layout (cover + form fields, no backdrop/title/buttons).
pub fn view_body<'a>(
    name: &str,
    description: &str,
    cover_path: Option<&str>,
    watch_available: bool,
    watch_enabled: bool,
    watch_path: Option<&'a str>,
    locale: Locale,
) -> Element<'a, Message> {
    // Cover
    let cover_content: Element<'a, Message> = if let Some(path) = cover_path {
        image(path)
            .width(120)
            .height(120)
            .content_fit(iced::ContentFit::Cover)
            .into()
    } else {
        container(
            svg(svg::Handle::from_memory(icons::MUSIC.as_bytes()))
                .width(40)
                .height(40)
                .style(|_theme, _status| svg::Style {
                    color: Some(theme::icon_muted(&iced::Theme::Dark)),
                }),
        )
        .width(120)
        .height(120)
        .center_x(120)
        .center_y(120)
        .style(|theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::surface_container(theme))),
            border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
    };

    let change_cover_btn = button(
        text(locale.get(Key::EditPlaylistChangeCover).to_string())
            .size(theme::TEXT_SIZE_CAPTION)
            .style(|theme| text::Style {
                color: Some(theme::text_muted(theme)),
            }),
    )
    .padding([4, 8])
    .style(|theme, status| {
        let bg = matches!(status, button::Status::Hovered).then_some(theme::hover_bg(theme));
        button::Style {
            background: bg.map(iced::Background::Color),
            ..Default::default()
        }
    })
    .on_press(Message::PickCoverImage);

    let cover_section = column![
        container(cover_content)
            .width(120)
            .height(120)
            .style(|_theme| iced::widget::container::Style {
                border: iced::Border {
                    radius: 12.0.into(),
                    width: 1.0,
                    color: Color::from_rgba(1.0, 1.0, 1.0, 0.06)
                },
                ..Default::default()
            }),
        Space::new().height(8),
        change_cover_btn,
    ]
    .align_x(Alignment::Center);

    // Form fields
    let name_label = text(locale.get(Key::EditPlaylistName).to_string())
        .size(theme::TEXT_SIZE_BODY)
        .style(|theme| text::Style {
            color: Some(theme::text_secondary(theme)),
        });
    let name_input = text_input(locale.get(Key::EditPlaylistNamePlaceholder), name)
        .on_input(Message::EditPlaylistNameChanged)
        .padding(12)
        .size(theme::TEXT_SIZE_BODY_LARGE)
        .style(|theme, _status| text_input::Style {
            background: iced::Background::Color(theme::surface_container(theme)),
            border: iced::Border {
                color: theme::divider(theme),
                width: 1.0,
                radius: 6.0.into(),
            },
            icon: theme::text_muted(theme),
            placeholder: theme::text_muted(theme),
            value: theme::text_primary(theme),
            selection: theme::ACCENT_PINK,
        });
    let desc_label = text(locale.get(Key::EditPlaylistDesc).to_string())
        .size(theme::TEXT_SIZE_BODY)
        .style(|theme| text::Style {
            color: Some(theme::text_secondary(theme)),
        });
    let desc_input = text_input(locale.get(Key::EditPlaylistDescPlaceholder), description)
        .on_input(Message::EditPlaylistDescriptionChanged)
        .padding(12)
        .size(theme::TEXT_SIZE_BODY_LARGE)
        .style(|theme, _status| text_input::Style {
            background: iced::Background::Color(theme::surface_container(theme)),
            border: iced::Border {
                color: theme::divider(theme),
                width: 1.0,
                radius: 6.0.into(),
            },
            icon: theme::text_muted(theme),
            placeholder: theme::text_muted(theme),
            value: theme::text_primary(theme),
            selection: theme::ACCENT_PINK,
        });

    let watch_section: Element<'a, Message> = if watch_available {
        container(
            column![
                row![
                    column![
                        text(locale.get(Key::EditPlaylistWatchLibrary).to_string())
                            .size(theme::TEXT_SIZE_BODY)
                            .style(|theme| text::Style {
                                color: Some(theme::text_secondary(theme))
                            }),
                        Space::new().height(4),
                        text(locale.get(Key::EditPlaylistWatchLibraryDesc).to_string())
                            .size(theme::TEXT_SIZE_CAPTION)
                            .style(|theme| text::Style {
                                color: Some(theme::text_muted(theme))
                            }),
                    ]
                    .spacing(0)
                    .width(Fill),
                    toggler(watch_enabled)
                        .on_toggle(Message::EditPlaylistWatchEnabledChanged)
                        .size(theme::TEXT_SIZE_TITLE),
                ]
                .align_y(Alignment::Center),
                Space::new().height(8),
                text(watch_path.unwrap_or_default())
                    .size(theme::TEXT_SIZE_CAPTION)
                    .style(|theme| text::Style {
                        color: Some(theme::text_muted(theme))
                    }),
            ]
            .spacing(0),
        )
        .padding(12)
        .style(|theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::surface_container(theme))),
            border: iced::Border {
                color: theme::divider(theme),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .into()
    } else {
        Space::new().height(0).into()
    };
    let watch_spacing: Element<'a, Message> = if watch_available {
        Space::new().height(12).into()
    } else {
        Space::new().height(0).into()
    };

    let form_fields = column![
        name_label,
        Space::new().height(8),
        name_input,
        Space::new().height(12),
        desc_label,
        Space::new().height(8),
        desc_input,
        watch_spacing,
        watch_section,
    ]
    .width(Fill);

    row![cover_section, Space::new().width(24), form_fields]
        .align_y(Alignment::Start)
        .into()
}
