//! Shared empty-chrome surface used to start native window dragging.

use iced::widget::{Space, container, mouse_area};
use iced::{Element, Fill, Length, mouse};

use crate::app::Message;
use crate::ui::responsive::{ResponsiveContext, top_bar_height};

const TOP_BAR_FRACTION: f32 = 2.0 / 3.0;

/// Resolve the drag layer height from the shared top-bar contract.
pub fn height(context: ResponsiveContext) -> f32 {
    top_bar_height(&context) * TOP_BAR_FRACTION
}

/// Build a full-width drag layer for use below real top-chrome controls.
pub fn view(context: ResponsiveContext) -> Element<'static, Message> {
    mouse_area(
        container(Space::new())
            .width(Fill)
            .height(Length::Fixed(height(context))),
    )
    .on_press(Message::WindowDrag)
    .interaction(mouse::Interaction::Grab)
    .into()
}

#[cfg(test)]
mod tests {
    use super::height;
    use crate::ui::responsive::{ResponsiveContext, top_bar_height};
    use iced::Size;

    #[test]
    fn drag_region_is_exactly_two_thirds_of_the_top_bar() {
        for viewport in [
            Size::new(1_920.0, 1_080.0),
            Size::new(960.0, 1_080.0),
            Size::new(960.0, 540.0),
        ] {
            let context = ResponsiveContext::from_viewport(viewport);
            let expected = top_bar_height(&context) * 2.0 / 3.0;
            assert!((height(context) - expected).abs() <= f32::EPSILON * expected.abs());
        }
    }
}
