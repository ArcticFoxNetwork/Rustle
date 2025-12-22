//! Unified play mode button widget
//!
//! Provides a reusable play mode toggle button with tooltip.
//! Used by both the player bar and lyrics page.

use iced::widget::{button, svg, text, tooltip};
use iced::{Color, Element};

use crate::app::Message;
use crate::features::PlayMode;
use crate::ui::{icons, theme};

/// Size variant for play mode button
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ButtonSize {
    /// Small size for player bar (18px icon)
    Small,
    /// Large size for lyrics page (22px icon)
    Large,
}

impl ButtonSize {
    fn icon_size(&self) -> f32 {
        match self {
            Self::Small => 18.0,
            Self::Large => 22.0,
        }
    }

    fn padding(&self) -> f32 {
        match self {
            Self::Small => 8.0,
            Self::Large => 10.0,
        }
    }

    fn radius(&self) -> f32 {
        match self {
            Self::Small => 4.0,
            Self::Large => 21.0,
        }
    }
}

/// Build the play mode button with tooltip
pub fn view(play_mode: PlayMode, size: ButtonSize) -> Element<'static, Message> {
    let play_mode_icon = match play_mode {
        PlayMode::Sequential => icons::PLAY_SEQUENTIAL,
        PlayMode::LoopAll => icons::LOOP_ALL,
        PlayMode::LoopOne => icons::LOOP_ONE,
        PlayMode::Shuffle => icons::SHUFFLE,
    };
    let play_mode_tooltip = play_mode.display_name();

    let icon_size = size.icon_size();
    let padding = size.padding();
    let radius = size.radius();

    tooltip(
        button(
            svg(svg::Handle::from_memory(play_mode_icon.as_bytes()))
                .width(icon_size)
                .height(icon_size)
                .style(|_theme, _status| svg::Style {
                    color: Some(theme::TEXT_SECONDARY),
                }),
        )
        .padding(padding)
        .style(move |theme, status| {
            let bg = match status {
                button::Status::Hovered => crate::ui::theme::hover_bg(theme),
                _ => Color::TRANSPARENT,
            };
            button::Style {
                background: Some(iced::Background::Color(bg)),
                border: iced::Border {
                    radius: radius.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .on_press(Message::CyclePlayMode),
        text(play_mode_tooltip).size(12),
        tooltip::Position::Top,
    )
    .gap(4)
    .style(|theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(
            crate::ui::theme::surface_container(theme),
        )),
        border: iced::Border {
            radius: 4.0.into(),
            color: crate::ui::theme::divider(theme),
            width: 1.0,
        },
        ..Default::default()
    })
    .into()
}
