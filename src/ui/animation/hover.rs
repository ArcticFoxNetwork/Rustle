//! Hover animation manager using iced_anim
//!
//! Provides optimized hover state management for UI elements.
//! Only tracks active + fading animations for O(1) complexity.

use std::hash::Hash;
use std::time::{Duration, Instant};

use iced_anim::Animated;
use iced_anim::transition::Easing;

/// Hover animation duration (200ms for snappy feel)
const HOVER_DURATION: Duration = Duration::from_millis(200);

/// Optimized hover animation manager for exclusive hover states
///
/// Only one item can be hovered at a time, so we only track:
/// - The currently active (hovered) item
/// - The previously active item (fading out)
///
/// This reduces memory usage and CPU overhead from O(n) to O(1).
#[derive(Debug)]
pub struct HoverAnimations<K: Eq + Hash + Clone> {
    /// Currently hovered item key
    active_key: Option<K>,
    /// Animation for active item (fading in)
    active_anim: Animated<f32>,
    /// Previously hovered item key (fading out)
    fading_key: Option<K>,
    /// Animation for fading item
    fading_anim: Animated<f32>,
}

impl<K: Eq + Hash + Clone> Default for HoverAnimations<K> {
    fn default() -> Self {
        Self::new()
    }
}

/// Create hover easing with custom duration
fn hover_easing() -> Easing {
    Easing::EASE_OUT.with_duration(HOVER_DURATION)
}

impl<K: Eq + Hash + Clone> HoverAnimations<K> {
    /// Create a new empty hover animation manager
    pub fn new() -> Self {
        Self {
            active_key: None,
            active_anim: Animated::transition(0.0, hover_easing()),
            fading_key: None,
            fading_anim: Animated::transition(0.0, hover_easing()),
        }
    }

    /// Set hovered item exclusively - only one key can be hovered at a time
    /// Pass None to unhover all
    ///
    /// Optimized: O(1) complexity
    pub fn set_hovered_exclusive(&mut self, key: Option<K>) {
        // Early return if state hasn't changed
        if self.active_key == key {
            return;
        }

        match (&self.active_key, key) {
            // Case A: Mouse entered a new item while another was hovered
            (Some(_old_key), Some(new_key)) => {
                // Move current active to fading state
                if let Some(old) = self.active_key.take() {
                    self.fading_key = Some(old);
                    // Start fading from current active value
                    let current_val = *self.active_anim.value();
                    self.fading_anim = Animated::transition(current_val, hover_easing());
                    self.fading_anim.update(0.0.into());
                }

                // Create new active animation
                self.active_key = Some(new_key);
                self.active_anim = Animated::transition(0.0, hover_easing());
                self.active_anim.update(1.0.into());
            }

            // Case B: Mouse entered a new item (nothing was hovered before)
            (None, Some(new_key)) => {
                self.active_key = Some(new_key);
                self.active_anim = Animated::transition(0.0, hover_easing());
                self.active_anim.update(1.0.into());
            }

            // Case C: Mouse left (unhover current)
            (Some(_), None) => {
                if let Some(old) = self.active_key.take() {
                    self.fading_key = Some(old);
                    let current_val = *self.active_anim.value();
                    self.fading_anim = Animated::transition(current_val, hover_easing());
                    self.fading_anim.update(0.0.into());
                }
            }

            // Case D: Nothing to do
            (None, None) => {}
        }
    }

    /// Get interpolated value for a key (0.0 to 1.0)
    pub fn get_progress(&self, key: &K) -> f32 {
        if self.active_key.as_ref() == Some(key) {
            *self.active_anim.value()
        } else if self.fading_key.as_ref() == Some(key) {
            *self.fading_anim.value()
        } else {
            0.0
        }
    }

    /// Get interpolated f32 value between from and to
    pub fn interpolate_f32(&self, key: &K, from: f32, to: f32) -> f32 {
        let progress = self.get_progress(key);
        from + (to - from) * progress
    }

    /// Check if any animation is currently in progress
    pub fn is_animating(&self) -> bool {
        self.active_anim.is_animating() || self.fading_anim.is_animating()
    }

    /// Clean up completed fade-out animations
    pub fn cleanup_completed(&mut self) {
        // Remove fading animation if it's done (reached 0)
        if let Some(_) = &self.fading_key {
            if *self.fading_anim.value() < 0.01
                && self.fading_anim.value() == self.fading_anim.target()
            {
                self.fading_key = None;
            }
        }
    }

    /// Clear all animation state
    pub fn clear(&mut self) {
        self.active_key = None;
        self.fading_key = None;
        self.active_anim = Animated::transition(0.0, hover_easing());
        self.fading_anim = Animated::transition(0.0, hover_easing());
    }

    /// Check if a specific key is currently the active (hovered) item
    #[allow(dead_code)]
    pub fn is_active(&self, key: &K) -> bool {
        self.active_key.as_ref() == Some(key)
    }

    /// Tick the animations forward in time
    /// Must be called on each animation frame to update values
    pub fn tick(&mut self, now: Instant) {
        self.active_anim.tick(now);
        self.fading_anim.tick(now);
    }
}

/// Single hover animation state (for dialogs, buttons, etc.)
#[derive(Debug)]
pub struct SingleHoverAnimation {
    animation: Animated<f32>,
}

/// Create single hover easing with custom duration
fn single_hover_easing() -> Easing {
    Easing::EASE.with_duration(HOVER_DURATION)
}

impl Default for SingleHoverAnimation {
    fn default() -> Self {
        Self::new()
    }
}

impl SingleHoverAnimation {
    /// Create a new single hover animation
    pub fn new() -> Self {
        Self {
            animation: Animated::transition(0.0, single_hover_easing()),
        }
    }

    /// Start the animation (go to active state)
    pub fn start(&mut self) {
        self.animation.update(1.0.into());
    }

    /// Stop the animation (go to inactive state)
    pub fn stop(&mut self) {
        self.animation.update(0.0.into());
    }

    /// Get progress (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        *self.animation.value()
    }

    /// Check if animation is in progress
    pub fn is_animating(&self) -> bool {
        self.animation.is_animating()
    }

    /// Tick the animation forward in time
    /// Must be called on each animation frame to update values
    pub fn tick(&mut self, now: Instant) {
        self.animation.tick(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hover_animations_exclusive() {
        let mut anims: HoverAnimations<i64> = HoverAnimations::new();

        // Initially no progress
        assert_eq!(anims.get_progress(&1), 0.0);

        // After hover enter
        anims.set_hovered_exclusive(Some(1));
        assert!(anims.is_active(&1));

        // Switch to another item
        anims.set_hovered_exclusive(Some(2));
        assert!(anims.is_active(&2));
        assert!(!anims.is_active(&1));
    }

    #[test]
    fn test_single_animation() {
        let mut anim = SingleHoverAnimation::new();

        // Initially at 0
        assert_eq!(anim.progress(), 0.0);

        // Start animation
        anim.start();
        // Target should be 1.0
        assert!(anim.is_animating() || anim.progress() > 0.0);
    }

    #[test]
    fn test_progress_range() {
        let mut anims: HoverAnimations<i64> = HoverAnimations::new();

        // Progress should always be in [0, 1]
        assert!(anims.get_progress(&1) >= 0.0);
        assert!(anims.get_progress(&1) <= 1.0);

        anims.set_hovered_exclusive(Some(1));
        assert!(anims.get_progress(&1) >= 0.0);
        assert!(anims.get_progress(&1) <= 1.0);
    }
}
