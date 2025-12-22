//! Layout calculations for Apple Music-style lyrics
//!
//! Shared logic for determining font sizes, line heights, and spacing based on viewport size.
//! Supports main lyrics, translation, and romanized text.

#![allow(dead_code)]

use super::types::FontSizeConfig;

/// Layout parameters calculated from viewport dimensions
#[derive(Debug, Clone, Copy)]
pub struct LayoutMetrics {
    /// Main lyric font size in pixels
    pub main_font_size: f32,
    /// Translation/sub line font size in pixels
    pub sub_font_size: f32,
    /// Romanized text font size in pixels
    pub roman_font_size: f32,
    /// Main line height in pixels
    pub line_height: f32,
    /// Translation line height in pixels
    pub trans_line_height: f32,
    /// Romanized line height in pixels
    pub roman_line_height: f32,
    /// Spacing between lyric groups in pixels
    pub line_spacing: f32,
    /// Content width in pixels
    pub content_width: f32,
    /// Left padding for normal lines
    pub padding_left: f32,
    /// Right padding for duet lines
    pub padding_right: f32,
}

impl LayoutMetrics {
    /// Calculate layout metrics based on viewport size and scale factor
    pub fn new(viewport_width: f32, viewport_height: f32, scale_factor: f32) -> Self {
        Self::new_with_config(
            viewport_width,
            viewport_height,
            scale_factor,
            &FontSizeConfig::default(),
        )
    }

    /// Calculate layout metrics with custom font size configuration
    pub fn new_with_config(
        viewport_width: f32,
        viewport_height: f32,
        scale_factor: f32,
        config: &FontSizeConfig,
    ) -> Self {
        let logical_height = viewport_height / scale_factor;

        // Calculate main font size using config (with min/max clamping and multiplier)
        let main_font_size_logical = config.calculate_font_size(logical_height);

        // Calculate sub font sizes using config ratios
        let sub_font_size_logical = config.calculate_translation_size(main_font_size_logical);
        let roman_font_size_logical = config.calculate_romanized_size(main_font_size_logical);

        // Convert to physical pixels
        let main_font_size = main_font_size_logical * scale_factor;
        let sub_font_size = sub_font_size_logical * scale_factor;
        let roman_font_size = roman_font_size_logical * scale_factor;

        // Line heights (1.4x font size)
        let line_height = main_font_size * 1.4;
        let trans_line_height = sub_font_size * 1.3;
        let roman_line_height = roman_font_size * 1.2;

        // Spacing between lyric groups
        let line_spacing = main_font_size * 0.5;

        // Content area
        let content_width = viewport_width * 0.8;
        let padding_left = viewport_width * 0.05;
        let padding_right = viewport_width * 0.05;

        Self {
            main_font_size,
            sub_font_size,
            roman_font_size,
            line_height,
            trans_line_height,
            roman_line_height,
            line_spacing,
            content_width,
            padding_left,
            padding_right,
        }
    }

    /// Calculate total height for a lyric line including translation and romanized text
    pub fn total_line_height(&self, has_translation: bool, has_romanized: bool) -> f32 {
        let mut height = self.line_height;
        if has_translation {
            height += self.trans_line_height;
        }
        if has_romanized {
            height += self.roman_line_height;
        }
        height
    }

    /// Calculate X position for a line (left-aligned for normal, right-aligned for duet)
    pub fn line_x_position(&self, is_duet: bool, line_width: f32, container_width: f32) -> f32 {
        if is_duet {
            // Right-aligned for duet lines
            container_width - line_width - self.padding_right
        } else {
            // Left-aligned for normal lines
            self.padding_left
        }
    }
}
