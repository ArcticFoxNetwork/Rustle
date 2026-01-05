//! Unified animation system for Rustle
//!
//! This module provides CSS-like animations using `iced_anim` and custom
//! hover animation management.
//!
//! # Usage
//!
//! ```rust
//! use crate::ui::animation::prelude::*;
//!
//! // CSS-like transition
//! let opacity: Animated<f32> = Animated::transition(0.0, Easing::EASE);
//!
//! // Spring animation
//! let scale: Animated<f32> = Animated::spring(1.0, Motion::BOUNCY);
//! ```

mod hover;
pub mod prelude;

pub use hover::{HoverAnimations, SingleHoverAnimation};
