//! Reusable UI widgets - composable components without business logic
//!
//! Widgets combine primitives and basic iced elements into reusable UI patterns.
//! They should not contain any business logic or depend on `crate::app` directly.
//!
//! # Design Principles
//!
//! - **No business logic**: Widgets must not import from `crate::app::Message`
//! - **Generic callbacks**: Use generic Message types or callback functions
//! - **Composable**: Build on primitives and iced's built-in widgets
//! - **Reusable**: Can be used by multiple components
//!
//! # Relationship to Other Layers
//!
//! - **Primitives** (`crate::ui::primitives`): Low-level Widget trait implementations
//! - **Widgets** (this module): Composable UI patterns
//! - **Components** (`crate::ui::components`): Business-specific UI with Message handling

pub mod multi_track_slider;
pub mod play_mode_button;
pub mod playback_controls;
pub mod playlist_card;
pub mod progress_slider;
pub mod section_header;
mod toast;
mod vertical_slider;

// Re-export from primitives for backward compatibility
pub use crate::ui::primitives::{
    ProgressRing, VirtualList, VirtualListState, square_cover, view_progress_ring_styled,
};

pub use play_mode_button::ButtonSize as PlayModeButtonSize;
pub use playback_controls::ControlSize;
pub use playlist_card::view as playlist_card;
pub use progress_slider::SliderSize;
pub use toast::{Toast, view_toast};
pub use vertical_slider::vertical_slider;
