//! Animation prelude - commonly used types re-exported for convenience
//!
//! # Usage
//!
//! ```rust
//! use crate::ui::animation::prelude::*;
//! ```

// Re-export iced_anim types
pub use iced_anim::Animated;
pub use iced_anim::spring::Motion;
pub use iced_anim::transition::Easing;

/// Animation presets for common use cases
#[allow(dead_code)]
pub mod presets {
    use super::*;

    /// Quick hover animation (ease-out for snappy feel)
    pub fn hover_quick() -> Animated<f32> {
        Animated::transition(0.0, Easing::EASE_OUT)
    }

    /// Dialog fade animation (ease for smooth open/close)
    pub fn dialog_fade() -> Animated<f32> {
        Animated::transition(0.0, Easing::EASE)
    }

    /// Page transition animation (ease-in-out for smooth both ends)
    pub fn page_transition() -> Animated<f32> {
        Animated::transition(0.0, Easing::EASE_IN_OUT)
    }

    /// Bouncy spring for playful effects
    pub fn bouncy() -> Animated<f32> {
        Animated::spring(0.0, Motion::BOUNCY)
    }

    /// Smooth spring for natural movement
    pub fn smooth() -> Animated<f32> {
        Animated::spring(0.0, Motion::SMOOTH)
    }

    /// Snappy spring for responsive UI
    pub fn snappy() -> Animated<f32> {
        Animated::spring(0.0, Motion::SNAPPY)
    }
}
