//! Square cover art primitive that maintains 1:1 aspect ratio
//!
//! A low-level widget that implements iced's `Widget` trait to ensure
//! cover art is always displayed as a square, taking the minimum of
//! available width/height.
//!
//! # Design
//!
//! This is a primitive component - it uses generic Message types and
//! does not depend on application-specific types.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{self, Widget};
use iced::widget::{container, image, svg};
use iced::{Element, Length, Rectangle, Size, Theme};

use crate::ui::icons;

/// Create a square cover element that maintains 1:1 aspect ratio
pub fn view<'a, Message: 'a>(cover_path: Option<&str>) -> Element<'a, Message> {
    SquareCoverWidget::new(cover_path.map(String::from)).into()
}

/// A widget wrapper that forces square aspect ratio
pub struct SquareCoverWidget {
    cover_path: Option<String>,
}

impl SquareCoverWidget {
    pub fn new(cover_path: Option<String>) -> Self {
        Self { cover_path }
    }

    fn build_content<'a, Message: 'a>(&'a self, size: f32) -> Element<'a, Message> {
        if let Some(ref path) = self.cover_path {
            container(
                image(image::Handle::from_path(path))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .content_fit(iced::ContentFit::Cover)
                    .border_radius(12.0),
            )
            .width(size)
            .height(size)
            .style(|theme| iced::widget::container::Style {
                border: iced::Border {
                    radius: 12.0.into(),
                    ..Default::default()
                },
                shadow: iced::Shadow {
                    color: crate::ui::theme::shadow_color(theme),
                    offset: iced::Vector::new(0.0, 8.0),
                    blur_radius: 32.0,
                },
                ..Default::default()
            })
            .into()
        } else {
            container(
                svg(svg::Handle::from_memory(icons::MUSIC.as_bytes()))
                    .width(120)
                    .height(120)
                    .style(|_theme, _status| svg::Style {
                        color: Some(crate::ui::theme::icon_muted(&iced::Theme::Dark)),
                    }),
            )
            .width(size)
            .height(size)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(crate::ui::theme::placeholder_bg(
                    theme,
                ))),
                border: iced::Border {
                    radius: 12.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
        }
    }
}

impl<Message> Widget<Message, Theme, iced::Renderer> for SquareCoverWidget {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        // Get available width and use it as both width and height for square
        let max_width = limits.max().width;
        let square_size = max_width.min(limits.max().height).min(400.0); // Cap at 400

        let mut content: Element<'_, Message> = self.build_content(square_size);
        let child_limits = layout::Limits::new(Size::ZERO, Size::new(square_size, square_size));

        let mut child_node =
            content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, &child_limits);

        child_node = child_node.move_to(iced::Point::new((max_width - square_size) / 2.0, 0.0));

        layout::Node::with_children(Size::new(max_width, square_size), vec![child_node])
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: iced::mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let square_size = bounds.height;
        let content: Element<'_, Message> = self.build_content(square_size);

        if let Some(child_layout) = layout.children().next() {
            content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                child_layout,
                cursor,
                viewport,
            );
        }
    }

    fn children(&self) -> Vec<widget::Tree> {
        let content: Element<'_, Message> = self.build_content(100.0); // Dummy size for tree creation
        vec![widget::Tree::new(&content)]
    }

    fn diff(&self, tree: &mut widget::Tree) {
        let content: Element<'_, Message> = self.build_content(100.0);
        tree.diff_children(&[content]);
    }
}

impl<'a, Message: 'a> From<SquareCoverWidget> for Element<'a, Message> {
    fn from(widget: SquareCoverWidget) -> Self {
        Element::new(widget)
    }
}
