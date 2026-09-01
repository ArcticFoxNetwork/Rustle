//! Unified animation system for Rustle
//!
//! This module provides CSS-like animations using `iced_anim` and custom
//! hover animation management.
//!
mod hover;
mod scroll;

pub use hover::{HoverAnimations, SingleHoverAnimation};
pub use scroll::{SmoothScrollEvent, SmoothScrollState, SmoothScrollTarget};
