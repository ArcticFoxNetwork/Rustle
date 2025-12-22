//! Animation utilities for hover transitions and other effects
//!
//! Provides a unified animation system using iced's official Animation API.
//!
//! Key optimization: HoverAnimations uses a single active state instead of
//! maintaining a HashMap for all items. This reduces complexity from O(n) to O(1)
//! since users can only hover one element at a time.

use std::hash::Hash;

use iced::animation::Animation;
use iced::time::Instant;

/// Type alias for hover animation (animates between false and true states)
pub type HoverAnimation = Animation<bool>;

/// Create a new hover animation with very quick duration (100ms)
pub fn new_hover_animation() -> HoverAnimation {
    Animation::new(false).very_quick()
}

/// Active animation state for a single hovered item
#[derive(Debug)]
struct ActiveAnimation<ID> {
    /// The ID of the currently hovered item
    id: ID,
    /// Animation for the hover-in effect
    animation: HoverAnimation,
}

/// Animation state for the previously hovered item (fading out)
#[derive(Debug)]
struct FadingAnimation<ID> {
    /// The ID of the item that was previously hovered
    id: ID,
    /// Animation for the fade-out effect
    animation: HoverAnimation,
}

/// Optimized animation manager for exclusive hover states
///
/// Only one item can be hovered at a time, so we only track:
/// - The currently active (hovered) item
/// - The previously active item (fading out)
///
/// This reduces memory usage and CPU overhead from O(n) to O(1).
#[derive(Debug)]
pub struct HoverAnimations<K: Eq + Hash> {
    /// Currently hovered item (if any)
    active: Option<ActiveAnimation<K>>,
    /// Previously hovered item that's fading out
    fading: Option<FadingAnimation<K>>,
}

impl<K: Eq + Hash + Clone> Default for HoverAnimations<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Eq + Hash + Clone> HoverAnimations<K> {
    /// Create a new empty animation manager
    pub fn new() -> Self {
        Self {
            active: None,
            fading: None,
        }
    }

    /// Set hovered state exclusively - only one key can be hovered at a time
    /// Pass None to unhover all
    ///
    /// Optimized: O(1) complexity, no HashMap iteration
    /// Early return if state hasn't changed to prevent animation reset
    pub fn set_hovered_exclusive(&mut self, key: Option<K>, now: Instant) {
        // Early return: check if state is already what we want
        let current_id = self.active.as_ref().map(|a| &a.id);
        if current_id == key.as_ref() {
            // Same state, nothing to do - this prevents animation reset
            return;
        }

        match (&self.active, key) {
            // Case A: Mouse entered a new item while another was hovered
            (Some(active), Some(new_id)) if active.id != new_id => {
                // Move current active to fading state
                let mut fading_anim = new_hover_animation();
                fading_anim.go_mut(true, now); // Start from "hovered" state
                fading_anim.go_mut(false, now); // Begin fade out

                self.fading = Some(FadingAnimation {
                    id: active.id.clone(),
                    animation: fading_anim,
                });

                // Create new active animation
                let mut new_anim = new_hover_animation();
                new_anim.go_mut(true, now);
                self.active = Some(ActiveAnimation {
                    id: new_id,
                    animation: new_anim,
                });
            }

            // Case B: Mouse entered a new item (nothing was hovered before)
            (None, Some(new_id)) => {
                let mut new_anim = new_hover_animation();
                new_anim.go_mut(true, now);
                self.active = Some(ActiveAnimation {
                    id: new_id,
                    animation: new_anim,
                });
            }

            // Case C: Mouse left the list (unhover current)
            (Some(_), None) => {
                if let Some(active) = self.active.take() {
                    // Move to fading state
                    let mut fading_anim = active.animation;
                    fading_anim.go_mut(false, now);
                    self.fading = Some(FadingAnimation {
                        id: active.id,
                        animation: fading_anim,
                    });
                }
            }

            // Case D: Same item still hovered or nothing to do
            // (This case is now handled by early return above)
            (Some(_), Some(_)) => {}
            (None, None) => {}
        }
    }

    /// Get interpolated value for a key (0.0 to 1.0)
    pub fn get_progress(&self, key: &K, now: Instant) -> f32 {
        // Check active animation first
        if let Some(active) = &self.active {
            if &active.id == key {
                return active.animation.interpolate(0.0_f32, 1.0_f32, now);
            }
        }

        // Check fading animation
        if let Some(fading) = &self.fading {
            if &fading.id == key {
                return fading.animation.interpolate(0.0_f32, 1.0_f32, now);
            }
        }

        0.0
    }

    /// Get interpolated f32 value
    pub fn interpolate_f32(&self, key: &K, from: f32, to: f32, now: Instant) -> f32 {
        let progress = self.get_progress(key, now);
        from + (to - from) * progress
    }

    /// Check if any animation is currently in progress
    /// O(1) complexity - only checks at most 2 animations
    pub fn is_animating(&self, now: Instant) -> bool {
        let active_animating = self
            .active
            .as_ref()
            .map(|a| a.animation.is_animating(now))
            .unwrap_or(false);

        let fading_animating = self
            .fading
            .as_ref()
            .map(|f| f.animation.is_animating(now))
            .unwrap_or(false);

        active_animating || fading_animating
    }

    /// Clean up completed fade-out animations
    /// Call this periodically to release memory
    pub fn cleanup_completed(&mut self, now: Instant) {
        // Remove fading animation if it's done
        if let Some(fading) = &self.fading {
            let progress = fading.animation.interpolate(0.0_f32, 1.0_f32, now);
            if progress < 0.01 && !fading.animation.is_animating(now) {
                self.fading = None;
            }
        }
    }

    /// Clear all animation state
    /// Call this when navigating away from a page
    pub fn clear(&mut self) {
        self.active = None;
        self.fading = None;
    }

    /// Check if a specific key is currently the active (hovered) item
    pub fn is_active(&self, key: &K) -> bool {
        self.active.as_ref().map(|a| &a.id == key).unwrap_or(false)
    }
}

/// Single hover animation state (for dialogs, buttons, etc.)
#[derive(Debug)]
pub struct SingleHoverAnimation {
    animation: HoverAnimation,
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
            animation: new_hover_animation(),
        }
    }

    /// Start the animation (go to active state)
    pub fn start(&mut self, now: Instant) {
        self.animation.go_mut(true, now);
    }

    /// Stop the animation (go to inactive state)
    pub fn stop(&mut self, now: Instant) {
        self.animation.go_mut(false, now);
    }

    /// Get progress (0.0 to 1.0)
    pub fn progress(&self, now: Instant) -> f32 {
        self.animation.interpolate(0.0_f32, 1.0_f32, now)
    }

    /// Check if animation is in progress
    pub fn is_animating(&self, now: Instant) -> bool {
        self.animation.is_animating(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hover_animations_exclusive() {
        let mut anims: HoverAnimations<i64> = HoverAnimations::new();
        let now = Instant::now();

        // Initially no progress
        assert_eq!(anims.get_progress(&1, now), 0.0);

        // After hover enter, should start animating
        anims.set_hovered_exclusive(Some(1), now);
        assert!(anims.is_animating(now));
        assert!(anims.is_active(&1));

        // Switch to another item
        anims.set_hovered_exclusive(Some(2), now);
        assert!(anims.is_active(&2));
        assert!(!anims.is_active(&1));
    }

    #[test]
    fn test_single_animation() {
        let mut anim = SingleHoverAnimation::new();
        let now = Instant::now();

        // Initially at 0
        assert_eq!(anim.progress(now), 0.0);

        // Start animation
        anim.start(now);
        assert!(anim.is_animating(now));
    }
}
