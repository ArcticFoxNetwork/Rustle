//! Unified playback control widgets
//!
//! Provides reusable playback controls (prev, play/pause, next) with consistent styling.
//! Used by both the player bar and lyrics page.

use iced::widget::{Space, button, container, row, svg};
use iced::{Alignment, Color, Element, Padding};

use crate::app::Message;
use crate::features::PlayMode;
use crate::ui::{icons, theme};

use super::play_mode_button::{self, ButtonSize as PlayModeButtonSize};

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
            Self::Small => 20.0,
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
pub fn play_button_with_buffering(
    is_playing: bool,
    is_buffering: bool,
    size: ControlSize,
) -> Element<'static, Message> {
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
    let icon_opacity = if show_loading { 0.4 } else { 1.0 };
    // Offset to visually center the triangle (play icon is not symmetric)
    let offset = if is_playing || show_loading {
        0.0
    } else if size == ControlSize::Small {
        2.0
    } else {
        3.0
    };

    let btn = button(
        container(
            svg(svg::Handle::from_memory(play_icon.as_bytes()))
                .width(icon_size)
                .height(icon_size)
                .style(move |theme, _status| svg::Style {
                    // Icon color should contrast with button background
                    color: Some(if show_loading {
                        theme::opaque_color(theme::icon_muted(theme))
                    } else {
                        theme::background(theme)
                    }),
                })
                .opacity(icon_opacity),
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
    .style(|_theme, _status| button::Style {
        background: Some(iced::Background::Color(Color::TRANSPARENT)),
        ..Default::default()
    })
    .on_press(Message::TogglePlayback);

    // Always enable button - user can pause even during buffering
    super::hover_surface(btn)
        .enabled(!show_loading)
        .style(move |theme, progress| {
            let background = if show_loading {
                theme::surface_container(theme)
            } else {
                theme::lerp_color(
                    theme::text_primary(theme),
                    theme::play_button_hover(theme),
                    progress,
                )
            };
            container::Style {
                background: Some(iced::Background::Color(background)),
                border: iced::Border {
                    radius: (btn_size / 2.0).into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

/// Build the previous song button
pub fn prev_button(size: ControlSize, disabled: bool) -> Element<'static, Message> {
    let icon_size = size.skip_icon_size();
    let padding = size.skip_button_padding();
    let radius = size.skip_button_radius();
    let icon_opacity = if disabled { 0.5 } else { 1.0 };

    let btn = button(
        svg(svg::Handle::from_memory(icons::SKIP_PREV.as_bytes()))
            .width(icon_size)
            .height(icon_size)
            .style(move |_theme, _status| svg::Style {
                color: Some(if disabled {
                    theme::opaque_color(theme::TEXT_DISABLED)
                } else {
                    theme::text_secondary(_theme)
                }),
            })
            .opacity(icon_opacity),
    )
    .padding(padding)
    .style(|_theme, _status| button::Style {
        background: Some(iced::Background::Color(Color::TRANSPARENT)),
        ..Default::default()
    });

    let btn = if disabled {
        btn
    } else {
        btn.on_press(Message::PrevSong)
    };

    super::hover_surface(btn)
        .enabled(!disabled)
        .style(move |theme, progress| container::Style {
            background: Some(iced::Background::Color(theme::hover_bg_alpha(
                theme,
                0.12 * progress,
            ))),
            border: iced::Border {
                radius: radius.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

/// Build the next song button
pub fn next_button(size: ControlSize) -> Element<'static, Message> {
    let icon_size = size.skip_icon_size();
    let padding = size.skip_button_padding();
    let radius = size.skip_button_radius();

    let btn = button(
        svg(svg::Handle::from_memory(icons::SKIP_NEXT.as_bytes()))
            .width(icon_size)
            .height(icon_size)
            .style(|_theme, _status| svg::Style {
                color: Some(theme::text_secondary(_theme)),
            }),
    )
    .padding(padding)
    .style(|_theme, _status| button::Style {
        background: Some(iced::Background::Color(Color::TRANSPARENT)),
        ..Default::default()
    })
    .on_press(Message::NextSong);

    super::hover_surface(btn)
        .style(move |theme, progress| container::Style {
            background: Some(iced::Background::Color(theme::hover_bg_alpha(
                theme,
                0.12 * progress,
            ))),
            border: iced::Border {
                radius: radius.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

/// Build the favorite button used by the player bar.
///
/// `favorite` contains the NCM song ID and current favorite state. Local songs
/// and an empty player pass `None`, which keeps the button visible but disabled.
pub fn favorite_button(
    size: ControlSize,
    favorite: Option<(u64, bool)>,
) -> Element<'static, Message> {
    let icon_size = size.skip_icon_size();
    let padding = size.skip_button_padding();
    let radius = size.skip_button_radius();
    let is_liked = favorite.is_some_and(|(_, is_liked)| is_liked);
    let enabled = favorite.is_some();
    let icon_opacity = if enabled { 1.0 } else { 0.4 };
    let heart_icon = if is_liked {
        icons::HEART
    } else {
        icons::HEART_OUTLINE
    };

    let btn = button(
        svg(svg::Handle::from_memory(heart_icon.as_bytes()))
            .width(icon_size)
            .height(icon_size)
            .style(move |theme, _status| svg::Style {
                color: Some(if !enabled {
                    theme::opaque_color(theme::icon_muted(theme))
                } else if is_liked {
                    theme::ACCENT_PINK
                } else {
                    theme::text_secondary(theme)
                }),
            })
            .opacity(icon_opacity),
    )
    .padding(padding)
    .style(|_theme, _status| button::Style {
        background: Some(iced::Background::Color(Color::TRANSPARENT)),
        ..Default::default()
    });

    let btn = if let Some((song_id, _)) = favorite {
        btn.on_press(Message::ToggleFavorite(song_id))
    } else {
        btn
    };

    super::hover_surface(btn)
        .enabled(enabled)
        .style(move |theme, progress| container::Style {
            background: Some(iced::Background::Color(theme::hover_bg_alpha(
                theme,
                0.12 * progress,
            ))),
            border: iced::Border {
                radius: radius.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

/// Build the player bar controls, including play mode and favorite actions.
pub fn view_player_bar(
    is_playing: bool,
    is_buffering: bool,
    size: ControlSize,
    is_fm_mode: bool,
    is_first_song: bool,
    play_mode: PlayMode,
    favorite: Option<(u64, bool)>,
) -> Element<'static, Message> {
    let spacing = size.spacing();
    let prev_disabled = is_fm_mode && is_first_song;

    row![
        play_mode_button::view(play_mode, PlayModeButtonSize::Small, is_fm_mode),
        Space::new().width(spacing),
        prev_button(size, prev_disabled),
        Space::new().width(spacing),
        play_button_with_buffering(is_playing, is_buffering, size),
        Space::new().width(spacing),
        next_button(size),
        Space::new().width(spacing),
        favorite_button(size, favorite),
    ]
    .align_y(Alignment::Center)
    .into()
}

/// Build the complete playback controls row with buffering state
pub fn view(
    is_playing: bool,
    is_buffering: bool,
    size: ControlSize,
    is_fm_mode: bool,
    is_first_song: bool,
) -> Element<'static, Message> {
    let spacing = size.spacing();
    let prev_disabled = is_fm_mode && is_first_song;

    row![
        prev_button(size, prev_disabled),
        Space::new().width(spacing),
        play_button_with_buffering(is_playing, is_buffering, size),
        Space::new().width(spacing),
        next_button(size),
    ]
    .align_y(Alignment::Center)
    .into()
}

/// Build the complete playback controls row (prev, play, next)
pub fn view_simple(is_playing: bool, size: ControlSize) -> Element<'static, Message> {
    view(is_playing, false, size, false, false)
}
