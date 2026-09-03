//! Unified play mode button widget
//!
//! Provides a reusable play mode toggle button with tooltip.
//! Used by both the player bar and lyrics page.

use iced::widget::{button, container, svg, text, tooltip};
use iced::{Color, Element};

use crate::ui::theme;

/// Size variant for play mode button
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ButtonSize {
    /// Small size for player bar (18px icon)
    Small,
    /// Large size for lyrics page (22px icon)
    Large,
    /// Density-scaled small control used by the player bar.
    ScaledSmall(f32),
    /// Density-scaled large control used by full-screen control surfaces.
    ScaledLarge(f32),
}

impl ButtonSize {
    fn icon_size(&self) -> f32 {
        match self {
            Self::Small => 20.0,
            Self::Large => 22.0,
            Self::ScaledSmall(scale) => 20.0 * scale.max(0.1),
            Self::ScaledLarge(scale) => 22.0 * scale.max(0.1),
        }
    }

    fn padding(&self) -> f32 {
        match self {
            Self::Small => 8.0,
            Self::Large => 10.0,
            Self::ScaledSmall(scale) => 8.0 * scale.max(0.1),
            Self::ScaledLarge(scale) => 10.0 * scale.max(0.1),
        }
    }

    fn radius(&self) -> f32 {
        (self.icon_size() + self.padding() * 2.0) / 2.0
    }
}

/// Build the play mode button with tooltip
pub fn view<M: Clone + 'static>(
    play_mode_icon: &'static str,
    play_mode_tooltip: &'static str,
    size: ButtonSize,
    on_press: M,
) -> Element<'static, M> {
    let icon_size = size.icon_size();
    let padding = size.padding();
    let radius = size.radius();

    let button = button(
        svg(svg::Handle::from_memory(play_mode_icon.as_bytes()))
            .width(icon_size)
            .height(icon_size)
            .style(move |_theme, _status| svg::Style {
                color: Some(theme::text_secondary(_theme)),
            }),
    )
    .padding(padding)
    .style(|_theme, _status| button::Style {
        background: Some(iced::Background::Color(Color::TRANSPARENT)),
        ..Default::default()
    })
    .on_press(on_press);
    let button = super::hover_surface(button).style(move |theme, progress| container::Style {
        background: Some(iced::Background::Color(theme::hover_bg_alpha(
            theme,
            0.12 * progress,
        ))),
        border: iced::Border {
            radius: radius.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    tooltip(
        button,
        text(play_mode_tooltip).size(theme::TEXT_SIZE_CAPTION),
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

#[cfg(test)]
mod tests {
    use super::ButtonSize;

    #[test]
    fn hover_backgrounds_are_circular_at_every_size() {
        assert_eq!(ButtonSize::Small.radius(), 18.0);
        assert_eq!(ButtonSize::Large.radius(), 21.0);
    }
}
