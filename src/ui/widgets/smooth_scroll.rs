//! Event adapter that adds smooth line-wheel behavior to native scrollables.

use iced::advanced::Shell;
use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::{Operation, Tree, Widget};
use iced::mouse::{self, Cursor};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

use crate::ui::animation::{SmoothScrollEvent, SmoothScrollTarget};

const LINE_SCROLL_PIXELS: f32 = 60.0;

/// Wraps a native Iced scrollable and converts vertical line-wheel input into
/// application-driven smooth-scroll requests.
pub struct SmoothScroll<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
    target: SmoothScrollTarget,
    on_event: Box<dyn Fn(SmoothScrollEvent) -> Message + 'a>,
}

impl<'a, Message, Theme, Renderer> SmoothScroll<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        target: SmoothScrollTarget,
        on_event: impl Fn(SmoothScrollEvent) -> Message + 'a,
    ) -> Self {
        Self {
            content: content.into(),
            target,
            on_event: Box::new(on_event),
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for SmoothScroll<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: renderer::Renderer,
{
    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let cursor_over = cursor.is_over(layout.bounds());

        if cursor_over
            && let Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { y, .. },
            }) = event
            && let Some(delta) = line_scroll_delta(*y)
        {
            shell.publish((self.on_event)(SmoothScrollEvent::Requested {
                target: self.target,
                delta,
            }));
            shell.capture_event();
            return;
        }

        if cancels_smooth_scroll(event, layout.bounds(), cursor) {
            shell.publish((self.on_event)(SmoothScrollEvent::Cancelled {
                target: self.target,
            }));
        }

        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            shell,
            viewport,
        );
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<SmoothScroll<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(scroll: SmoothScroll<'a, Message, Theme, Renderer>) -> Self {
        Element::new(scroll)
    }
}

pub fn smooth_scroll<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    target: SmoothScrollTarget,
    on_event: impl Fn(SmoothScrollEvent) -> Message + 'a,
) -> SmoothScroll<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    SmoothScroll::new(content, target, on_event)
}

fn line_scroll_delta(lines_y: f32) -> Option<f32> {
    let delta = -lines_y * LINE_SCROLL_PIXELS;
    (delta.abs() > f32::EPSILON).then_some(delta)
}

fn cancels_smooth_scroll(event: &Event, bounds: Rectangle, cursor: Cursor) -> bool {
    match event {
        Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Pixels { .. },
        })
        | Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left | mouse::Button::Middle)) => {
            cursor.is_over(bounds)
        }
        Event::Touch(iced::touch::Event::FingerPressed { position, .. }) => {
            bounds.contains(*position)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::line_scroll_delta;

    #[test]
    fn line_wheel_is_converted_to_content_offset_direction() {
        assert_eq!(line_scroll_delta(1.0), Some(-60.0));
        assert_eq!(line_scroll_delta(-2.0), Some(120.0));
        assert_eq!(line_scroll_delta(0.0), None);
    }
}
