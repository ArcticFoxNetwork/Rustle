//! Physics simulation for lyrics scrolling
//!
//! Implements scrolling physics with:
//! - Momentum and inertia
//! - Rubber banding at boundaries
//! - Magnetic snapping to lines
//! - Smooth transitions between states

use super::spring::Spring;
use std::time::Instant;

/// Scroll state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollState {
    /// Auto-following playback
    AutoPlay,
    /// User is actively scrolling (mouse wheel or drag)
    #[allow(dead_code)]
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
    /// Manual scroll lower bound (top of document)
    min_scroll_y: f32,
    /// Manual scroll upper bound (bottom of document)
    max_scroll_y: f32,
    /// Spring for snapping with damper/stiffness
    snap_spring: Spring,

    /// Physics parameters
    _impulse_sensitivity: f32,
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
            min_scroll_y: 0.0,
            max_scroll_y: 0.0,
            snap_spring: Spring::with_params(0.0, 0.8, 10.0),
            _impulse_sensitivity: 5.0,
            friction: 0.995,
            snap_threshold: 50.0,
            max_overscroll: 200.0,
        }
    }

    /// 更新物理模拟
    ///
    /// @param dt 距上次更新的时间（秒）
    pub fn update(&mut self, dt: f32, _is_hovering: bool) {
        self.snap_spring.update(dt as f64);

        match self.state {
            ScrollState::UserInteraction => {
                self.scroll_y -= self.velocity * dt;
                self.apply_boundary_forces(dt);
                self.velocity *= self.friction.powf(dt * 60.0);

                if self.velocity.abs() < self.snap_threshold {
                    self.state = ScrollState::Inertia;
                }
            }
            ScrollState::Inertia => {
                self.velocity *= self.friction.powf(dt * 60.0);
                self.scroll_y -= self.velocity * dt;
                self.apply_boundary_forces(dt);

                if self.velocity.abs() < self.snap_threshold {
                    self.start_snapping();
                }
            }
            ScrollState::Snapping => {
                self.scroll_y = self.snap_spring.position() as f32;

                if self.snap_spring.arrived() {
                    self.state = ScrollState::Idle;
                    self.last_interaction_time = Instant::now();
                }
            }
            ScrollState::AutoPlay => {
                if self.snap_spring.target().abs() > 0.1 {
                    self.snap_spring.set_target(0.0);
                }
                self.scroll_y = self.snap_spring.position() as f32;
            }
            ScrollState::Idle => {
                self.velocity = 0.0;
            }
        }
    }

    /// Directly modify scroll position (for mouse wheel)
    pub fn scroll_by(&mut self, delta: f32) {
        self.state = ScrollState::Idle;
        self.last_interaction_time = Instant::now();
        self.velocity = 0.0;
        self.scroll_y += delta;
        self.clamp_position();
        self.snap_spring.set_position(self.scroll_y as f64);
        self.snap_spring.set_velocity(0.0);
        self.snap_spring.set_target(self.scroll_y as f64);
    }

    /// Apply impulse from touch/drag
    #[allow(dead_code)]
    pub fn apply_impulse(&mut self, impulse: f32) {
        self.state = ScrollState::UserInteraction;
        self.last_interaction_time = Instant::now();

        let target_velocity = self.velocity + impulse * self._impulse_sensitivity;
        self.velocity += (target_velocity - self.velocity) * 0.5;
    }

    /// Start snapping to nearest line
    fn start_snapping(&mut self) {
        self.state = ScrollState::Snapping;

        let nearest_line = (self.scroll_y / self.line_height).round() * self.line_height;
        let target =
            nearest_line.clamp(-(self.content_height - self.viewport_height).max(0.0), 0.0);

        self.snap_spring.set_position(self.scroll_y as f64);
        self.snap_spring.set_velocity(-self.velocity as f64);
        self.snap_spring.set_target(target as f64);
    }

    /// Apply rubber banding forces at boundaries
    fn apply_boundary_forces(&mut self, _dt: f32) {
        let _ = self.max_overscroll;
        // Boundaries are disabled because in the new per-line staggered layout system,
        // the physics engine only tracks the temporary manual scroll offset around 0.0.
    }

    /// Start auto-play mode
    pub fn start_auto_play(&mut self) {
        if self.state != ScrollState::AutoPlay {
            self.snap_spring.set_position(self.scroll_y as f64);
            self.snap_spring.set_velocity(self.velocity as f64);
            self.snap_spring.set_target(0.0);
        }
        self.state = ScrollState::AutoPlay;
    }

    /// Set manual scroll bounds.
    pub fn set_scroll_bounds(&mut self, min_scroll_y: f32, max_scroll_y: f32) {
        self.min_scroll_y = min_scroll_y;
        self.max_scroll_y = max_scroll_y;
    }

    /// Set viewport dimensions
    pub fn set_viewport_height(&mut self, height: f32) {
        self.viewport_height = height;
    }

    /// Get current scroll position
    pub fn position(&self) -> f32 {
        self.scroll_y
    }

    /// Get current state
    pub fn state(&self) -> ScrollState {
        self.state
    }

    /// 获取距上次交互的时间
    pub fn time_since_interaction(&self) -> f32 {
        self.last_interaction_time.elapsed().as_secs_f32()
    }

    /// Clamp current manual scroll position to the latest bounds.
    pub fn clamp_position(&mut self) -> f32 {
        let min_bound = self.min_scroll_y.min(self.max_scroll_y);
        let max_bound = self.min_scroll_y.max(self.max_scroll_y);
        self.scroll_y = self.scroll_y.clamp(min_bound, max_bound);
        self.scroll_y
    }

    /// Reset manual scrolling and return to auto-follow.
    pub fn reset_manual_scroll(&mut self) {
        self.scroll_y = 0.0;
        self.velocity = 0.0;
        self.last_interaction_time = Instant::now();
        self.snap_spring.set_position(0.0);
        self.snap_spring.set_velocity(0.0);
        self.snap_spring.set_target(0.0);
        self.state = ScrollState::AutoPlay;
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
