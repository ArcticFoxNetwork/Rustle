//! Lens model for Apple Music-like visual effects
//!
//! Implements non-linear scaling and blur based on distance from focal point.
//! Creates a "lens" effect where text appears sharp and large at the center,
//! becoming smaller and blurrier towards the edges.

#![allow(dead_code)]

/// Lens model for calculating visual properties based on position
#[derive(Debug, Clone)]
pub struct LensModel {
    /// Maximum blur radius in pixels
    max_blur: f32,
    /// Scale reduction factor at edges (0.0-1.0)
    edge_scale_factor: f32,
    /// Blur curve exponent (higher = more dramatic blur)
    blur_curve: f32,
    /// Scale curve exponent (higher = more dramatic scaling)
    scale_curve: f32,
    /// Focal zone size (fraction of viewport height)
    focal_zone: f32,
}

impl LensModel {
    /// Create new lens model with default parameters
    pub fn new() -> Self {
        Self {
            max_blur: 20.0,
            edge_scale_factor: 0.97, // default
            blur_curve: 1.5,
            scale_curve: 3.0,
            focal_zone: 0.4,
        }
    }

    /// Create lens model with custom parameters
    pub fn with_params(
        max_blur: f32,
        edge_scale_factor: f32,
        blur_curve: f32,
        scale_curve: f32,
        focal_zone: f32,
    ) -> Self {
        Self {
            max_blur,
            edge_scale_factor,
            blur_curve,
            scale_curve,
            focal_zone: focal_zone.clamp(0.1, 0.9),
        }
    }

    /// Set focal zone size (fraction of viewport)
    pub fn set_focal_zone(&mut self, zone: f32) {
        self.focal_zone = zone.clamp(0.1, 0.9);
    }

    /// Calculate scale and blur for a given position
    ///
    /// # Arguments
    /// * `relative_y` - Y position relative to viewport center
    /// * `viewport_height` - Height of the viewport
    /// * `velocity` - Current scroll velocity (for motion blur)
    ///
    /// # Returns
    /// Tuple of (scale, blur)
    pub fn calculate(&self, relative_y: f32, viewport_height: f32, velocity: f32) -> (f32, f32) {
        // Normalize distance (-1.0 to 1.0)
        let half_viewport = viewport_height * self.focal_zone;
        let normalized_dist = (relative_y / half_viewport).clamp(-1.0, 1.0);
        let abs_dist = normalized_dist.abs();

        // Calculate scale using high-order curve
        // Only the center area is at full scale
        let scale_factor =
            1.0 - (1.0 - (1.0 - abs_dist).powf(self.scale_curve)) * (1.0 - self.edge_scale_factor);
        let scale = scale_factor.clamp(self.edge_scale_factor, 1.0);

        // Calculate base blur using curve
        let base_blur = abs_dist.powf(self.blur_curve) * self.max_blur;

        // Add motion blur based on velocity
        let motion_blur = (velocity.abs() / 1000.0).clamp(0.0, 5.0);
        let total_blur = base_blur + motion_blur;

        (scale, total_blur)
    }

    /// Calculate opacity based on distance
    ///
    /// Creates a subtle fade-out towards edges
    pub fn calculate_opacity(&self, relative_y: f32, viewport_height: f32) -> f32 {
        // Use a gentler curve for opacity
        let half_viewport = viewport_height * 0.5;
        let normalized_dist = (relative_y / half_viewport).clamp(-1.0, 1.0);
        let abs_dist = normalized_dist.abs();

        // Opacity drops more gently than scale/blur
        let opacity = 1.0 - abs_dist.powf(2.0) * 0.3;
        opacity.clamp(0.3, 1.0)
    }

    /// Calculate glow intensity for active lines
    ///
    /// Glow is strongest at center and fades out
    pub fn calculate_glow(&self, relative_y: f32, viewport_height: f32, is_active: bool) -> f32 {
        if !is_active {
            return 0.0;
        }

        // Glow fades with distance from center
        let half_viewport = viewport_height * self.focal_zone;
        let normalized_dist = (relative_y / half_viewport).clamp(-1.0, 1.0);
        let abs_dist = normalized_dist.abs();

        // Glow drops off quickly but smoothly
        let glow = (1.0 - abs_dist.powf(1.5)).clamp(0.0, 1.0);
        glow
    }

    /// Get the maximum blur radius
    pub fn max_blur(&self) -> f32 {
        self.max_blur
    }

    /// Set the maximum blur radius
    pub fn set_max_blur(&mut self, blur: f32) {
        self.max_blur = blur.clamp(0.0, 50.0);
    }

    /// Get the edge scale factor
    pub fn edge_scale_factor(&self) -> f32 {
        self.edge_scale_factor
    }

    /// Set the edge scale factor
    pub fn set_edge_scale_factor(&mut self, factor: f32) {
        self.edge_scale_factor = factor.clamp(0.5, 1.0);
    }
}

impl Default for LensModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lens_calculation() {
        let lens = LensModel::new();
        let viewport_h = 800.0;

        // At center: full scale, no blur
        let (scale, blur) = lens.calculate(0.0, viewport_h, 0.0);
        assert!((scale - 1.0).abs() < 0.01);
        assert!(blur < 1.0);

        // At edge: reduced scale, increased blur
        // With focal_zone=0.4, half_viewport=320, at 200px:
        // normalized_dist = 200/320 = 0.625
        // blur = 0.625^1.5 * 20 ≈ 9.88
        let (scale, blur) = lens.calculate(200.0, viewport_h, 0.0);
        assert!(scale < 1.0);
        assert!(blur > 5.0); // Blur increases with distance

        // At further edge: more blur
        let (scale, blur) = lens.calculate(300.0, viewport_h, 0.0);
        assert!(scale < 1.0);
        assert!(blur > 10.0);
    }

    #[test]
    fn test_opacity() {
        let lens = LensModel::new();
        let viewport_h = 800.0;

        // At center: full opacity
        let opacity = lens.calculate_opacity(0.0, viewport_h);
        assert!((opacity - 1.0).abs() < 0.01);

        // At edge: reduced opacity but still visible
        let opacity = lens.calculate_opacity(400.0, viewport_h);
        assert!(opacity > 0.3);
        assert!(opacity < 1.0);
    }

    #[test]
    fn test_motion_blur() {
        let lens = LensModel::new();
        let viewport_h = 800.0;

        // No velocity: no motion blur
        let (_, blur) = lens.calculate(0.0, viewport_h, 0.0);
        let base_blur = blur;

        // With velocity: additional blur
        let (_, blur) = lens.calculate(0.0, viewport_h, 1000.0);
        assert!(blur > base_blur);
    }
}
