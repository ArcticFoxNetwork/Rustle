//! Physics simulation for lyrics scrolling
//!
//! Implements Apple Music-like scrolling physics with:
//! - Momentum and inertia
//! - Rubber banding at boundaries
//! - Magnetic snapping to lines
//! - Smooth transitions between states

#![allow(dead_code)]

use super::spring::Spring;
use std::time::Instant;

/// Scroll state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollState {
    /// Auto-following playback
    AutoPlay,
    /// User is actively scrolling (mouse wheel or drag)
    UserInteraction,
    /// Inertia after user stops scrolling
    Inertia,
    /// Snapping to nearest line
    Snapping,
    /// Idle, waiting to return to auto-play
    Idle,
}

/// Physics simulation for scrolling
#[derive(Debug)]
pub struct ScrollPhysics {
    /// Current scroll position in pixels
    scroll_y: f32,
    /// Current velocity in pixels per second
    velocity: f32,
    /// Content height in pixels
    content_height: f32,
    /// Viewport height in pixels
    viewport_height: f32,
    /// Line height for snapping calculations
    line_height: f32,

    /// State machine
    state: ScrollState,
    /// Last interaction time
    last_interaction_time: Instant,
    /// Auto-play target position
    auto_target: f32,
    /// Spring for snapping (Apple Music-style with damper/stiffness)
    snap_spring: Spring,

    /// Physics parameters
    impulse_sensitivity: f32,
    friction: f32,
    snap_threshold: f32,
    max_overscroll: f32,
}

impl ScrollPhysics {
    /// Create new scroll physics
    pub fn new(viewport_height: f32, line_height: f32) -> Self {
        Self {
            scroll_y: 0.0,
            velocity: 0.0,
            content_height: 0.0,
            viewport_height,
            line_height,
            state: ScrollState::AutoPlay,
            last_interaction_time: Instant::now(),
            auto_target: 0.0,
            snap_spring: Spring::with_params(0.0, 0.8, 10.0),
            impulse_sensitivity: 5.0,
            friction: 0.995,
            snap_threshold: 50.0,
            max_overscroll: 200.0,
        }
    }

    /// Update physics simulation
    ///
    /// @param dt Time since last update in SECONDS
    pub fn update(&mut self, dt: f32, _is_hovering: bool) {
        // Apple Music-style: Always update spring first
        self.snap_spring.update(dt as f64);

        match self.state {
            ScrollState::UserInteraction => {
                // Apply velocity with the exponential decay
                self.scroll_y -= self.velocity * dt;

                // Apply boundary forces for rubber banding
                self.apply_boundary_forces(dt);

                // the friction: v = v * friction^(dt * 60)
                self.velocity *= self.friction.powf(dt * 60.0);

                // Check if should transition to inertia
                if self.velocity.abs() < self.snap_threshold {
                    self.state = ScrollState::Inertia;
                }
            }
            ScrollState::Inertia => {
                // the exponential decay formula
                self.velocity *= self.friction.powf(dt * 60.0);
                self.scroll_y -= self.velocity * dt;

                // Apply boundary forces for rubber banding
                self.apply_boundary_forces(dt);

                // Check if should snap
                if self.velocity.abs() < self.snap_threshold {
                    self.start_snapping();
                }
            }
            ScrollState::Snapping => {
                // Use the spring animation
                self.scroll_y = self.snap_spring.position() as f32;

                if self.snap_spring.arrived() {
                    self.state = ScrollState::Idle;
                    self.last_interaction_time = Instant::now();
                }
            }

            ScrollState::AutoPlay => {
                // Use spring animation for smooth Apple Music-style scrolling
                // Only update target if it changed to avoid resetting spring time
                if (self.snap_spring.target() - self.auto_target as f64).abs() > 0.1 {
                    self.snap_spring.set_target(self.auto_target as f64);
                }
                self.scroll_y = self.snap_spring.position() as f32;
            }

            ScrollState::Idle => {
                // Stay in place, waiting for auto-play transition
                self.velocity = 0.0;
            }
        }
    }

    /// Apply impulse from mouse wheel or drag
    pub fn apply_impulse(&mut self, impulse: f32) {
        self.state = ScrollState::UserInteraction;
        self.last_interaction_time = Instant::now();

        // Apply impulse with sensitivity
        let target_velocity = self.velocity + impulse * self.impulse_sensitivity;

        // Smooth velocity changes for weighty feel
        self.velocity = lerp(self.velocity, target_velocity, 0.5);
    }

    /// Start snapping to nearest line
    fn start_snapping(&mut self) {
        self.state = ScrollState::Snapping;

        // Find nearest line
        let nearest_line = (self.scroll_y / self.line_height).round() * self.line_height;
        let target =
            nearest_line.clamp(-(self.content_height - self.viewport_height).max(0.0), 0.0);

        // Configure spring for snapping with the approach
        self.snap_spring.set_position(self.scroll_y as f64);
        self.snap_spring.set_velocity(-self.velocity as f64); // Inherit velocity for seamless transition
        self.snap_spring.set_target(target as f64);
    }

    /// Apply rubber banding forces at boundaries
    fn apply_boundary_forces(&mut self, dt: f32) {
        let min_y = -(self.content_height - self.viewport_height).min(0.0);
        let max_y = 0.0;

        if self.scroll_y < min_y {
            // Over-scrolling at top
            let overscroll = (min_y - self.scroll_y).abs();
            let friction = 1.0 / (1.0 + (overscroll / self.max_overscroll).powf(2.0));
            self.velocity *= friction;

            // Apply spring force
            let spring_force = (min_y - self.scroll_y) * 5.0;
            self.velocity += spring_force * dt;
            self.velocity *= 0.8;
        } else if self.scroll_y > max_y {
            // Over-scrolling at bottom
            let overscroll = (self.scroll_y - max_y).abs();
            let friction = 1.0 / (1.0 + (overscroll / self.max_overscroll).powf(2.0));
            self.velocity *= friction;

            // Apply spring force
            let spring_force = (max_y - self.scroll_y) * 5.0;
            self.velocity += spring_force * dt;
            self.velocity *= 0.8;
        }
    }

    /// Start auto-play mode
    pub fn start_auto_play(&mut self) {
        // Only reset spring if not already in auto-play mode
        // This prevents jerky motion when target changes frequently
        if self.state != ScrollState::AutoPlay {
            self.snap_spring.set_position(self.scroll_y as f64);
            self.snap_spring.set_velocity(self.velocity as f64);
            self.snap_spring.set_target(self.auto_target as f64);
        }
        self.state = ScrollState::AutoPlay;
    }

    /// Set auto-play target position
    pub fn set_auto_target(&mut self, target: f32) {
        // Only update if target actually changed to avoid resetting spring time
        if (self.auto_target - target).abs() > 0.1 {
            self.auto_target = target;
            if self.state == ScrollState::AutoPlay {
                self.snap_spring.set_target(target as f64);
            }
        }
    }

    /// Set content dimensions
    pub fn set_content_height(&mut self, height: f32) {
        self.content_height = height;
    }

    /// Set viewport dimensions
    pub fn set_viewport_height(&mut self, height: f32) {
        self.viewport_height = height;
    }

    /// Get current scroll position
    pub fn position(&self) -> f32 {
        self.scroll_y
    }

    /// Get current velocity
    pub fn velocity(&self) -> f32 {
        self.velocity
    }

    /// Get current state
    pub fn state(&self) -> ScrollState {
        self.state
    }

    /// Get time since last interaction
    pub fn time_since_interaction(&self) -> f32 {
        self.last_interaction_time.elapsed().as_secs_f32()
    }

    /// Force set position (for initialization)
    pub fn set_position(&mut self, pos: f32) {
        self.scroll_y = pos;
        self.snap_spring.set_position(pos as f64);
    }

    /// Set friction coefficient
    pub fn set_friction(&mut self, friction: f32) {
        self.friction = friction;
    }

    /// Set snap threshold
    pub fn set_snap_threshold(&mut self, threshold: f32) {
        self.snap_threshold = threshold;
    }

    /// Set maximum overscroll distance
    pub fn set_max_overscroll(&mut self, max: f32) {
        self.max_overscroll = max;
    }
}

impl Default for ScrollPhysics {
    fn default() -> Self {
        Self::new(800.0, 48.0)
    }
}

/// Linear interpolation
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
