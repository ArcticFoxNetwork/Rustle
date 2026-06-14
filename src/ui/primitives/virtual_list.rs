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

use iced::advanced::Shell;
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{self, Tree, Widget};
use iced::mouse::{self, Cursor};
use iced::{Color, Element, Event, Length, Point, Rectangle, Size};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::rc::Rc;

/// Buffer items to render above and below the visible area
const BUFFER_ITEMS: usize = 8;

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
pub struct VirtualList<'a, Message, Theme, Renderer, Key = usize>
where
    Renderer: renderer::Renderer,
    Key: Eq + Hash + Clone + 'static,
{
    /// Total number of items
    item_count: usize,
    /// Item height
    item_height: f32,
    /// Function to build an item element by index
    item_builder: Box<dyn Fn(usize) -> Element<'a, Message, Theme, Renderer> + 'a>,
    /// Function to return a stable identity key for an item index
    item_key: Box<dyn Fn(usize) -> Key + 'a>,
    /// Shared state
    state: Rc<RefCell<VirtualListState>>,
    /// Width of the list
    width: Length,
    /// Height of the list
    height: Length,
    /// Whether to show scrollbar
    show_scrollbar: bool,
    /// Message to send when mouse moves over empty area (not over any item)
    on_empty_area: Option<Message>,
    /// Function to create hover message for an item index
    on_item_hover: Option<Box<dyn Fn(usize) -> Message + 'a>>,
}

impl<'a, Message, Theme, Renderer> VirtualList<'a, Message, Theme, Renderer, usize>
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
            item_key: Box::new(|index| index),
            state,
            width: Length::Fill,
            height: Length::Fill,
            show_scrollbar: true,
            on_empty_area: None,
            on_item_hover: None,
        }
    }
}

impl<'a, Message, Theme, Renderer, Key> VirtualList<'a, Message, Theme, Renderer, Key>
where
    Renderer: renderer::Renderer,
    Key: Eq + Hash + Clone + 'static,
{
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

    /// Set a message to send when mouse moves over empty area (not over any item)
    /// This is useful for clearing hover states when mouse leaves all items
    pub fn on_empty_area(mut self, message: Message) -> Self {
        self.on_empty_area = Some(message);
        self
    }

    /// Set a callback to create hover message for each item
    /// This is called on every mouse move to update hover state reliably
    /// even when mouse moves fast between items
    pub fn on_item_hover<F>(mut self, f: F) -> Self
    where
        F: Fn(usize) -> Message + 'a,
    {
        self.on_item_hover = Some(Box::new(f));
        self
    }

    /// Set a stable identity key for each item.
    ///
    /// Use a domain identifier (for example, a track id plus original index) when
    /// list order can change because of filtering, sorting, or pagination.
    pub fn keyed_by<NewKey, F>(self, f: F) -> VirtualList<'a, Message, Theme, Renderer, NewKey>
    where
        NewKey: Eq + Hash + Clone + 'static,
        F: Fn(usize) -> NewKey + 'a,
    {
        VirtualList {
            item_count: self.item_count,
            item_height: self.item_height,
            item_builder: self.item_builder,
            item_key: Box::new(f),
            state: self.state,
            width: self.width,
            height: self.height,
            show_scrollbar: self.show_scrollbar,
            on_empty_area: self.on_empty_area,
            on_item_hover: self.on_item_hover,
        }
    }
}

/// Internal state for widget tree
struct VirtualListInternalState<Key> {
    /// Trees keyed by item identity (persistent across scrolls and filtering)
    item_trees: HashMap<Key, Tree>,
    /// Cached visible range from layout phase
    cached_visible_range: (usize, usize),
    /// Whether the scrollbar is being dragged
    scrollbar_dragging: bool,
    /// The Y position where drag started (relative to scrollbar top)
    drag_start_offset: f32,
    /// Whether mouse is hovering over scrollbar
    scrollbar_hovered: bool,
    /// Last hovered item key for deduplication
    last_hovered_item: Option<Key>,
}

impl<Key> Default for VirtualListInternalState<Key> {
    fn default() -> Self {
        Self {
            item_trees: HashMap::new(),
            cached_visible_range: (0, 0),
            scrollbar_dragging: false,
            drag_start_offset: 0.0,
            scrollbar_hovered: false,
            last_hovered_item: None,
        }
    }
}

impl<'a, Message, Theme, Renderer, Key> Widget<Message, Theme, Renderer>
    for VirtualList<'a, Message, Theme, Renderer, Key>
where
    Message: Clone + 'a,
    Renderer: renderer::Renderer,
    Key: Eq + Hash + Clone + 'static,
{
    fn diff(&self, tree: &mut Tree) {
        let (start, end) = {
            let mut state = self.state.borrow_mut();
            state.item_count = self.item_count;
            state.item_height = self.item_height;
            state.visible_range()
        };

        let internal_state = tree.state.downcast_mut::<VirtualListInternalState<Key>>();

        internal_state.cached_visible_range = (start, end);

        // Prune trees for items far outside the visible range (keep a generous buffer)
        let prune_start = start.saturating_sub(BUFFER_ITEMS * 2);
        let prune_end = (end + BUFFER_ITEMS * 2).min(self.item_count);
        let retained_keys: HashSet<Key> = (prune_start..prune_end)
            .map(|idx| (self.item_key)(idx))
            .collect();
        internal_state
            .item_trees
            .retain(|key, _| retained_keys.contains(key));

        // Ensure trees exist for all visible items and diff them
        for item_idx in start..end {
            let element = (self.item_builder)(item_idx);
            let item_key = (self.item_key)(item_idx);
            let tree = internal_state
                .item_trees
                .entry(item_key)
                .or_insert_with(Tree::empty);
            tree.diff(&element);
        }
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<VirtualListInternalState<Key>>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(VirtualListInternalState::<Key>::default())
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
        let internal_state = tree.state.downcast_mut::<VirtualListInternalState<Key>>();

        // Cache the visible range
        internal_state.cached_visible_range = (start, end);

        // Build layout for visible items
        let mut children = Vec::with_capacity(visible_count);
        let item_limits = layout::Limits::new(Size::ZERO, Size::new(size.width, self.item_height));

        for item_idx in start..end {
            let mut element = (self.item_builder)(item_idx);
            let item_key = (self.item_key)(item_idx);

            // Get tree — normally diffed in diff(), but range may have changed
            // if viewport_height was updated between diff() and layout().
            let tree = internal_state
                .item_trees
                .entry(item_key)
                .or_insert_with(|| {
                    let mut t = Tree::empty();
                    t.diff(&element);
                    t
                });
            tree.diff(&element);

            let node = element.as_widget_mut().layout(tree, renderer, &item_limits);

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
        let internal_state = tree.state.downcast_ref::<VirtualListInternalState<Key>>();
        let (start, end) = internal_state.cached_visible_range;

        // Early return if no items to draw
        if start >= end {
            return;
        }

        // Find the subset of items actually on screen (within viewport bounds)
        let scroll_offset = self.state.borrow().scroll_offset;
        let mut on_screen_start = end;
        let mut on_screen_end = start;
        for item_idx in start..end {
            let y = item_idx as f32 * self.item_height - scroll_offset;
            let y_end = y + self.item_height;
            if y_end > 0.0 && y < bounds.height {
                if item_idx < on_screen_start {
                    on_screen_start = item_idx;
                }
                on_screen_end = item_idx + 1;
            }
        }

        if on_screen_start >= on_screen_end {
            return;
        }

        // Draw list items (clipped to bounds)
        renderer.with_layer(bounds, |renderer| {
            let mut children = layout.children();
            // Skip to the first on-screen item
            let skip = on_screen_start.saturating_sub(start);
            if skip > 0 && children.nth(skip - 1).is_none() {
                return;
            }

            for item_idx in on_screen_start..on_screen_end {
                let item_key = (self.item_key)(item_idx);
                let child_layout = match children.next() {
                    Some(l) => l,
                    None => break,
                };

                if let Some(child_tree) = internal_state.item_trees.get(&item_key) {
                    let element = (self.item_builder)(item_idx);
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
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let internal_state = tree.state.downcast_mut::<VirtualListInternalState<Key>>();

        let scrollbar_bounds = {
            let state = self.state.borrow();
            self.calculate_scrollbar_bounds(bounds, &state)
        };

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(position) = cursor.position()
                    && let Some(sb_bounds) = scrollbar_bounds
                    && sb_bounds.contains(position)
                {
                    internal_state.scrollbar_dragging = true;
                    internal_state.drag_start_offset = position.y - sb_bounds.y;
                    shell.capture_event();
                    return;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if internal_state.scrollbar_dragging =>
            {
                internal_state.scrollbar_dragging = false;
                shell.capture_event();
                return;
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

                // Handle hover state directly on CursorMoved for reliable tracking
                if bounds.contains(*position) {
                    if let Some(on_hover) = &self.on_item_hover {
                        let state = self.state.borrow();
                        let scroll_offset = state.scroll_offset;
                        let item_count = state.item_count;
                        drop(state);

                        let relative_y = position.y - bounds.y + scroll_offset;
                        let target_item_idx = (relative_y / self.item_height).floor() as usize;

                        if target_item_idx < item_count {
                            let target_item_key = (self.item_key)(target_item_idx);
                            if internal_state.last_hovered_item.as_ref() != Some(&target_item_key) {
                                internal_state.last_hovered_item = Some(target_item_key);
                                shell.publish((on_hover)(target_item_idx));
                            }
                        } else if internal_state.last_hovered_item.is_some() {
                            internal_state.last_hovered_item = None;
                            if let Some(msg) = &self.on_empty_area {
                                shell.publish(msg.clone());
                            }
                        }
                    }
                } else if internal_state.last_hovered_item.is_some() {
                    internal_state.last_hovered_item = None;
                    if let Some(msg) = &self.on_empty_area {
                        shell.publish(msg.clone());
                    }
                }
            }
            _ => {}
        }

        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event
            && let Some(position) = cursor.position()
            && bounds.contains(position)
        {
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
                    let item_count = state.item_count;
                    drop(state);

                    let relative_y = pos.y - bounds.y + scroll_offset;
                    let target_item_idx = (relative_y / self.item_height).floor() as usize;

                    // Check if mouse is over empty area (beyond the last item)
                    if target_item_idx >= item_count {
                        if internal_state.last_hovered_item.is_some() {
                            internal_state.last_hovered_item = None;
                            if let Some(msg) = &self.on_empty_area {
                                shell.publish(msg.clone());
                            }
                        }
                        return;
                    }

                    let (cached_start, _) = internal_state.cached_visible_range;
                    if target_item_idx >= cached_start {
                        let slot_idx = target_item_idx - cached_start;
                        let item_key = (self.item_key)(target_item_idx);
                        if let Some(child_tree) = internal_state.item_trees.get_mut(&item_key) {
                            let mut children = layout.children();
                            if let Some(child_layout) = children.nth(slot_idx) {
                                let mut element = (self.item_builder)(target_item_idx);
                                element.as_widget_mut().update(
                                    child_tree,
                                    event,
                                    child_layout,
                                    cursor,
                                    renderer,
                                    shell,
                                    viewport,
                                );
                            }
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

                    let (cached_start, _) = internal_state.cached_visible_range;
                    if target_item_idx >= cached_start {
                        let slot_idx = target_item_idx - cached_start;
                        let item_key = (self.item_key)(target_item_idx);
                        if let Some(child_tree) = internal_state.item_trees.get_mut(&item_key) {
                            let mut children = layout.children();
                            if let Some(child_layout) = children.nth(slot_idx) {
                                let mut element = (self.item_builder)(target_item_idx);
                                element.as_widget_mut().update(
                                    child_tree,
                                    event,
                                    child_layout,
                                    cursor,
                                    renderer,
                                    shell,
                                    viewport,
                                );
                            }
                        }
                    }
                }
            }

            Event::Keyboard(_) => {
                let (start, end) = internal_state.cached_visible_range;
                let mut children = layout.children();
                for item_idx in start..end {
                    let item_key = (self.item_key)(item_idx);
                    if let Some(child_layout) = children.next()
                        && let Some(child_tree) = internal_state.item_trees.get_mut(&item_key)
                    {
                        let mut element = (self.item_builder)(item_idx);
                        element.as_widget_mut().update(
                            child_tree,
                            event,
                            child_layout,
                            cursor,
                            renderer,
                            shell,
                            viewport,
                        );
                    }
                }
            }

            Event::Window(_) | Event::InputMethod(_) | Event::Clipboard(_) => {}
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
        let internal_state = tree.state.downcast_ref::<VirtualListInternalState<Key>>();

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
        if let Some(sb_bounds) = self.calculate_scrollbar_bounds(bounds, &state)
            && sb_bounds.contains(cursor_pos)
        {
            return mouse::Interaction::Grab;
        }
        let scroll_offset = state.scroll_offset;
        drop(state);

        let relative_y = cursor_pos.y - bounds.y + scroll_offset;
        let target_item_idx = (relative_y / self.item_height).floor() as usize;

        let (cached_start, _) = internal_state.cached_visible_range;
        if target_item_idx >= cached_start {
            let slot_idx = target_item_idx - cached_start;
            let item_key = (self.item_key)(target_item_idx);
            if let Some(child_tree) = internal_state.item_trees.get(&item_key) {
                let mut children = layout.children();
                if let Some(child_layout) = children.nth(slot_idx) {
                    let element = (self.item_builder)(target_item_idx);
                    return element.as_widget().mouse_interaction(
                        child_tree,
                        child_layout,
                        cursor,
                        viewport,
                        renderer,
                    );
                }
            }
        }

        mouse::Interaction::default()
    }
}

impl<'a, Message, Theme, Renderer, Key> VirtualList<'a, Message, Theme, Renderer, Key>
where
    Renderer: renderer::Renderer,
    Key: Eq + Hash + Clone + 'static,
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

impl<'a, Message, Theme, Renderer, Key> From<VirtualList<'a, Message, Theme, Renderer, Key>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
    Key: Eq + Hash + Clone + 'static,
{
    fn from(list: VirtualList<'a, Message, Theme, Renderer, Key>) -> Self {
        Element::new(list)
    }
}
