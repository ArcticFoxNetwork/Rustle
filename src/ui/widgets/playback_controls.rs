//! Unified playback control widgets
//!
//! Provides reusable playback controls (prev, play/pause, next) with consistent styling.
//! Used by both the player bar and lyrics page.

use iced::widget::{Space, button, container, row, svg};
use iced::{Alignment, Color, Element, Padding};

use crate::app::Message;
use crate::ui::{icons, theme};

/// Size variant for playback controls
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControlSize {
    /// Small size for player bar (40px play button)
    Small,
    /// Large size for lyrics page (64px play button)
    Large,
}

impl ControlSize {
    fn play_button_size(&self) -> f32 {
        match self {
            Self::Small => 40.0,
            Self::Large => 64.0,
        }
    }

    fn play_icon_size(&self) -> f32 {
        match self {
            Self::Small => 18.0,
            Self::Large => 28.0,
        }
    }

    fn skip_icon_size(&self) -> f32 {
        match self {
            Self::Small => 20.0,
            Self::Large => 28.0,
        }
    }

    fn skip_button_padding(&self) -> f32 {
        match self {
            Self::Small => 8.0,
            Self::Large => 12.0,
        }
    }

    fn skip_button_radius(&self) -> f32 {
        match self {
            Self::Small => 20.0,
            Self::Large => 26.0,
        }
    }

    fn spacing(&self) -> f32 {
        match self {
            Self::Small => 8.0,
            Self::Large => 16.0,
        }
    }
}

/// Build the play/pause button with buffering state
pub fn play_button_with_buffering(is_playing: bool, is_buffering: bool, size: ControlSize) -> Element<'static, Message> {
    // Only show loading icon when playing AND buffering
    let show_loading = is_playing && is_buffering;
    let play_icon = if show_loading {
        icons::LOADING
    } else if is_playing {
        icons::PAUSE
    } else {
        icons::PLAY
    };

    let btn_size = size.play_button_size();
    let icon_size = size.play_icon_size();
    let inner_padding = (btn_size - icon_size) / 2.0;
    // Offset to visually center the triangle (play icon is not symmetric)
    let offset = if is_playing || show_loading {
        0.0
    } else {
        if size == ControlSize::Small { 2.0 } else { 3.0 }
    };

    let btn = button(
        container(
            svg(svg::Handle::from_memory(play_icon.as_bytes()))
                .width(icon_size)
                .height(icon_size)
                .style(move |theme, _status| svg::Style {
                    // Icon color should contrast with button background
                    color: Some(if show_loading {
                        theme::icon_muted(theme)
                    } else {
                        theme::background(theme)
                    }),
                }),
        )
        .padding(Padding {
            top: inner_padding,
            bottom: inner_padding,
            left: inner_padding + offset,
            right: inner_padding - offset,
        }),
    )
    .padding(0)
    .width(btn_size)
    .height(btn_size)
    .style(move |theme, status| {
        let bg = if show_loading {
            theme::surface_container(theme)
        } else {
            match status {
                button::Status::Hovered => theme::play_button_hover(theme),
                _ => theme::text_primary(theme),
            }
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            border: iced::Border {
                radius: (btn_size / 2.0).into(),
                ..Default::default()
            },
            ..Default::default()
        }
    });

    // Always enable button - user can pause even during buffering
    btn.on_press(Message::TogglePlayback).into()
}

/// Build the previous song button
pub fn prev_button(size: ControlSize) -> Element<'static, Message> {
    let icon_size = size.skip_icon_size();
    let padding = size.skip_button_padding();
    let radius = size.skip_button_radius();

    button(
        svg(svg::Handle::from_memory(icons::SKIP_PREV.as_bytes()))
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
    .on_press(Message::PrevSong)
    .into()
}

/// Build the next song button
pub fn next_button(size: ControlSize) -> Element<'static, Message> {
    let icon_size = size.skip_icon_size();
    let padding = size.skip_button_padding();
    let radius = size.skip_button_radius();

    button(
        svg(svg::Handle::from_memory(icons::SKIP_NEXT.as_bytes()))
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
    .on_press(Message::NextSong)
    .into()
}

/// Build the complete playback controls row (prev, play, next)
pub fn view(is_playing: bool, size: ControlSize) -> Element<'static, Message> {
    view_with_buffering(is_playing, false, size)
}

/// Build the complete playback controls row with buffering state
pub fn view_with_buffering(is_playing: bool, is_buffering: bool, size: ControlSize) -> Element<'static, Message> {
    let spacing = size.spacing();

    row![
        prev_button(size),
        Space::new().width(spacing),
        play_button_with_buffering(is_playing, is_buffering, size),
        Space::new().width(spacing),
        next_button(size),
    ]
    .align_y(Alignment::Center)
    .into()
}
