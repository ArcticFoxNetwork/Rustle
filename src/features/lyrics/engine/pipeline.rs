//! iced Pipeline/Primitive 适配层
//!
//! 本模块实现 iced 框架的 `Pipeline` 和 `Primitive` trait，
//! 作为 iced shader widget 和底层 GPU 渲染（gpu_pipeline.rs）之间的桥梁。
//!
//! ## 架构关系
//!
//! ```text
//! lyrics.rs (UI Page)
//!     ↓ 创建 LyricsEngineProgram
//! program.rs (iced Program trait)
//!     ↓ draw() 返回 LyricsEnginePrimitive
//! pipeline.rs (本模块)
//!     ├─ LyricsEnginePrimitive: 收集渲染数据
//!     └─ LyricsEnginePipeline: 调用 GPU 管线
//!         ↓ prepare() / render()
//! gpu_pipeline.rs (LyricsGpuPipeline)
//!     └─ 实际的 wgpu 渲染实现
//! ```

#![allow(dead_code)]

use super::{LyricsEngine, LyricsEngineConfig};
use crate::features::lyrics::engine::{
    CachedShapedLine,
    gpu_pipeline::LyricsGpuPipeline,
    types::{ComputedLineStyle, LyricLineData},
};
use iced::Rectangle;
use iced::widget::shader::{Pipeline, Primitive};
use std::collections::HashSet;
use std::sync::Arc;
use wgpu;

/// iced Pipeline 实现，管理 GPU 管线生命周期
pub struct LyricsEnginePipeline {
    /// The GPU pipeline for text rendering
    gpu_pipeline: Option<LyricsGpuPipeline>,
    /// Whether the pipeline is initialized
    initialized: bool,
    /// Current format
    format: Option<wgpu::TextureFormat>,
    /// Cached render parameters for blur pass
    cached_render_params: Option<CachedRenderParams>,
}

/// Cached parameters for render pass (set in prepare, used in render)
#[derive(Clone)]
struct CachedRenderParams {
    viewport_width: u32,
    viewport_height: u32,
    current_time_ms: f32,
    font_size: f32,
    enable_blur: bool,
}

impl LyricsEnginePipeline {
    /// Create a new pipeline (uninitialized)
    pub fn new() -> Self {
        Self {
            gpu_pipeline: None,
            initialized: false,
            format: None,
            cached_render_params: None,
        }
    }
}

impl Default for LyricsEnginePipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Pipeline for LyricsEnginePipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let gpu_pipeline = LyricsGpuPipeline::new(device, format);
        Self {
            gpu_pipeline: Some(gpu_pipeline),
            initialized: true,
            format: Some(format),
            cached_render_params: None,
        }
    }
}

/// 歌词渲染数据（Primitive）
///
/// 包含一帧渲染所需的所有数据，由 `LyricsEngineProgram::draw()` 创建，
/// 传递给 `LyricsEnginePipeline::prepare()` 进行 GPU 数据准备。
///
/// Apple Music-style features:
/// - Per-line spring animations for Y position and scale
/// - Distance-based blur levels
/// - Staggered animation delays
/// - Interlude dots animation
///
/// Performance optimization:
/// - Uses Arc<Vec<LyricLineData>> to avoid cloning lyrics data each frame
/// - Uses pre-allocated AnimationBuffers for animation state
/// - Uses Arc<Vec<CachedShapedLine>> for Single Source of Truth text layout
#[derive(Debug, Clone)]
pub struct LyricsEnginePrimitive {
    /// Lyrics lines (Arc for O(1) clone, thread-safe)
    pub lines: Arc<Vec<LyricLineData>>,
    /// Cached shaped lines from LyricsEngine (Single Source of Truth)
    /// Contains all glyph positions, heights, and word bounds
    pub shaped_lines: Arc<Vec<CachedShapedLine>>,
    /// Current scroll position (legacy, kept for compatibility)
    pub scroll_position: f32,
    /// Buffered (active) line indices
    pub buffered_lines: HashSet<usize>,
    /// Scroll target index
    pub scroll_to_index: usize,
    /// Current playback time in milliseconds
    pub current_time_ms: f32,
    /// Engine configuration
    pub config: LyricsEngineConfig,
    /// Whether playback is active
    pub is_playing: bool,
    /// Interlude dots state
    pub interlude_dots: Option<InterludeDotsState>,
    /// Cached line heights from engine (in logical pixels)
    pub cached_line_heights: Vec<f32>,
    /// Per-line animated Y positions (in logical pixels)
    pub line_positions: Vec<f32>,
    /// Per-line animated scales (0.0 - 1.0)
    pub line_scales: Vec<f32>,
    /// Per-line blur levels (Apple Music-style distance-based blur)
    pub line_blur_levels: Vec<f32>,
    /// Per-line opacities
    pub line_opacities: Vec<f32>,
}

/// Serializable interlude dots state for primitive
#[derive(Debug, Clone)]
pub struct InterludeDotsState {
    pub enabled: bool,
    pub scale: f32,
    pub dot_opacities: [f32; 3],
    pub top: f32,
}

impl LyricsEnginePrimitive {
    /// Create a new primitive from engine state
    ///
    /// Captures all animation state including:
    /// - Per-line Y positions (from spring animations)
    /// - Per-line scales (from spring animations)
    /// - Per-line blur levels (Apple Music-style distance-based)
    /// - Per-line opacities
    /// - Interlude dots state
    /// - Cached shaped lines (Single Source of Truth for text layout)
    ///
    /// Performance optimization:
    /// - Uses Arc<Vec<LyricLineData>> for O(1) clone of lyrics data (thread-safe)
    /// - Uses Arc<Vec<CachedShapedLine>> for O(1) clone of shaped lines
    /// - Uses pre-allocated AnimationBuffers from engine instead of creating new Vecs each frame
    pub fn from_engine(
        engine: &mut LyricsEngine,
        lines: Arc<Vec<LyricLineData>>,
        current_time_ms: f32,
    ) -> Self {
        let line_count = lines.len();

        // Get pre-allocated animation buffers (updated in-place during engine.update())
        // This avoids calling individual getters that create new Vecs each frame
        let buffers = engine.animation_buffers();

        // Copy from pre-allocated buffers (much faster than creating new Vecs)
        let mut line_positions = buffers.positions().to_vec();
        let mut line_scales = buffers.scales().to_vec();
        let mut line_blur_levels = buffers.blur_levels().to_vec();
        let mut line_opacities = buffers.opacities().to_vec();

        // 确保所有向量的长度与 lines 匹配
        // 如果 animations 还没有被初始化（第一帧），用默认值填充
        // 默认值：position = 屏幕外, scale = 0.97 (inactive), blur = 高, opacity = 1.0
        let config = engine.config();
        let default_y = config.align_position * 800.0 * 2.0; // 屏幕外
        let default_scale = config.inactive_scale; // 0.97
        let default_blur = 3.0; // 中等模糊
        let default_opacity = 1.0;

        while line_positions.len() < line_count {
            line_positions.push(default_y + (line_positions.len() as f32 * 100.0));
        }
        while line_scales.len() < line_count {
            line_scales.push(default_scale);
        }
        while line_blur_levels.len() < line_count {
            line_blur_levels.push(default_blur);
        }
        while line_opacities.len() < line_count {
            line_opacities.push(default_opacity);
        }

        // Get cached shaped lines (Single Source of Truth)
        let shaped_lines = Arc::new(engine.cached_shaped_lines().to_vec());

        // Now get immutable data
        let dots = engine.interlude_dots();
        let interlude_dots = if dots.enabled {
            Some(InterludeDotsState {
                enabled: true,
                scale: dots.scale,
                dot_opacities: dots.dot_opacities,
                top: dots.top,
            })
        } else {
            None
        };

        Self {
            lines,                // Arc clone is O(1)
            shaped_lines,         // Arc clone is O(1)
            scroll_position: 0.0, // No longer used with per-line animations
            buffered_lines: engine.buffered_lines().clone(),
            scroll_to_index: engine.scroll_to_index(),
            current_time_ms,
            config: engine.config().clone(),
            is_playing: engine.is_playing(),
            interlude_dots,
            cached_line_heights: engine.cached_line_heights().to_vec(),
            line_positions,
            line_scales,
            line_blur_levels,
            line_opacities,
        }
    }

    /// Create a new primitive (legacy constructor)
    pub fn new(
        lines: Vec<LyricLineData>,
        scroll_position: f32,
        active_line: Option<usize>,
        current_time_ms: f32,
        config: LyricsEngineConfig,
    ) -> Self {
        let mut buffered_lines = HashSet::new();
        if let Some(idx) = active_line {
            buffered_lines.insert(idx);
        }
        let line_count = lines.len();
        Self {
            lines: Arc::new(lines),
            shaped_lines: Arc::new(Vec::new()), // Empty for legacy constructor
            scroll_position,
            buffered_lines,
            scroll_to_index: active_line.unwrap_or(0),
            current_time_ms,
            config,
            is_playing: true,
            interlude_dots: None,
            cached_line_heights: Vec::new(),
            line_positions: vec![0.0; line_count],
            line_scales: vec![1.0; line_count],
            line_blur_levels: vec![0.0; line_count],
            line_opacities: vec![1.0; line_count],
        }
    }

    /// Compute line styles for rendering
    pub fn compute_line_styles(&self, viewport: &Rectangle<f32>) -> Vec<ComputedLineStyle> {
        use crate::features::lyrics::engine::layout::LayoutMetrics;

        let mut styles = Vec::with_capacity(self.lines.len());
        let mut y_position = 0.0;

        // Calculate alignment position (default: 0.35 from top)
        let align_y = viewport.height * self.config.align_position;

        // Create lens model with config
        let mut lens = crate::features::lyrics::engine::LensModel::new();
        lens.set_edge_scale_factor(self.config.inactive_scale);

        // Calculate layout metrics
        let layout = LayoutMetrics::new(viewport.width, viewport.height, 1.0);

        for (idx, line) in self.lines.iter().enumerate() {
            // Calculate total height for this line (main + translation + romanized)
            let has_translation = line.translated.is_some();
            let has_romanized = line.romanized.is_some();
            let total_line_height = layout.total_line_height(has_translation, has_romanized);

            // Distance from alignment point
            let distance_from_center = y_position - self.scroll_position - align_y;

            // Use lens model to compute style
            let is_active = self.buffered_lines.contains(&idx);
            // velocity 为 0，primitive 中无法访问物理状态
            let (mut scale, blur) = lens.calculate(distance_from_center, viewport.height, 0.0);
            let opacity = lens.calculate_opacity(distance_from_center, viewport.height);
            let glow = lens.calculate_glow(distance_from_center, viewport.height, is_active);

            // Apply background line scale if applicable
            if line.is_bg && !is_active {
                scale *= self.config.bg_line_scale;
            }

            // Apply scale effect only if enabled
            if !self.config.enable_scale && !is_active {
                scale = 1.0;
            }

            // Apply hide passed lines (style)
            let final_opacity =
                if self.config.hide_passed_lines && idx < self.scroll_to_index && self.is_playing {
                    0.00001 // Nearly invisible but not zero
                } else if is_active {
                    0.85
                } else {
                    opacity
                };

            styles.push(ComputedLineStyle {
                y_position: y_position - self.scroll_position,
                scale,
                blur: if self.config.enable_blur { blur } else { 0.0 },
                opacity: final_opacity,
                glow,
                is_active,
            });

            y_position += total_line_height + self.config.line_spacing;
        }

        styles
    }

    /// Compute line styles for rendering using physical pixels
    ///
    /// This version uses per-line animated positions from LineAnimationManager
    /// instead of calculating positions from scroll offset.
    ///
    /// Apple Music-style features:
    /// - Per-line spring animations for Y position and scale
    /// - Distance-based blur (increases with distance from active line)
    /// - Staggered animation delays for "waterfall" effect
    /// - Proper opacity handling for background lines (CSS)
    pub fn compute_line_styles_physical(
        &self,
        viewport: &Rectangle<f32>,
        scale: f32,
    ) -> Vec<ComputedLineStyle> {
        let mut styles = Vec::with_capacity(self.lines.len());

        // Convert to physical pixels
        let physical_height = viewport.height * scale;

        // Calculate alignment position for lens calculations
        let align_y = physical_height * self.config.align_position;

        // Check if we have per-line animations
        let use_line_animations =
            !self.line_positions.is_empty() && self.line_positions.len() == self.lines.len();

        // Find the latest active line index for blur calculation
        let latest_index = self
            .buffered_lines
            .iter()
            .max()
            .copied()
            .unwrap_or(self.scroll_to_index);

        for (idx, line) in self.lines.iter().enumerate() {
            // Get animated Y position (in logical pixels, convert to physical)
            let y_position = if use_line_animations {
                self.line_positions[idx] * scale
            } else {
                // Fallback: use align_y (shouldn't happen in normal operation)
                align_y
            };

            // Get animated scale
            let animated_scale = if use_line_animations && idx < self.line_scales.len() {
                self.line_scales[idx]
            } else {
                1.0
            };

            let is_active = self.buffered_lines.contains(&idx);

            // Use pre-computed blur from LineAnimationManager if available
            // Otherwise calculate Apple Music-style distance-based blur
            let blur = if !self.config.enable_blur {
                0.0
            } else if idx < self.line_blur_levels.len() {
                // Use pre-computed blur from LineAnimationManager
                self.line_blur_levels[idx]
            } else if is_active {
                0.0
            } else {
                let mut level = 1.0;
                if idx < self.scroll_to_index {
                    // Lines above current: blur increases with distance
                    level += (self.scroll_to_index - idx) as f32 + 1.0;
                } else {
                    // Lines below current: blur increases with distance from latest active
                    level +=
                        (idx as i32 - latest_index.max(self.scroll_to_index) as i32).abs() as f32;
                }
                // Scale blur for smaller screens (default: window.innerWidth <= 1024 ? blur * 0.8 : blur)
                if physical_height <= 1024.0 {
                    level * 0.8
                } else {
                    level
                }
            };

            // Calculate glow for active lines
            let glow = if is_active { 0.5 } else { 0.0 };

            // Use pre-computed opacity from LineAnimationManager if available
            // This includes proper Apple Music-style handling for:
            // - Background lines: 0.0001 (inactive), 0.4 (active or not playing)
            // - Normal lines: 0.85 (active), 1.0 (inactive), 0.2 (non-dynamic)
            let final_opacity = if idx < self.line_opacities.len() {
                // Use pre-computed opacity from LineAnimationManager
                let base_opacity = self.line_opacities[idx];
                // Apply hide passed lines on top
                if self.config.hide_passed_lines && idx < self.scroll_to_index && self.is_playing {
                    0.00001
                } else {
                    base_opacity
                }
            } else {
                // Fallback calculation (shouldn't happen in normal operation)
                if self.config.hide_passed_lines && idx < self.scroll_to_index && self.is_playing {
                    0.00001
                } else if is_active {
                    0.85
                } else if line.is_bg {
                    // CSS: .lyricBgLine { opacity: 0.0001; }
                    // .lyricBgLine.active { opacity: 0.4; }
                    // :not(.playing) > .lyricBgLine { opacity: 0.4; }
                    if !self.is_playing { 0.4 } else { 0.0001 }
                } else {
                    1.0
                }
            };

            styles.push(ComputedLineStyle {
                y_position,
                scale: animated_scale,
                blur: blur.min(32.0), // default: Math.min(32, blur)
                opacity: final_opacity,
                glow,
                is_active,
            });
        }

        styles
    }

    /// Calculate X position for a line based on duet status
    pub fn line_x_position(
        &self,
        line: &LyricLineData,
        line_width: f32,
        container_width: f32,
    ) -> f32 {
        use crate::features::lyrics::engine::layout::LayoutMetrics;
        let layout = LayoutMetrics::new(container_width, 800.0, 1.0);
        layout.line_x_position(line.is_duet, line_width, container_width)
    }
}

impl Primitive for LyricsEnginePrimitive {
    type Pipeline = LyricsEnginePipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &iced::widget::shader::Viewport,
    ) {
        if !pipeline.initialized {
            return;
        }

        let Some(gpu_pipeline) = &mut pipeline.gpu_pipeline else {
            return;
        };

        // Get scale factor for physical pixels
        let scale = viewport.scale_factor() as f32;

        // Full viewport (window) size in physical pixels
        let full_viewport_width = viewport.physical_width() as f32;
        let full_viewport_height = viewport.physical_height() as f32;

        // Widget bounds in physical pixels
        let bounds_x = bounds.x * scale;
        let bounds_y = bounds.y * scale;
        let bounds_width = bounds.width * scale;
        let bounds_height = bounds.height * scale;

        // Calculate font size using FontSizeConfig
        // The config handles min/max clamping and multiplier
        // We use logical height (bounds.height) for calculation, then multiply by scale
        let font_size = self
            .config
            .font_size_config
            .calculate_font_size(bounds.height)
            * scale;

        // Compute line styles based on scroll position (using physical pixels)
        let line_styles = self.compute_line_styles_physical(bounds, scale);

        // Convert cached_line_heights from logical to physical pixels
        // LyricsEngine calculates heights in logical pixels, GPU needs physical pixels
        let physical_line_heights: Vec<f32> =
            self.cached_line_heights.iter().map(|h| h * scale).collect();

        // Prepare GPU pipeline with new data
        // Use cached shaped_lines from LyricsEngine (Single Source of Truth)
        // This avoids duplicate text shaping in GPU pipeline
        gpu_pipeline.prepare_with_shaped_lines(
            device,
            queue,
            full_viewport_width,
            full_viewport_height,
            bounds_x,
            bounds_y,
            bounds_width,
            bounds_height,
            &self.lines,        // Arc<Vec<T>> derefs to &[T]
            &self.shaped_lines, // Pre-shaped lines from LyricsEngine
            &line_styles,
            &physical_line_heights, // Pre-calculated by LyricsEngine, converted to physical pixels
            self.current_time_ms,
            self.scroll_position,
            font_size,
            self.config.word_fade_width,
            scale, // Scale factor for logical to physical conversion
        );

        // Prepare interlude dots if present
        if let Some(ref dots) = self.interlude_dots {
            let mut dots_state = crate::features::lyrics::engine::InterludeDots::new();
            dots_state.left = bounds_width * 0.5; // Centered horizontally
            dots_state.top = dots.top * scale;
            dots_state.enabled = dots.enabled;
            dots_state.scale = dots.scale;
            dots_state.dot_opacities = dots.dot_opacities;

            gpu_pipeline.prepare_interlude_dots(
                device,
                queue,
                &dots_state,
                full_viewport_width,
                full_viewport_height,
                bounds_x,
                bounds_y,
                scale,
            );
        }

        // Prepare blur rendering resources
        let enable_blur = self.config.enable_blur && gpu_pipeline.is_blur_enabled();
        if enable_blur {
            gpu_pipeline.prepare_blur(
                device,
                queue,
                viewport.physical_width(),
                viewport.physical_height(),
                self.current_time_ms,
                font_size,
            );
        }

        // Cache render parameters
        pipeline.cached_render_params = Some(CachedRenderParams {
            viewport_width: viewport.physical_width(),
            viewport_height: viewport.physical_height(),
            current_time_ms: self.current_time_ms,
            font_size,
            enable_blur,
        });
    }

    fn render(
        &self,
        pipeline: &Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        if !pipeline.initialized {
            return;
        }

        let Some(gpu_pipeline) = &pipeline.gpu_pipeline else {
            return;
        };

        // 检查是否启用模糊效果并且有缓存的渲染参数
        let use_blur = pipeline
            .cached_render_params
            .as_ref()
            .map(|p| p.enable_blur)
            .unwrap_or(false);

        if use_blur {
            // 逐行模糊渲染模式 (正确的 Apple Music 风格)
            // 每行歌词独立渲染和模糊，避免不同行之间的模糊混合
            if let Some(ref params) = pipeline.cached_render_params {
                gpu_pipeline.render_with_per_line_blur(
                    device,
                    queue,
                    encoder,
                    target,
                    clip_bounds,
                    params.viewport_width,
                    params.viewport_height,
                );
                return;
            }
        }

        // 直接渲染模式（不使用多 pass 模糊）
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Lyrics Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Don't clear - we're rendering on top of background
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Set scissor rect to clip bounds
        render_pass.set_scissor_rect(
            clip_bounds.x,
            clip_bounds.y,
            clip_bounds.width,
            clip_bounds.height,
        );

        // Render interlude dots first (behind text)
        gpu_pipeline.render_interlude_dots(&mut render_pass);

        // Render lyrics text
        gpu_pipeline.render(&mut render_pass);
    }
}
