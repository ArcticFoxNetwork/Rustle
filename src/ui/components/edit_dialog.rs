//! Edit playlist dialog component

use iced::mouse::Interaction;
use iced::widget::{
    Space, button, column, container, image, mouse_area, opaque, row, stack, svg, text, text_input,
};
use iced::{Alignment, Color, Element, Fill, Padding};

use crate::app::Message;
use crate::i18n::{Key, Locale};
use crate::ui::theme::BOLD_WEIGHT;
use crate::ui::{icons, theme};

/// Build the edit playlist dialog with animation
pub fn view<'a>(
    name: &str,
    description: &str,
    cover_path: Option<&str>,
    animation_progress: f32,
    locale: Locale,
) -> Element<'a, Message> {
    // Animation: opacity (iced doesn't support scale transforms)
    let opacity = animation_progress;

    // Semi-transparent backdrop with animated opacity - blocks clicks
    let backdrop_opacity = 0.7 * animation_progress;
    let backdrop = mouse_area(container(Space::new()).width(Fill).height(Fill).style(
        move |theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::overlay_backdrop(
                theme,
                backdrop_opacity,
            ))),
            ..Default::default()
        },
    ))
    .on_press(Message::CloseEditDialog);

    // Dialog content
    let title = text(locale.get(Key::EditPlaylistTitle).to_string())
        .size(24)
        .color(theme::TEXT_PRIMARY)
        .font(iced::Font {
            weight: BOLD_WEIGHT,
            ..Default::default()
        });

    // Cover image section
    let cover_content: Element<'a, Message> = if let Some(path) = cover_path {
        image(path)
            .width(120)
            .height(120)
            .content_fit(iced::ContentFit::Cover)
            .into()
    } else {
        // Placeholder with music icon
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

    let cover_box = container(cover_content).style(|_theme| iced::widget::container::Style {
        border: iced::Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    let change_cover_btn = button(
        text(locale.get(Key::EditPlaylistChangeCover).to_string())
            .size(13)
            .color(theme::TEXT_PRIMARY),
    )
    .padding(Padding::new(6.0).left(12.0).right(12.0))
    .style(|theme, status| {
        let bg = match status {
            button::Status::Hovered => theme::hover_bg(theme),
            _ => theme::surface_container(theme),
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            border: iced::Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            text_color: theme::TEXT_PRIMARY,
            ..Default::default()
        }
    })
    .on_press(Message::PickCoverImage);

    let cover_section =
        column![cover_box, Space::new().height(8), change_cover_btn,].align_x(Alignment::Center);

    let name_label = text(locale.get(Key::EditPlaylistName).to_string())
        .size(14)
        .color(theme::TEXT_SECONDARY);
    let name_input = text_input(locale.get(Key::EditPlaylistNamePlaceholder), name)
        .on_input(Message::EditPlaylistNameChanged)
        .padding(12)
        .size(15)
        .style(|theme, _status| text_input::Style {
            background: iced::Background::Color(theme::surface_container(theme)),
            border: iced::Border {
                color: theme::divider(theme),
                width: 1.0,
                radius: 6.0.into(),
            },
            icon: theme::TEXT_MUTED,
            placeholder: theme::TEXT_MUTED,
            value: theme::TEXT_PRIMARY,
            selection: theme::ACCENT_PINK,
        });

    let desc_label = text(locale.get(Key::EditPlaylistDesc).to_string())
        .size(14)
        .color(theme::TEXT_SECONDARY);
    let desc_input = text_input(locale.get(Key::EditPlaylistDescPlaceholder), description)
        .on_input(Message::EditPlaylistDescriptionChanged)
        .padding(12)
        .size(15)
        .style(|theme, _status| text_input::Style {
            background: iced::Background::Color(theme::surface_container(theme)),
            border: iced::Border {
                color: theme::divider(theme),
                width: 1.0,
                radius: 6.0.into(),
            },
            icon: theme::TEXT_MUTED,
            placeholder: theme::TEXT_MUTED,
            value: theme::TEXT_PRIMARY,
            selection: theme::ACCENT_PINK,
        });

    // Buttons with smooth hover transitions
    let cancel_btn = button(
        text(locale.get(Key::Cancel).to_string())
            .size(14)
            .color(theme::TEXT_PRIMARY),
    )
    .padding(Padding::new(10.0).left(24.0).right(24.0))
    .style(|theme, status| {
        let (bg, border_alpha) = match status {
            button::Status::Hovered => (theme::hover_bg(theme), 0.5),
            button::Status::Pressed => (theme::hover_bg(theme), 0.6),
            _ => (Color::TRANSPARENT, 0.3),
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            border: iced::Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, border_alpha),
                width: 1.0,
                radius: 20.0.into(),
            },
            text_color: theme::TEXT_PRIMARY,
            ..Default::default()
        }
    })
    .on_press(Message::CloseEditDialog);

    let save_btn = button(
        text(locale.get(Key::Save).to_string())
            .size(14)
            .color(theme::BLACK),
    )
    .padding(Padding::new(10.0).left(24.0).right(24.0))
    .style(|_theme, status| {
        let bg = match status {
            button::Status::Hovered => theme::ACCENT_PINK_HOVER,
            button::Status::Pressed => theme::ACCENT_PINK,
            _ => theme::ACCENT_PINK,
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            border: iced::Border {
                radius: 20.0.into(),
                ..Default::default()
            },
            text_color: theme::BLACK,
            ..Default::default()
        }
    })
    .on_press(Message::SavePlaylistEdits);

    let buttons = row![cancel_btn, Space::new().width(12), save_btn,].align_y(Alignment::Center);

    // Form fields column - reduced spacing between name and description
    let form_fields = column![
        name_label,
        Space::new().height(8),
        name_input,
        Space::new().height(12),
        desc_label,
        Space::new().height(8),
        desc_input,
    ]
    .width(Fill);

    // Cover + form row
    let content_row =
        row![cover_section, Space::new().width(24), form_fields,].align_y(Alignment::Start);

    let dialog_content = column![
        title,
        Space::new().height(24),
        content_row,
        Space::new().height(28),
        container(buttons).width(Fill).align_x(Alignment::End),
    ]
    .width(480)
    .padding(28);

    // Apply animation opacity to dialog
    let dialog_box = container(dialog_content).style(move |theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(Color::from_rgba(
            if theme::is_dark_theme(theme) {
                0.1
            } else {
                0.95
            },
            if theme::is_dark_theme(theme) {
                0.1
            } else {
                0.95
            },
            if theme::is_dark_theme(theme) {
                0.1
            } else {
                0.95
            },
            opacity,
        ))),
        border: iced::Border {
            color: theme::divider(theme),
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    });

    // Center the dialog in a container
    let dialog_centered = container(dialog_box)
        .width(Fill)
        .height(Fill)
        .center_x(Fill)
        .center_y(Fill);

    // Stack backdrop and dialog
    let dialog_stack = stack![backdrop, dialog_centered,].width(Fill).height(Fill);

    // Wrap in mouse_area with Idle interaction to reset cursor
    // and on_press to capture any stray clicks
    let event_blocker = mouse_area(dialog_stack)
        .interaction(Interaction::Idle)
        .on_press(Message::Noop);

    // Use opaque to block all mouse button events from propagating to underlying widgets
    opaque(event_blocker).into()
}
