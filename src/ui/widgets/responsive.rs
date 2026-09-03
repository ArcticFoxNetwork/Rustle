//! Responsive layout helpers shared by grid-like pages.

use iced::widget::{Sensor, container, scrollable};
use iced::{Element, Fill};

use crate::ui::animation::{SmoothScrollEvent, SmoothScrollTarget};
use crate::ui::responsive::ResponsiveContext;
use crate::ui::widgets::smooth_scroll;

// Compatibility wrappers keep the existing `ui::widgets` and page callers
// stable while the pure policy now lives in `ui::responsive`.
pub use crate::ui::responsive::calculate_grid_columns;

/// Derive usable content width from a measured container size.
pub fn usable_content_width(size: iced::Size, horizontal_padding: f32) -> f32 {
    ResponsiveContext::from_viewport(size).usable_content_width(horizontal_padding)
}

/// Scrollable content that reports its rendered width through [`Sensor`].
pub fn measured_scrollable<'a, Message, F>(
    content: impl Into<Element<'a, Message>>,
    scroll_id: &'static str,
    on_resize: F,
    on_scroll_event: impl Fn(SmoothScrollEvent) -> Message + 'a,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
    F: Fn(iced::Size) -> Message + Clone + 'static,
{
    let measured_content = Sensor::new(container(content).width(Fill))
        .on_show(on_resize.clone())
        .on_resize(on_resize);

    smooth_scroll(
        scrollable(measured_content)
            .width(Fill)
            .height(Fill)
            .id(iced::widget::Id::new(scroll_id))
            .style(crate::ui::theme::dark_scrollable),
        SmoothScrollTarget::Native(scroll_id),
        on_scroll_event,
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::{calculate_grid_columns, usable_content_width};
    use crate::ui::responsive::calculate_grid_columns_clamped;

    #[test]
    fn usable_width_respects_padding() {
        assert_eq!(
            usable_content_width(iced::Size::new(1024.0, 768.0), 64.0),
            960.0
        );
    }

    #[test]
    fn usable_width_has_floor() {
        assert_eq!(
            usable_content_width(iced::Size::new(120.0, 768.0), 64.0),
            200.0
        );
    }

    #[test]
    fn grid_columns_expand_with_width() {
        assert_eq!(calculate_grid_columns(160.0, 160.0, 24.0), 1);
        assert_eq!(calculate_grid_columns(528.0, 160.0, 24.0), 3);
    }

    #[test]
    fn clamped_columns_respect_maximum() {
        assert_eq!(calculate_grid_columns_clamped(4000.0, 160.0, 24.0, 5), 5);
    }
}
