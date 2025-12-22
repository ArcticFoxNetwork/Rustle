//! Per-line animation system for lyrics
//!
//! Each lyric line has its own Spring for Y position and scale,
//! enabling smooth, independent animations with staggered delays.
//!
//! ## Spring Parameters
//!
//! Different spring parameters are used for different properties:
//!
//! ### Position Y
//! - mass: 0.9, damping: 15, stiffness: 90
//! - Provides responsive but smooth vertical movement
//!
//! ### Scale (normal lines)
//! - mass: 2, damping: 25, stiffness: 100
//! - Heavier mass for more deliberate scale changes
//!
//! ### Scale (background lines)
//! - mass: 1, damping: 20, stiffness: 50
//! - Slower, more subtle animation for background vocals
//!
//! ## Performance Optimization
//!
//! `AnimationBuffers` provides pre-allocated buffers for animation state,
//! avoiding per-frame Vec allocations in the render path.

use super::spring::{Spring, SpringParams};

// ============================================================================
// AnimationBuffers - Pre-allocated buffers for animation state
// ============================================================================

/// Pre-allocated buffers for animation state
///
/// This struct avoids per-frame Vec allocations by maintaining
/// pre-sized buffers that are updated in-place.
///
/// ## Usage
///
/// ```ignore
/// let mut buffers = AnimationBuffers::default();
/// buffers.ensure_capacity(line_count);
/// buffers.update_from_manager(&line_animation_manager);
/// // Now use buffers.positions(), buffers.scales(), etc.
/// ```
#[derive(Debug, Clone)]
pub struct AnimationBuffers {
    /// Y positions in logical pixels
    positions: Vec<f32>,
    /// Scale values (0.0 - 1.0)
    scales: Vec<f32>,
    /// Blur levels (Apple Music-style distance-based)
    blur_levels: Vec<f32>,
    /// Opacity values (0.0 - 1.0)
    opacities: Vec<f32>,
}

impl Default for AnimationBuffers {
    fn default() -> Self {
        Self {
            positions: Vec::new(),
            scales: Vec::new(),
            blur_levels: Vec::new(),
            opacities: Vec::new(),
        }
    }
}

impl AnimationBuffers {
    /// Create new empty buffers
    pub fn new() -> Self {
        Self::default()
    }

    /// Create buffers with pre-allocated capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            positions: Vec::with_capacity(capacity),
            scales: Vec::with_capacity(capacity),
            blur_levels: Vec::with_capacity(capacity),
            opacities: Vec::with_capacity(capacity),
        }
    }

    /// Ensure buffers have enough capacity for the given line count
    ///
    /// This resizes buffers only when necessary, avoiding allocations
    /// when the line count hasn't changed.
    pub fn ensure_capacity(&mut self, line_count: usize) {
        // Only resize if needed
        if self.positions.len() != line_count {
            self.positions.resize(line_count, 0.0);
            self.scales.resize(line_count, 1.0);
            self.blur_levels.resize(line_count, 0.0);
            self.opacities.resize(line_count, 1.0);
        }
    }

    /// Update buffers from LineAnimationManager (in-place, no allocations)
    ///
    /// This copies the current animation state from the manager into
    /// the pre-allocated buffers.
    pub fn update_from_manager(&mut self, manager: &LineAnimationManager) {
        let animations = manager.animations_slice();
        let len = animations.len();

        // Ensure capacity
        self.ensure_capacity(len);

        // Update in-place
        for (i, anim) in animations.iter().enumerate() {
            self.positions[i] = anim.current_y();
            self.scales[i] = anim.current_scale();
            self.blur_levels[i] = anim.blur;
            self.opacities[i] = anim.opacity;
        }
    }

    /// Get positions slice (no allocation)
    #[inline]
    pub fn positions(&self) -> &[f32] {
        &self.positions
    }

    /// Get scales slice (no allocation)
    #[inline]
    pub fn scales(&self) -> &[f32] {
        &self.scales
    }

    /// Get blur levels slice (no allocation)
    #[inline]
    pub fn blur_levels(&self) -> &[f32] {
        &self.blur_levels
    }

    /// Get opacities slice (no allocation)
    #[inline]
    pub fn opacities(&self) -> &[f32] {
        &self.opacities
    }

    /// Get current capacity (number of lines)
    #[inline]
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    /// Check if buffers are empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Clear all buffers
    pub fn clear(&mut self) {
        self.positions.clear();
        self.scales.clear();
        self.blur_levels.clear();
        self.opacities.clear();
    }
}

// ============================================================================
// LineAnimation - Animation state for a single line
// ============================================================================

/// Animation state for a single lyric line
#[derive(Debug, Clone)]
pub struct LineAnimation {
    /// Y position spring (in logical pixels)
    pub pos_y: Spring,
    /// Scale spring (0-100, where 100 = 1.0)
    pub scale: Spring,
    /// Target opacity (0.0 - 1.0) - will be smoothly interpolated
    pub target_opacity: f32,
    /// Current animated opacity (0.0 - 1.0)
    pub opacity: f32,
    /// Target blur level - will be smoothly interpolated
    pub target_blur: f32,
    /// Current animated blur level (Apple Music-style distance-based blur)
    pub blur: f32,
    /// Whether this line is currently active
    pub is_active: bool,
    /// Whether this is a background line
    pub is_bg: bool,
    /// Current delay for staggered animation (seconds)
    pub delay: f32,
}

impl LineAnimation {
    /// Create a new line animation with initial Y position
    pub fn new(initial_y: f32, is_bg: bool) -> Self {
        // Use the spring parameters via SpringParams
        let mut pos_y = Spring::from_params(initial_y as f64, SpringParams::POS_Y);
        pos_y.set_target(initial_y as f64);

        // Scale spring: different parameters for normal vs background lines
        let scale_params = if is_bg {
            SpringParams::SCALE_BG
        } else {
            SpringParams::SCALE
        };
        let mut scale = Spring::from_params(100.0, scale_params);
        scale.set_target(100.0);

        Self {
            pos_y,
            scale,
            target_opacity: 1.0,
            opacity: 1.0,
            target_blur: 0.0,
            blur: 0.0,
            is_active: false,
            is_bg,
            delay: 0.0,
        }
    }

    /// Set target Y position with optional delay (Apple Music-style staggered animation)
    /// delay is in seconds
    pub fn set_target_y(&mut self, target: f32, delay: f32) {
        self.delay = delay;
        if delay > 0.0 {
            self.pos_y
                .set_target_with_delay(target as f64, delay as f64);
        } else {
            self.pos_y.set_target(target as f64);
        }
    }

    /// Set target scale (0-100) with optional delay
    pub fn set_target_scale(&mut self, target: f32) {
        // Scale uses the same delay as position for coordinated animation
        if self.delay > 0.0 {
            self.scale
                .set_target_with_delay(target as f64, self.delay as f64);
        } else {
            self.scale.set_target(target as f64);
        }
    }

    /// Force set Y position (no animation)
    pub fn set_position_y(&mut self, pos: f32) {
        self.pos_y.set_position(pos as f64);
        self.pos_y.set_target(pos as f64);
    }

    /// Force set scale (no animation)
    pub fn set_scale(&mut self, scale: f32) {
        self.scale.set_position(scale as f64);
        self.scale.set_target(scale as f64);
    }

    /// Get current animated Y position
    pub fn current_y(&self) -> f32 {
        self.pos_y.position() as f32
    }

    /// Get current animated scale (0.0 - 1.0)
    pub fn current_scale(&self) -> f32 {
        self.scale.position() as f32 / 100.0
    }

    /// Check if animation has arrived at target
    pub fn arrived(&self) -> bool {
        self.pos_y.arrived() && self.scale.arrived()
    }

    /// Update spring parameters (for runtime configuration)
    pub fn update_pos_y_params(&mut self, params: SpringParams) {
        self.pos_y.update_params(params);
    }

    /// Update scale spring parameters
    pub fn update_scale_params(&mut self, params: SpringParams) {
        self.scale.update_params(params);
    }

    /// Update springs and smooth transitions - MUST be called every frame (Apple Music-style)
    ///
    /// @param delta Time since last update in SECONDS
    pub fn update(&mut self, delta: f32) {
        self.pos_y.update(delta as f64);
        self.scale.update(delta as f64);

        // Smooth interpolation for blur and opacity
        // Use exponential decay for smooth CSS-like transitions
        // CSS transitions are typically 0.3-0.5s
        // Speed factor: lower = slower transition
        // 3.0 means ~0.33s to reach 63% of target (similar to CSS 0.3s ease-out)
        const BLUR_TRANSITION_SPEED: f32 = 3.0;
        const OPACITY_TRANSITION_SPEED: f32 = 5.0;

        let blur_lerp = 1.0 - (-BLUR_TRANSITION_SPEED * delta).exp();
        let opacity_lerp = 1.0 - (-OPACITY_TRANSITION_SPEED * delta).exp();

        // Smooth blur transition
        self.blur += (self.target_blur - self.blur) * blur_lerp;

        // Smooth opacity transition
        self.opacity += (self.target_opacity - self.opacity) * opacity_lerp;
    }
}

/// Align anchor for lyric lines (the alignAnchor)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignAnchor {
    /// Align to top of the line (no adjustment)
    Top,
    /// Align to center of the line (default)
    #[default]
    Center,
    /// Align to bottom of the line
    Bottom,
}

/// Manager for all line animations
///
/// Implements Apple Music-style layout calculation with:
/// - Per-line spring animations for Y position and scale
/// - Staggered delays for "waterfall" effect
/// - Distance-based blur calculation
/// - Different spring parameters for normal vs background lines
#[derive(Debug, Default)]
pub struct LineAnimationManager {
    /// Animations for each line
    animations: Vec<LineAnimation>,
    /// Viewport height for calculations
    viewport_height: f32,
    /// Align position (0.0 - 1.0, default 0.35)
    align_position: f32,
    /// Align anchor: top/center/bottom (default: center)
    align_anchor: AlignAnchor,
    /// Whether to hide passed lines (the hidePassedLines)
    hide_passed_lines: bool,
    /// Custom position Y spring parameters (optional override)
    pos_y_params: Option<SpringParams>,
    /// Custom scale spring parameters (optional override)
    scale_params: Option<SpringParams>,
    /// Custom scale spring parameters for BG lines (optional override)
    scale_bg_params: Option<SpringParams>,
}

impl LineAnimationManager {
    /// Create a new animation manager
    pub fn new() -> Self {
        Self {
            animations: Vec::new(),
            viewport_height: 800.0,
            align_position: 0.35,
            align_anchor: AlignAnchor::Center,
            hide_passed_lines: false,
            pos_y_params: None,
            scale_params: None,
            scale_bg_params: None,
        }
    }

    /// Set viewport height
    pub fn set_viewport_height(&mut self, height: f32) {
        self.viewport_height = height;
    }

    /// Set align position (0.0 - 1.0, default: 0.35)
    pub fn set_align_position(&mut self, pos: f32) {
        self.align_position = pos;
    }

    /// Set align anchor (the setAlignAnchor)
    ///
    /// - Top: align to top of the target line
    /// - Center: align to center of the target line (default)
    /// - Bottom: align to bottom of the target line
    pub fn set_align_anchor(&mut self, anchor: AlignAnchor) {
        self.align_anchor = anchor;
    }

    /// Set whether to hide passed lines (the hidePassedLines)
    pub fn set_hide_passed_lines(&mut self, hide: bool) {
        self.hide_passed_lines = hide;
    }

    /// Set custom position Y spring parameters
    pub fn set_pos_y_spring_params(&mut self, params: SpringParams) {
        self.pos_y_params = Some(params);
        for anim in &mut self.animations {
            anim.update_pos_y_params(params);
        }
    }

    /// Set custom scale spring parameters
    pub fn set_scale_spring_params(&mut self, params: SpringParams) {
        self.scale_params = Some(params);
        for anim in &mut self.animations {
            if !anim.is_bg {
                anim.update_scale_params(params);
            }
        }
    }

    /// Set custom scale spring parameters for background lines
    pub fn set_scale_bg_spring_params(&mut self, params: SpringParams) {
        self.scale_bg_params = Some(params);
        for anim in &mut self.animations {
            if anim.is_bg {
                anim.update_scale_params(params);
            }
        }
    }

    /// Ensure we have enough animations for the given line count
    /// Returns true if animations were reset (new song)
    pub fn ensure_capacity(&mut self, line_count: usize, is_bg_flags: &[bool]) -> bool {
        if self.animations.len() != line_count {
            // Reset all animations for new song
            self.animations.clear();
            let initial_y = self.viewport_height * 2.0; // Start off-screen like

            for i in 0..line_count {
                let is_bg = is_bg_flags.get(i).copied().unwrap_or(false);
                let mut anim = LineAnimation::new(initial_y, is_bg);

                // Apply custom spring parameters if set
                if let Some(params) = self.pos_y_params {
                    anim.update_pos_y_params(params);
                }
                if is_bg {
                    if let Some(params) = self.scale_bg_params {
                        anim.update_scale_params(params);
                    }
                } else if let Some(params) = self.scale_params {
                    anim.update_scale_params(params);
                }

                self.animations.push(anim);
            }
            return true;
        }
        false
    }

    /// Get animation for a specific line
    pub fn get(&self, index: usize) -> Option<&LineAnimation> {
        self.animations.get(index)
    }

    /// Get mutable animation for a specific line
    pub fn get_mut(&mut self, index: usize) -> Option<&mut LineAnimation> {
        self.animations.get_mut(index)
    }

    /// Get current Y positions for all lines (for rendering)
    pub fn current_positions(&self) -> Vec<f32> {
        self.animations.iter().map(|a| a.current_y()).collect()
    }

    /// Get current scales for all lines (for rendering)
    pub fn current_scales(&self) -> Vec<f32> {
        self.animations.iter().map(|a| a.current_scale()).collect()
    }

    /// Update all springs - MUST be called every frame (Apple Music-style)
    ///
    /// @param delta Time since last update in SECONDS
    pub fn update(&mut self, delta: f32) {
        for anim in &mut self.animations {
            anim.update(delta);
        }
    }

    /// Get current blur levels for all lines (Apple Music-style distance-based blur)
    pub fn current_blur_levels(&self) -> Vec<f32> {
        self.animations.iter().map(|a| a.blur).collect()
    }

    /// Get current opacities for all lines
    pub fn current_opacities(&self) -> Vec<f32> {
        self.animations.iter().map(|a| a.opacity).collect()
    }

    /// Calculate and set target positions for all lines (the calcLayout)
    ///
    /// This implements the layout algorithm with:
    /// - Staggered delays for "waterfall" animation effect
    /// - Distance-based blur calculation
    /// - Different handling for background vs normal lines
    ///
    /// Parameters:
    /// - line_heights: Height of each line in logical pixels
    /// - line_spacing: Spacing between lines
    /// - scroll_to_index: Current target line index
    /// - buffered_lines: Set of active line indices
    /// - is_playing: Whether playback is active
    /// - is_seek: Whether this is a seek operation (force immediate position)
    /// - enable_scale: Whether to apply scale effect
    /// - inactive_scale: Scale factor for inactive lines (default: 0.97)
    /// - bg_line_scale: Scale factor for background lines (default: 0.75)
    #[allow(clippy::too_many_arguments)]
    pub fn calc_layout(
        &mut self,
        line_heights: &[f32],
        line_spacing: f32,
        scroll_to_index: usize,
        buffered_lines: &std::collections::HashSet<usize>,
        is_playing: bool,
        is_seek: bool,
        enable_scale: bool,
        inactive_scale: f32,
        bg_line_scale: f32,
    ) {
        self.calc_layout_with_stagger(
            line_heights,
            line_spacing,
            scroll_to_index,
            buffered_lines,
            is_playing,
            is_seek,
            enable_scale,
            inactive_scale,
            bg_line_scale,
            0.05, // default base delay
            1.05, // default reduction factor
        )
    }

    /// Calculate layout with custom stagger parameters
    ///
    /// This is the full implementation with configurable stagger animation.
    ///
    /// Apple Music-style features:
    /// - isNonDynamic: Different opacity for non-dynamic lyrics (all lines have only 1 word)
    /// - Small screen blur: blur * 0.8 when viewport_width <= 1024
    #[allow(clippy::too_many_arguments)]
    pub fn calc_layout_with_stagger(
        &mut self,
        line_heights: &[f32],
        line_spacing: f32,
        scroll_to_index: usize,
        buffered_lines: &std::collections::HashSet<usize>,
        is_playing: bool,
        is_seek: bool,
        enable_scale: bool,
        inactive_scale: f32,
        bg_line_scale: f32,
        stagger_base_delay: f32,
        stagger_reduction_factor: f32,
    ) {
        self.calc_layout_full(
            line_heights,
            line_spacing,
            scroll_to_index,
            buffered_lines,
            is_playing,
            is_seek,
            enable_scale,
            inactive_scale,
            bg_line_scale,
            stagger_base_delay,
            stagger_reduction_factor,
            false,  // is_non_dynamic
            2000.0, // viewport_width (default large)
        )
    }

    /// Full layout calculation with all parameters
    ///
    /// This is a 1:1 port of the `calcLayout` from `lyric-player/base.ts`.
    ///
    /// Additional parameters:
    /// - is_non_dynamic: True if all lines have only 1 word (affects opacity)
    /// - viewport_width: Used for small screen blur adjustment (blur * 0.8 when <= 1024)
    #[allow(clippy::too_many_arguments)]
    pub fn calc_layout_full(
        &mut self,
        line_heights: &[f32],
        line_spacing: f32,
        scroll_to_index: usize,
        buffered_lines: &std::collections::HashSet<usize>,
        is_playing: bool,
        is_seek: bool,
        enable_scale: bool,
        inactive_scale: f32,
        bg_line_scale: f32,
        stagger_base_delay: f32,
        stagger_reduction_factor: f32,
        is_non_dynamic: bool,
        viewport_width: f32,
    ) {
        if self.animations.is_empty() {
            return;
        }

        // default: LINE_HEIGHT_FALLBACK = size[1] / 5
        let line_height_fallback = self.viewport_height / 5.0;

        // default: Small screen blur adjustment
        let blur_multiplier = if viewport_width <= 1024.0 { 0.8 } else { 1.0 };

        // default: targetAlignIndex (may differ from scrollToIndex during interlude)
        // For now, we use scroll_to_index directly. Interlude handling can be added later.
        let target_align_index = scroll_to_index;

        // default: Calculate scroll offset (sum of heights before target line, excluding BG lines when playing)
        // scrollOffset = currentLyricLineObjects.slice(0, targetAlignIndex).reduce(...)
        let mut scroll_offset = 0.0f32;
        for idx in 0..target_align_index.min(self.animations.len()) {
            let is_bg = self.animations.get(idx).map(|a| a.is_bg).unwrap_or(false);
            if is_bg && is_playing {
                continue; // default: Skip BG lines in scroll calculation when playing
            }
            let height = line_heights
                .get(idx)
                .copied()
                .unwrap_or(line_height_fallback);
            scroll_offset += height + line_spacing;
        }

        // default: curPos = -scrollOffset + size[1] * alignPosition
        // Note: Also has -this.scrollOffset at the start, but that's for manual scroll offset
        let mut cur_pos = -scroll_offset + self.viewport_height * self.align_position;

        // default: Apply alignAnchor adjustment to curPos
        if let Some(cur_line_height) = line_heights.get(target_align_index) {
            match self.align_anchor {
                AlignAnchor::Bottom => cur_pos -= *cur_line_height,
                AlignAnchor::Center => cur_pos -= *cur_line_height / 2.0,
                AlignAnchor::Top => {} // No adjustment
            }
        }

        // default: latestIndex = Math.max(...bufferedLines)
        // 注意：当 buffered_lines 为空时，使用 scroll_to_index 作为 latest_index
        // 但这会导致 is_active 条件 (idx >= scroll_to_index && idx < latest_index) 永远为 false
        // 因此我们需要确保至少 scroll_to_index 对应的行是 active 的
        let latest_index = buffered_lines
            .iter()
            .max()
            .copied()
            .unwrap_or(scroll_to_index);

        // default: delay and baseDelay
        let mut delay = 0.0f32;
        let mut base_delay = if is_seek { 0.0 } else { stagger_base_delay };

        for (idx, anim) in self.animations.iter_mut().enumerate() {
            let height = line_heights
                .get(idx)
                .copied()
                .unwrap_or(line_height_fallback);
            let has_buffered = buffered_lines.contains(&idx);

            // default: isActive = hasBuffered || (i >= scrollToIndex && i < latestIndex)
            // 修复：当 buffered_lines 为空时，scroll_to_index 对应的行应该是 active
            // 这发生在歌词间隙期间（interlude），此时没有行正在播放，但我们仍然
            // 希望 scroll_to_index 指向的行（即将播放的行）不被模糊
            let is_active = has_buffered
                || (idx >= scroll_to_index && idx < latest_index)
                || (buffered_lines.is_empty() && idx == scroll_to_index);

            // Update active state
            anim.is_active = is_active;

            // default: Calculate target scale
            // SCALE_ASPECT = enableScale ? 97 : 100
            // targetScale = 100 if active or !playing, else (isBG ? 75 : SCALE_ASPECT)
            let scale_aspect = if enable_scale {
                inactive_scale * 100.0
            } else {
                100.0
            };
            let target_scale = if !is_active && is_playing {
                if anim.is_bg {
                    bg_line_scale * 100.0
                } else {
                    scale_aspect
                }
            } else {
                100.0
            };

            // default: Calculate blur level (distance-based)
            let blur_level = if is_active {
                0.0
            } else {
                let mut level = 1.0;
                if idx < scroll_to_index {
                    // Lines above current: blur increases with distance
                    level += (scroll_to_index - idx) as f32 + 1.0;
                } else {
                    // Lines below current: blur increases with distance from latest active
                    level += (idx as i32 - latest_index.max(scroll_to_index) as i32).abs() as f32;
                }
                level * blur_multiplier
            };
            // Set target blur (will be smoothly interpolated in update())
            anim.target_blur = blur_level;

            // default: Calculate opacity
            // hidePassedLines logic + normal opacity logic
            let target_opacity = if self.hide_passed_lines {
                if idx < scroll_to_index && is_playing {
                    // default: 为了避免浏览器优化，使用极小但不为零的值
                    0.00001
                } else if anim.is_bg {
                    if is_active {
                        0.4
                    } else if !is_playing {
                        0.4
                    } else {
                        0.0001
                    }
                } else if has_buffered {
                    0.85
                } else if is_non_dynamic {
                    0.2
                } else {
                    1.0
                }
            } else {
                // No hidePassedLines
                if anim.is_bg {
                    if is_active {
                        0.4
                    } else if !is_playing {
                        0.4
                    } else {
                        0.0001
                    }
                } else if has_buffered {
                    0.85
                } else if is_non_dynamic {
                    0.2
                } else {
                    1.0
                }
            };
            // Set target opacity (will be smoothly interpolated in update())
            anim.target_opacity = target_opacity;

            // Set targets
            if is_seek {
                // Force immediate position on seek
                anim.set_position_y(cur_pos);
                anim.set_scale(target_scale);
                // Also force immediate blur and opacity on seek
                anim.blur = anim.target_blur;
                anim.opacity = anim.target_opacity;
            } else {
                anim.set_target_y(cur_pos, delay);
                anim.set_target_scale(target_scale);
            }

            // default: Advance position for next line
            // BG lines only take space if active or not playing
            let takes_space = if anim.is_bg {
                is_active || !is_playing
            } else {
                true
            };
            if takes_space {
                cur_pos += height + line_spacing;
            }

            // default: Update delay for staggered animation
            // Only apply delay when curPos >= 0 and not seeking
            if cur_pos >= 0.0 && !is_seek {
                if !anim.is_bg {
                    delay += base_delay;
                }
                // default: Reduce baseDelay after scrollToIndex for "waterfall" effect
                if idx >= scroll_to_index {
                    base_delay /= stagger_reduction_factor;
                }
            }
        }
    }

    /// Get number of animations
    pub fn len(&self) -> usize {
        self.animations.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.animations.is_empty()
    }

    /// Get all animations (for advanced access)
    pub fn animations(&self) -> &[LineAnimation] {
        &self.animations
    }

    /// Get animations slice (for AnimationBuffers)
    #[inline]
    pub fn animations_slice(&self) -> &[LineAnimation] {
        &self.animations
    }

    /// Get mutable access to all animations
    pub fn animations_mut(&mut self) -> &mut [LineAnimation] {
        &mut self.animations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Helper to create a LineAnimationManager with test data
    fn create_test_manager(line_count: usize) -> LineAnimationManager {
        let mut manager = LineAnimationManager::new();
        manager.set_viewport_height(800.0);
        let is_bg_flags: Vec<bool> = vec![false; line_count];
        manager.ensure_capacity(line_count, &is_bg_flags);
        manager
    }

    /// Helper to get blur level for a specific line after layout calculation
    fn get_blur_for_line(
        manager: &mut LineAnimationManager,
        line_count: usize,
        line_index: usize,
        scroll_to_index: usize,
        buffered_lines: &HashSet<usize>,
        viewport_width: f32,
    ) -> f32 {
        let line_heights: Vec<f32> = vec![48.0; line_count];
        manager.calc_layout_full(
            &line_heights,
            8.0, // line_spacing
            scroll_to_index,
            buffered_lines,
            true,  // is_playing
            true,  // is_seek (force immediate values)
            true,  // enable_scale
            0.97,  // inactive_scale
            0.75,  // bg_line_scale
            0.05,  // stagger_base_delay
            1.05,  // stagger_reduction_factor
            false, // is_non_dynamic
            viewport_width,
        );
        manager.animations[line_index].blur
    }

    // ========== Property 1: Active lines have zero blur ==========

    #[test]
    fn test_active_line_has_zero_blur() {
        let mut manager = create_test_manager(10);
        let mut buffered = HashSet::new();
        buffered.insert(5); // Line 5 is active

        let blur = get_blur_for_line(&mut manager, 10, 5, 5, &buffered, 1920.0);
        assert_eq!(blur, 0.0, "Active line should have zero blur");
    }

    #[test]
    fn test_multiple_active_lines_have_zero_blur() {
        let mut manager = create_test_manager(10);
        let mut buffered = HashSet::new();
        buffered.insert(3);
        buffered.insert(4);
        buffered.insert(5);

        for idx in 3..=5 {
            let blur = get_blur_for_line(&mut manager, 10, idx, 3, &buffered, 1920.0);
            assert_eq!(blur, 0.0, "Active line {} should have zero blur", idx);
        }
    }

    // ========== Property 2: Blur level calculation for inactive lines ==========

    #[test]
    fn test_blur_level_for_lines_before_current() {
        let mut manager = create_test_manager(10);
        let mut buffered = HashSet::new();
        buffered.insert(5);

        // Line 3 is before scroll_to_index (5)
        // Formula: 1 + abs(scrollToIndex - lineIndex) + 1 = 1 + abs(5 - 3) + 1 = 4
        let blur = get_blur_for_line(&mut manager, 10, 3, 5, &buffered, 1920.0);
        assert_eq!(
            blur, 4.0,
            "Line before current should have blur = 1 + distance + 1"
        );

        // Line 0: 1 + abs(5 - 0) + 1 = 7
        let blur = get_blur_for_line(&mut manager, 10, 0, 5, &buffered, 1920.0);
        assert_eq!(blur, 7.0, "Line 0 should have blur = 7");
    }

    #[test]
    fn test_blur_level_for_lines_after_current() {
        let mut manager = create_test_manager(10);
        let mut buffered = HashSet::new();
        buffered.insert(5);

        // Line 7 is after scroll_to_index (5), latest_index = 5
        // Formula: 1 + abs(lineIndex - max(scrollToIndex, latestIndex)) = 1 + abs(7 - 5) = 3
        let blur = get_blur_for_line(&mut manager, 10, 7, 5, &buffered, 1920.0);
        assert_eq!(
            blur, 3.0,
            "Line after current should have blur = 1 + distance"
        );

        // Line 9: 1 + abs(9 - 5) = 5
        let blur = get_blur_for_line(&mut manager, 10, 9, 5, &buffered, 1920.0);
        assert_eq!(blur, 5.0, "Line 9 should have blur = 5");
    }

    // ========== Property 3: Blur level viewport adjustment ==========

    #[test]
    fn test_blur_reduced_on_small_viewport() {
        let mut manager = create_test_manager(10);
        let mut buffered = HashSet::new();
        buffered.insert(5);

        // Line 3 with large viewport: blur = 4.0
        let blur_large = get_blur_for_line(&mut manager, 10, 3, 5, &buffered, 1920.0);

        // Line 3 with small viewport (<= 1024): blur = 4.0 * 0.8 = 3.2
        let blur_small = get_blur_for_line(&mut manager, 10, 3, 5, &buffered, 1024.0);

        assert_eq!(blur_large, 4.0, "Large viewport should have full blur");
        assert_eq!(blur_small, 3.2, "Small viewport should have blur * 0.8");
    }

    #[test]
    fn test_blur_not_reduced_above_1024() {
        let mut manager = create_test_manager(10);
        let mut buffered = HashSet::new();
        buffered.insert(5);

        let blur_1025 = get_blur_for_line(&mut manager, 10, 3, 5, &buffered, 1025.0);
        let blur_1920 = get_blur_for_line(&mut manager, 10, 3, 5, &buffered, 1920.0);

        assert_eq!(
            blur_1025, blur_1920,
            "Blur should be same for viewports > 1024"
        );
    }

    // ========== Property 4: Blur level maximum cap ==========
    // Note: The cap is applied in pipeline.rs, not in line_animation.rs
    // This test verifies the blur calculation can produce values > 32
    // The actual cap is tested in pipeline tests

    #[test]
    fn test_blur_can_exceed_32_before_cap() {
        let mut manager = create_test_manager(50);
        let mut buffered = HashSet::new();
        buffered.insert(25);

        // Line 0 with scroll_to_index = 25
        // Formula: 1 + abs(25 - 0) + 1 = 27
        let blur = get_blur_for_line(&mut manager, 50, 0, 25, &buffered, 1920.0);
        assert_eq!(
            blur, 27.0,
            "Blur calculation should produce 27 for distant line"
        );

        // Line 0 with scroll_to_index = 40
        // Formula: 1 + abs(40 - 0) + 1 = 42
        let mut buffered2 = HashSet::new();
        buffered2.insert(40);
        let blur = get_blur_for_line(&mut manager, 50, 0, 40, &buffered2, 1920.0);
        assert_eq!(
            blur, 42.0,
            "Blur calculation can exceed 32 (cap applied later)"
        );
    }
}
