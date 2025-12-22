//! Virtual List Primitive
//!
//! A high-performance virtualized list component that only renders visible items.
//! Inspired by Android's RecyclerView, this component can handle large lists
//! while maintaining smooth scrolling.
//!
//! # Design
//!
//! This is a primitive component that implements iced's `Widget` trait.
//! It uses generic Message and Theme types and does not depend on
//! application-specific types.
//!
//! # Key Features
//!
//! - Tree Diffing: Preserves widget state (focus, animations, etc.) when scrolling
//! - Scrollbar: Visual indicator of scroll position
//! - Buffer items: Renders extra items above/below viewport for smooth scrolling
//! - Element Caching: Each item_builder is called only once per frame
//! - Optimized: Minimizes item_builder calls per frame

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::mouse::{self, Cursor};
use iced::{Color, Element, Event, Length, Point, Rectangle, Size};
use std::cell::RefCell;
use std::rc::Rc;

/// Buffer items to render above and below the visible area
const BUFFER_ITEMS: usize = 3;

/// Scrollbar configuration
const SCROLLBAR_WIDTH: f32 = 6.0;
const SCROLLBAR_MIN_HEIGHT: f32 = 30.0;
const SCROLLBAR_MARGIN: f32 = 2.0;
const SCROLLBAR_BORDER_RADIUS: f32 = 3.0;

/// State for the virtual list
#[derive(Debug, Clone)]
pub struct VirtualListState {
    /// Current scroll offset in pixels
    pub scroll_offset: f32,
    /// Viewport height in pixels
    pub viewport_height: f32,
    /// Total item count
    pub item_count: usize,
    /// Item height
    pub item_height: f32,
}

impl Default for VirtualListState {
    fn default() -> Self {
        Self {
            scroll_offset: 0.0,
            viewport_height: 0.0,
            item_count: 0,
            item_height: 62.0,
        }
    }
}

impl VirtualListState {
    /// Create a new state with the given item count and default height
    pub fn new(item_count: usize, item_height: f32) -> Self {
        Self {
            scroll_offset: 0.0,
            viewport_height: 0.0,
            item_count,
            item_height,
        }
    }

    /// Get total content height
    pub fn total_height(&self) -> f32 {
        self.item_count as f32 * self.item_height
    }

    /// Calculate visible range with buffer
    pub fn visible_range(&self) -> (usize, usize) {
        if self.item_count == 0 || self.viewport_height <= 0.0 {
            return (0, 0);
        }

        let first_visible = (self.scroll_offset / self.item_height).floor() as usize;
        let visible_count = (self.viewport_height / self.item_height).ceil() as usize + 1;

        let start = first_visible.saturating_sub(BUFFER_ITEMS);
        let end = (first_visible + visible_count + BUFFER_ITEMS).min(self.item_count);

        (start, end)
    }

    /// Update state
    pub fn update(&mut self, item_count: usize, item_height: f32, viewport_height: f32) {
        self.item_count = item_count;
        self.item_height = item_height;
        self.viewport_height = viewport_height;

        // Clamp scroll offset
        let max_scroll = self.max_scroll();
        self.scroll_offset = self.scroll_offset.clamp(0.0, max_scroll);
    }

    /// Get maximum scroll offset
    pub fn max_scroll(&self) -> f32 {
        (self.total_height() - self.viewport_height).max(0.0)
    }
}

/// A virtual list widget that only renders visible items
pub struct VirtualList<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    /// Total number of items
    item_count: usize,
    /// Item height
    item_height: f32,
    /// Function to build an item element by index
    item_builder: Box<dyn Fn(usize) -> Element<'a, Message, Theme, Renderer> + 'a>,
    /// Shared state
    state: Rc<RefCell<VirtualListState>>,
    /// Width of the list
    width: Length,
    /// Height of the list
    height: Length,
    /// Whether to show scrollbar
    show_scrollbar: bool,
}

impl<'a, Message, Theme, Renderer> VirtualList<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    /// Create a new virtual list
    pub fn new<F>(item_count: usize, item_height: f32, item_builder: F) -> Self
    where
        F: Fn(usize) -> Element<'a, Message, Theme, Renderer> + 'a,
    {
        let state = Rc::new(RefCell::new(VirtualListState::new(item_count, item_height)));
        Self {
            item_count,
            item_height,
            item_builder: Box::new(item_builder),
            state,
            width: Length::Fill,
            height: Length::Fill,
            show_scrollbar: true,
        }
    }

    /// Set the width of the list
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Set the height of the list
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Set external state (for persistence across frames)
    pub fn state(mut self, state: Rc<RefCell<VirtualListState>>) -> Self {
        self.state = state;
        self
    }

    /// Set spacing between items (not used in fixed height mode)
    pub fn spacing(self, _spacing: f32) -> Self {
        self
    }

    /// Show or hide the scrollbar
    pub fn scrollbar(mut self, show: bool) -> Self {
        self.show_scrollbar = show;
        self
    }
}

/// Internal state for widget tree
struct VirtualListInternalState {
    /// Cached trees for visible items
    visible_trees: Vec<Tree>,
    /// Cached visible range from layout phase
    cached_visible_range: (usize, usize),
    /// Cached item indices for each tree slot
    cached_item_indices: Vec<usize>,
    /// Whether the scrollbar is being dragged
    scrollbar_dragging: bool,
    /// The Y position where drag started (relative to scrollbar top)
    drag_start_offset: f32,
    /// Whether mouse is hovering over scrollbar
    scrollbar_hovered: bool,
    /// Frame counter to track when elements need rebuilding
    frame_id: u64,
    /// Last frame's visible range for smart diffing
    last_visible_range: (usize, usize),
}

impl Default for VirtualListInternalState {
    fn default() -> Self {
        Self {
            visible_trees: Vec::new(),
            cached_visible_range: (0, 0),
            cached_item_indices: Vec::new(),
            scrollbar_dragging: false,
            drag_start_offset: 0.0,
            scrollbar_hovered: false,
            frame_id: 0,
            last_visible_range: (0, 0),
        }
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for VirtualList<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Renderer: renderer::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<VirtualListInternalState>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(VirtualListInternalState::default())
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let limits = limits.width(self.width).height(self.height);
        let size = limits.resolve(self.width, self.height, Size::ZERO);

        // Update state
        {
            let mut state = self.state.borrow_mut();
            state.update(self.item_count, self.item_height, size.height);
        }

        let state = self.state.borrow();
        let (start, end) = state.visible_range();
        let visible_count = end - start;
        let scroll_offset = state.scroll_offset;
        drop(state);

        // Get internal state
        let internal_state = tree.state.downcast_mut::<VirtualListInternalState>();

        // Increment frame counter
        internal_state.frame_id = internal_state.frame_id.wrapping_add(1);

        // Track visible range for potential future optimizations
        internal_state.last_visible_range = (start, end);

        // Resize vectors to match visible count
        internal_state
            .visible_trees
            .resize_with(visible_count, Tree::empty);
        internal_state.cached_item_indices.resize(visible_count, 0);

        // Cache the visible range
        internal_state.cached_visible_range = (start, end);

        // Build layout for visible items - call item_builder only ONCE per item
        let mut children = Vec::with_capacity(visible_count);
        let item_limits = layout::Limits::new(Size::ZERO, Size::new(size.width, self.item_height));

        for (slot_idx, item_idx) in (start..end).enumerate() {
            // Build element once
            let mut element = (self.item_builder)(item_idx);

            // Diff tree with element (preserves widget state)
            internal_state.visible_trees[slot_idx].diff(&element);
            internal_state.cached_item_indices[slot_idx] = item_idx;

            // Layout the element
            let item_tree = &mut internal_state.visible_trees[slot_idx];
            let node = element
                .as_widget_mut()
                .layout(item_tree, renderer, &item_limits);

            // Position the node based on its index in the full list
            let y_position = item_idx as f32 * self.item_height - scroll_offset;
            let positioned = node.move_to(Point::new(0.0, y_position));
            children.push(positioned);
        }

        layout::Node::with_children(size, children)
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
        let bounds = layout.bounds();
        let internal_state = tree.state.downcast_ref::<VirtualListInternalState>();

        // Early return if no items to draw
        if internal_state.cached_item_indices.is_empty() {
            return;
        }

        // Pre-calculate the visible slot range to minimize item_builder calls
        let (visible_start_slot, visible_end_slot) = {
            let state = self.state.borrow();
            let scroll_offset = state.scroll_offset;

            let mut start_slot: Option<usize> = None;
            let mut end_slot = 0usize;

            for (slot_idx, &item_idx) in internal_state.cached_item_indices.iter().enumerate() {
                let y = item_idx as f32 * self.item_height - scroll_offset;
                let y_end = y + self.item_height;

                if y_end > 0.0 && y < bounds.height {
                    if start_slot.is_none() {
                        start_slot = Some(slot_idx);
                    }
                    end_slot = slot_idx + 1;
                }
            }

            match start_slot {
                Some(start) => (start, end_slot),
                None if !internal_state.cached_item_indices.is_empty() => {
                    (0, internal_state.cached_item_indices.len())
                }
                None => (0, 0),
            }
        };

        // Draw list items (clipped to bounds)
        renderer.with_layer(bounds, |renderer| {
            let children: Vec<_> = layout.children().collect();

            for slot_idx in visible_start_slot..visible_end_slot {
                if slot_idx >= internal_state.cached_item_indices.len() {
                    break;
                }

                let item_idx = internal_state.cached_item_indices[slot_idx];
                let child_layout = children[slot_idx];

                if slot_idx < internal_state.visible_trees.len() {
                    let element = (self.item_builder)(item_idx);
                    let child_tree = &internal_state.visible_trees[slot_idx];

                    element.as_widget().draw(
                        child_tree,
                        renderer,
                        theme,
                        style,
                        child_layout,
                        cursor,
                        viewport,
                    );
                }
            }
        });

        // Draw scrollbar overlay
        if self.show_scrollbar {
            let state = self.state.borrow();
            if state.total_height() > bounds.height {
                self.draw_scrollbar(
                    renderer,
                    bounds,
                    &state,
                    internal_state.scrollbar_hovered,
                    internal_state.scrollbar_dragging,
                );
            }
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let internal_state = tree.state.downcast_mut::<VirtualListInternalState>();

        let scrollbar_bounds = {
            let state = self.state.borrow();
            self.calculate_scrollbar_bounds(bounds, &state)
        };

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(position) = cursor.position() {
                    if let Some(sb_bounds) = scrollbar_bounds {
                        if sb_bounds.contains(position) {
                            internal_state.scrollbar_dragging = true;
                            internal_state.drag_start_offset = position.y - sb_bounds.y;
                            shell.capture_event();
                            return;
                        }
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if internal_state.scrollbar_dragging {
                    internal_state.scrollbar_dragging = false;
                    shell.capture_event();
                    return;
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if let Some(sb_bounds) = scrollbar_bounds {
                    let was_hovered = internal_state.scrollbar_hovered;
                    internal_state.scrollbar_hovered = sb_bounds.contains(*position);
                    if was_hovered != internal_state.scrollbar_hovered {
                        shell.request_redraw();
                    }
                }

                if internal_state.scrollbar_dragging {
                    let state_ref = self.state.borrow();
                    let total_height = state_ref.total_height();
                    let max_scroll = state_ref.max_scroll();
                    drop(state_ref);

                    if max_scroll > 0.0 && total_height > 0.0 {
                        let view_ratio = bounds.height / total_height;
                        let scrollbar_height =
                            (bounds.height * view_ratio).max(SCROLLBAR_MIN_HEIGHT);
                        let available_track = bounds.height - scrollbar_height;

                        if available_track > 0.0 {
                            let scrollbar_top =
                                position.y - bounds.y - internal_state.drag_start_offset;
                            let scroll_ratio = (scrollbar_top / available_track).clamp(0.0, 1.0);
                            let new_offset = scroll_ratio * max_scroll;

                            let mut state = self.state.borrow_mut();
                            if (new_offset - state.scroll_offset).abs() > 0.01 {
                                state.scroll_offset = new_offset;
                                shell.invalidate_layout();
                            }
                        }
                    }
                    shell.capture_event();
                    return;
                }
            }
            _ => {}
        }

        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event {
            if let Some(position) = cursor.position() {
                if bounds.contains(position) {
                    let delta_y = match delta {
                        mouse::ScrollDelta::Lines { y, .. } => y * 50.0,
                        mouse::ScrollDelta::Pixels { y, .. } => *y,
                    };

                    let mut state = self.state.borrow_mut();
                    let max_scroll = state.max_scroll();
                    let new_offset = (state.scroll_offset - delta_y).clamp(0.0, max_scroll);

                    if (new_offset - state.scroll_offset).abs() > 0.01 {
                        state.scroll_offset = new_offset;
                        shell.invalidate_layout();
                    }
                    shell.capture_event();
                }
            }
        }

        if internal_state.scrollbar_dragging {
            return;
        }

        let cursor_pos = cursor.position();
        let cursor_in_bounds = cursor_pos.map(|pos| bounds.contains(pos)).unwrap_or(false);

        match event {
            Event::Mouse(_) => {
                if !cursor_in_bounds {
                    return;
                }

                if let Some(pos) = cursor_pos {
                    let state = self.state.borrow();
                    let scroll_offset = state.scroll_offset;
                    drop(state);

                    let relative_y = pos.y - bounds.y + scroll_offset;
                    let target_item_idx = (relative_y / self.item_height).floor() as usize;

                    let children: Vec<_> = layout.children().collect();
                    for (slot_idx, &item_idx) in
                        internal_state.cached_item_indices.iter().enumerate()
                    {
                        if item_idx == target_item_idx
                            && slot_idx < internal_state.visible_trees.len()
                            && slot_idx < children.len()
                        {
                            let mut element = (self.item_builder)(item_idx);
                            let child_tree = &mut internal_state.visible_trees[slot_idx];
                            let child_layout = children[slot_idx];

                            element.as_widget_mut().update(
                                child_tree,
                                event,
                                child_layout,
                                cursor,
                                renderer,
                                clipboard,
                                shell,
                                viewport,
                            );
                            break;
                        }
                    }
                }
            }

            Event::Touch(_) => {
                if !cursor_in_bounds {
                    return;
                }

                if let Some(pos) = cursor_pos {
                    let state = self.state.borrow();
                    let scroll_offset = state.scroll_offset;
                    drop(state);

                    let relative_y = pos.y - bounds.y + scroll_offset;
                    let target_item_idx = (relative_y / self.item_height).floor() as usize;

                    let children: Vec<_> = layout.children().collect();
                    for (slot_idx, &item_idx) in
                        internal_state.cached_item_indices.iter().enumerate()
                    {
                        if item_idx == target_item_idx
                            && slot_idx < internal_state.visible_trees.len()
                            && slot_idx < children.len()
                        {
                            let mut element = (self.item_builder)(item_idx);
                            let child_tree = &mut internal_state.visible_trees[slot_idx];
                            let child_layout = children[slot_idx];

                            element.as_widget_mut().update(
                                child_tree,
                                event,
                                child_layout,
                                cursor,
                                renderer,
                                clipboard,
                                shell,
                                viewport,
                            );
                            break;
                        }
                    }
                }
            }

            Event::Keyboard(_) => {
                let children: Vec<_> = layout.children().collect();
                for (slot_idx, &item_idx) in internal_state.cached_item_indices.iter().enumerate() {
                    if slot_idx < internal_state.visible_trees.len() && slot_idx < children.len() {
                        let mut element = (self.item_builder)(item_idx);
                        let child_tree = &mut internal_state.visible_trees[slot_idx];
                        let child_layout = children[slot_idx];

                        element.as_widget_mut().update(
                            child_tree,
                            event,
                            child_layout,
                            cursor,
                            renderer,
                            clipboard,
                            shell,
                            viewport,
                        );
                    }
                }
            }

            Event::Window(_) | Event::InputMethod(_) => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        let internal_state = tree.state.downcast_ref::<VirtualListInternalState>();

        if internal_state.scrollbar_dragging {
            return mouse::Interaction::Grabbing;
        }

        let cursor_pos = match cursor.position() {
            Some(pos) => pos,
            None => return mouse::Interaction::default(),
        };

        if !bounds.contains(cursor_pos) {
            return mouse::Interaction::default();
        }

        let state = self.state.borrow();
        if let Some(sb_bounds) = self.calculate_scrollbar_bounds(bounds, &state) {
            if sb_bounds.contains(cursor_pos) {
                return mouse::Interaction::Grab;
            }
        }
        let scroll_offset = state.scroll_offset;
        drop(state);

        let relative_y = cursor_pos.y - bounds.y + scroll_offset;
        let target_item_idx = (relative_y / self.item_height).floor() as usize;

        let children: Vec<_> = layout.children().collect();
        for (slot_idx, &item_idx) in internal_state.cached_item_indices.iter().enumerate() {
            if item_idx == target_item_idx
                && slot_idx < internal_state.visible_trees.len()
                && slot_idx < children.len()
            {
                let element = (self.item_builder)(item_idx);
                let child_tree = &internal_state.visible_trees[slot_idx];
                let child_layout = children[slot_idx];

                let interaction = element.as_widget().mouse_interaction(
                    child_tree,
                    child_layout,
                    cursor,
                    viewport,
                    renderer,
                );

                return interaction;
            }
        }

        mouse::Interaction::default()
    }
}

impl<'a, Message, Theme, Renderer> VirtualList<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn calculate_scrollbar_bounds(
        &self,
        bounds: Rectangle,
        state: &VirtualListState,
    ) -> Option<Rectangle> {
        if !self.show_scrollbar {
            return None;
        }

        let total_height = state.total_height();
        let max_scroll = state.max_scroll();

        if max_scroll <= 0.0 || total_height <= 0.0 {
            return None;
        }

        let view_ratio = bounds.height / total_height;
        let scrollbar_height = (bounds.height * view_ratio).max(SCROLLBAR_MIN_HEIGHT);

        let scroll_ratio = if max_scroll > 0.0 {
            state.scroll_offset / max_scroll
        } else {
            0.0
        };
        let available_track = bounds.height - scrollbar_height;
        let scrollbar_y = scroll_ratio * available_track;

        Some(Rectangle {
            x: bounds.x + bounds.width - SCROLLBAR_WIDTH - SCROLLBAR_MARGIN,
            y: bounds.y + scrollbar_y,
            width: SCROLLBAR_WIDTH,
            height: scrollbar_height,
        })
    }

    fn draw_scrollbar(
        &self,
        renderer: &mut Renderer,
        bounds: Rectangle,
        state: &VirtualListState,
        is_hovered: bool,
        is_dragging: bool,
    ) {
        if let Some(scrollbar_bounds) = self.calculate_scrollbar_bounds(bounds, state) {
            let alpha = if is_dragging {
                0.6
            } else if is_hovered {
                0.5
            } else {
                0.3
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds: scrollbar_bounds,
                    border: iced::Border {
                        radius: SCROLLBAR_BORDER_RADIUS.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                },
                Color::from_rgba(1.0, 1.0, 1.0, alpha),
            );
        }
    }
}

impl<'a, Message, Theme, Renderer> From<VirtualList<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(list: VirtualList<'a, Message, Theme, Renderer>) -> Self {
        Element::new(list)
    }
}
