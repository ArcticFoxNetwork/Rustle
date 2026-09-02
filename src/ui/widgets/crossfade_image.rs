//! Stateful image widget that cross-fades whenever its handle changes.

use std::time::Duration;

use iced::advanced::Shell;
use iced::advanced::image;
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{Tree, Widget, tree};
use iced::mouse::{self, Cursor};
use iced::time::Instant;
use iced::widget::image::{self as image_widget, FilterMethod};
use iced::{ContentFit, Element, Event, Length, Rectangle, Rotation, Size};

const DEFAULT_DURATION: Duration = Duration::from_millis(280);

/// An image that keeps the previous handle mounted while the next one fades in.
pub struct CrossfadeImage {
    handle: Option<image::Handle>,
    width: Length,
    height: Length,
    border_radius: iced::border::Radius,
    content_fit: ContentFit,
    filter_method: FilterMethod,
    scale: f32,
    duration: Duration,
}

impl CrossfadeImage {
    pub fn new(handle: Option<image::Handle>) -> Self {
        Self {
            handle,
            width: Length::Shrink,
            height: Length::Shrink,
            border_radius: iced::border::Radius::default(),
            content_fit: ContentFit::default(),
            filter_method: FilterMethod::default(),
            scale: 1.0,
            duration: DEFAULT_DURATION,
        }
    }

    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    #[must_use]
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    #[must_use]
    pub fn content_fit(mut self, content_fit: ContentFit) -> Self {
        self.content_fit = content_fit;
        self
    }

    #[must_use]
    pub fn border_radius(mut self, border_radius: impl Into<iced::border::Radius>) -> Self {
        self.border_radius = border_radius.into();
        self
    }

    #[must_use]
    pub fn scale(mut self, scale: impl Into<f32>) -> Self {
        self.scale = scale.into();
        self
    }

    #[must_use]
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }
}

#[derive(Debug, Default)]
struct State {
    previous: Option<image::Handle>,
    current: Option<image::Handle>,
    progress: f32,
    started_at: Option<Instant>,
}

impl State {
    fn sync_handle(&mut self, handle: &Option<image::Handle>, now: Instant) -> bool {
        if self.current == *handle {
            return false;
        }

        self.previous = self.current.take();
        self.current = handle.clone();
        self.progress = 0.0;
        self.started_at = Some(now);
        true
    }

    fn tick(&mut self, now: Instant, duration: Duration) -> bool {
        let Some(started_at) = self.started_at else {
            return false;
        };

        let elapsed = now.saturating_duration_since(started_at).as_secs_f32();
        let linear = (elapsed / duration.as_secs_f32().max(f32::EPSILON)).clamp(0.0, 1.0);
        self.progress = ease_out_cubic(linear);

        if linear >= 1.0 {
            self.progress = 1.0;
            self.previous = None;
            self.started_at = None;
            false
        } else {
            true
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for CrossfadeImage
where
    Renderer: renderer::Renderer + image::Renderer<Handle = image::Handle>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        if let Some(handle) = self.handle.as_ref() {
            image_widget::layout(
                renderer,
                limits,
                handle,
                self.width,
                self.height,
                None,
                self.content_fit,
                Rotation::default(),
                false,
            )
        } else {
            layout::Node::new(limits.resolve(self.width, self.height, Size::ZERO))
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        _layout: Layout<'_>,
        _cursor: Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let now = match event {
            Event::Window(iced::window::Event::RedrawRequested(now)) => *now,
            _ => Instant::now(),
        };

        if state.sync_handle(&self.handle, now) {
            shell.request_redraw();
        }

        if matches!(
            event,
            Event::Window(iced::window::Event::RedrawRequested(_))
        ) && state.tick(now, self.duration)
        {
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let progress = state.progress.clamp(0.0, 1.0);

        if let Some(previous) = state.previous.as_ref() {
            image_widget::draw(
                renderer,
                layout,
                previous,
                None,
                self.border_radius,
                self.content_fit,
                self.filter_method,
                Rotation::default(),
                1.0 - progress,
                self.scale,
            );
        }

        if let Some(current) = state.current.as_ref() {
            image_widget::draw(
                renderer,
                layout,
                current,
                None,
                self.border_radius,
                self.content_fit,
                self.filter_method,
                Rotation::default(),
                progress,
                self.scale,
            );
        }
    }
}

impl<'a, Message, Theme, Renderer> From<CrossfadeImage> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + image::Renderer<Handle = image::Handle> + 'a,
{
    fn from(image: CrossfadeImage) -> Self {
        Element::new(image)
    }
}

pub fn crossfade_image(handle: Option<image::Handle>) -> CrossfadeImage {
    CrossfadeImage::new(handle)
}

fn ease_out_cubic(value: f32) -> f32 {
    1.0 - (1.0 - value).powi(3)
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_DURATION, State, ease_out_cubic};
    use iced::time::Instant;
    use iced::widget::image;

    #[test]
    fn handle_changes_retain_the_previous_image_until_the_fade_finishes() {
        let now = Instant::now();
        let first = image::Handle::from_path("first.jpg");
        let second = image::Handle::from_path("second.jpg");
        let mut state = State::default();

        assert!(state.sync_handle(&Some(first.clone()), now));
        state.tick(now + DEFAULT_DURATION, DEFAULT_DURATION);
        assert_eq!(state.current, Some(first.clone()));
        assert_eq!(state.previous, None);

        assert!(state.sync_handle(&Some(second.clone()), now));
        assert_eq!(state.previous, Some(first));
        assert_eq!(state.current, Some(second));
    }

    #[test]
    fn easing_is_bounded_and_finishes_at_full_opacity() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        assert!(ease_out_cubic(0.5) > 0.5);
    }
}
