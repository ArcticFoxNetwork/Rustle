//! GPU-accelerated Lyrics Render Engine
//!
//! A high-performance render engine for Apple Music-style lyrics visualization.
//!
//! ## Architecture
//!
//! This engine bypasses glyphon's limitations for dynamic lyrics:
//! - Uses cosmic-text directly for text shaping
//! - Custom SDF glyph atlas management (MSDF)
//! - Custom vertex structure with timing data
//! - GPU-based word-by-word highlighting
//!
//! ## Key Components
//!
//! - `LyricsEngine`: Main engine managing state, scrolling, and line tracking
//! - `LyricsGpuPipeline`: GPU pipeline for text rendering with SDF shader
//! - `SdfCache`: MSDF texture atlas for glyph storage
//! - `TextShaper`: cosmic-text based text shaping
//! - `ScrollPhysics`: Physics simulation for smooth scrolling
//! - `LensModel`: Visual effects (scale, blur, opacity) based on distance
//! - `Spring`: Spring-based animation system

// Core modules
pub mod conversion;
pub mod gpu_pipeline;
pub mod interlude_dots;
pub mod layout;
pub mod lens;
pub mod line_animation;
pub mod per_line_blur;
pub mod physics;
pub mod pipeline;
pub mod program;
pub mod sdf_cache;
pub mod sdf_generator;
pub mod spring;
pub mod text_shaper;
pub mod types;
pub mod vertex;
pub mod word_splitter;

// Re-exports for convenience
pub use interlude_dots::InterludeDots;
pub use lens::LensModel;
pub use line_animation::{AnimationBuffers, LineAnimationManager};
pub use physics::{ScrollPhysics, ScrollState};
pub use sdf_cache::SdfPreGenerator;
pub use text_shaper::TextShaper;
pub use types::{ComputedLineStyle, FontSizeConfig, LyricLineData, WordData};

use cosmic_text::FontSystem;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Instant;

/// Configuration for the lyrics engine
///
/// All timing values are in seconds, distances in logical pixels.
///
/// ## Features
///
/// This configuration supports visual effects:
/// - Per-line spring animations with configurable parameters
/// - Distance-based blur (increases with distance from active line)
/// - Staggered animation delays for "waterfall" effect
/// - Emphasis effects for long words (scale, glow, float)
/// - Interlude dots animation
/// - Translation and romanized text support
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct LyricsEngineConfig {
    // === Font Size ===
    /// Font size configuration for lyrics rendering
    pub font_size_config: FontSizeConfig,

    // === Layout ===
    /// Base line height in logical pixels
    pub line_height: f32,
    /// Translation line height ratio (relative to main line)
    pub trans_height_ratio: f32,
    /// Romanized text height ratio (relative to main line)
    pub roman_height_ratio: f32,
    /// Line spacing in logical pixels
    pub line_spacing: f32,
    /// Maximum lines to render
    pub max_lines: usize,
    /// Alignment position (0.0 = top, 0.5 = center, 1.0 = bottom)
    /// Default: 0.35
    pub align_position: f32,
    /// Alignment anchor for current line
    pub align_anchor: AlignAnchor,

    // === Visual Effects ===
    /// Enable GPU blur effects (distance-based blur)
    pub enable_blur: bool,
    /// Enable scale effect for non-active lines
    pub enable_scale: bool,
    /// Blur pyramid levels
    pub blur_levels: usize,
    /// Scale factor for non-active lines (default: 0.97)
    pub inactive_scale: f32,
    /// Scale factor for background lyrics (default: 0.75)
    pub bg_line_scale: f32,
    /// Word fade width in em units (default: 0.5)
    /// - 0.5 for iPad-like effect
    /// - 1.0 for Android-like effect
    pub word_fade_width: f32,

    // === Physics ===
    /// Scroll timeout before auto-return (seconds)
    /// Default: 5.0
    pub scroll_timeout: f32,
    /// Friction coefficient for inertia scrolling
    pub scroll_friction: f32,
    /// Snap threshold velocity
    pub snap_threshold: f32,
    /// Maximum overscroll distance
    pub max_overscroll: f32,

    // === Spring Parameters ===
    // Position Y: mass=0.9, damping=15, stiffness=90
    /// Spring mass for Y position
    pub spring_mass: f32,
    /// Spring damping for Y position
    pub spring_damping: f32,
    /// Spring stiffness for Y position
    pub spring_stiffness: f32,

    // Scale: mass=2, damping=25, stiffness=100 (normal)
    //        mass=1, damping=20, stiffness=50 (background)
    /// Spring mass for scale (normal lines)
    pub scale_spring_mass: f32,
    /// Spring damping for scale (normal lines)
    pub scale_spring_damping: f32,
    /// Spring stiffness for scale (normal lines)
    pub scale_spring_stiffness: f32,
    /// Spring mass for scale (background lines)
    pub scale_bg_spring_mass: f32,
    /// Spring damping for scale (background lines)
    pub scale_bg_spring_damping: f32,
    /// Spring stiffness for scale (background lines)
    pub scale_bg_spring_stiffness: f32,

    // === Staggered Animation ===
    /// Base delay for staggered animation (seconds)
    /// Default: 0.05
    pub stagger_base_delay: f32,
    /// Delay reduction factor for lines after target
    /// Default: 1.05 (delay /= 1.05 for each line)
    pub stagger_reduction_factor: f32,

    // === Interlude ===
    /// Minimum interlude duration to show dots (ms)
    pub interlude_min_duration: u64,
    /// Target breathe duration for interlude dots (ms)
    pub interlude_breathe_duration: u64,

    // === Rendering ===
    /// Hide passed lines (scroll them out of view)
    pub hide_passed_lines: bool,
    /// Overscan distance for virtualization (pixels)
    /// Default: 300
    pub overscan_px: f32,

    // === Emphasis Effects ===
    /// Enable emphasis effects for long words
    pub enable_emphasis: bool,
    /// Minimum word duration for emphasis (ms)
    /// Default: 1000ms
    pub emphasis_min_duration: u64,
    /// Maximum word length for emphasis (characters)
    /// Default: 7 for non-CJK, unlimited for CJK
    pub emphasis_max_length: usize,
}

/// Alignment anchor for current lyric line
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum AlignAnchor {
    Top,
    #[default]
    Center,
    Bottom,
}

impl Default for LyricsEngineConfig {
    fn default() -> Self {
        Self {
            // Font Size
            font_size_config: FontSizeConfig::default(),

            // Layout
            line_height: 48.0,
            trans_height_ratio: 0.7,
            roman_height_ratio: 0.6,
            line_spacing: 8.0,
            max_lines: 128,
            align_position: 0.35,
            align_anchor: AlignAnchor::Center,

            // Visual Effects
            enable_blur: true,
            enable_scale: true,
            blur_levels: 5,
            inactive_scale: 0.97,
            bg_line_scale: 0.75,
            word_fade_width: 0.5,

            // Physics
            scroll_timeout: 5.0,
            scroll_friction: 0.995,
            snap_threshold: 50.0,
            max_overscroll: 200.0,

            // Spring (defaults for position Y)
            spring_mass: 0.9,
            spring_damping: 15.0,
            spring_stiffness: 90.0,

            // Spring (defaults for scale - normal lines)
            scale_spring_mass: 2.0,
            scale_spring_damping: 25.0,
            scale_spring_stiffness: 100.0,

            // Spring (defaults for scale - background lines)
            scale_bg_spring_mass: 1.0,
            scale_bg_spring_damping: 20.0,
            scale_bg_spring_stiffness: 50.0,

            // Staggered Animation (defaults)
            stagger_base_delay: 0.05,
            stagger_reduction_factor: 1.05,

            // Interlude
            interlude_min_duration: 4000,
            interlude_breathe_duration: 1500,

            // Rendering
            hide_passed_lines: false,
            overscan_px: 300.0,

            // Emphasis Effects
            enable_emphasis: true,
            emphasis_min_duration: 1000,
            emphasis_max_length: 7,
        }
    }
}

/// Shared font system type
pub type SharedFontSystem = Arc<Mutex<FontSystem>>;

/// 缓存的 shaped line 数据
/// 文本布局的唯一数据源
#[derive(Debug, Clone)]
pub struct CachedShapedLine {
    /// Main text shaped result
    pub main: text_shaper::ShapedLine,
    /// Translation text shaped result (if any)
    pub translation: Option<text_shaper::ShapedLine>,
    /// Romanized text shaped result (if any)
    pub romanized: Option<text_shaper::ShapedLine>,
    /// Total height of this line (main + translation + romanized)
    pub total_height: f32,
}

/// Main lyrics engine - manages scrolling, layout, and rendering
pub struct LyricsEngine {
    /// Configuration
    config: LyricsEngineConfig,
    /// Physics simulation for user scrolling (not used for auto-scroll anymore)
    physics: ScrollPhysics,
    /// Lens model for visual effects
    lens: LensModel,
    /// Interlude dots animation
    interlude_dots: InterludeDots,
    /// Text shaper for calculating line heights
    text_shaper: TextShaper,
    /// Per-line animation manager
    line_animations: LineAnimationManager,
    /// Pre-allocated animation buffers for efficient rendering
    /// Avoids per-frame Vec allocations in LyricsEnginePrimitive::from_engine
    animation_buffers: AnimationBuffers,
    /// Current time in milliseconds
    current_time_ms: f64,
    /// Whether mouse is hovering
    is_hovering: bool,
    /// Whether playback is active
    is_playing: bool,
    /// Current scroll target index (for auto-scroll)
    scroll_to_index: usize,
    /// Buffered (active) line indices
    buffered_lines: std::collections::HashSet<usize>,
    /// Hot (currently playing) line indices
    hot_lines: std::collections::HashSet<usize>,
    /// Last update time
    last_update: Instant,
    /// Cached shaped lines (Single Source of Truth for text layout)
    /// Contains all glyph positions, heights, and word bounds
    cached_shaped_lines: Vec<CachedShapedLine>,
    /// Cached line heights (derived from cached_shaped_lines for convenience)
    cached_line_heights: Vec<f32>,
    /// Last known content width for invalidation
    last_content_width: f32,
    /// Last known font size for invalidation
    last_font_size: f32,
    /// Viewport height for layout calculations
    viewport_height: f32,
}

impl LyricsEngine {
    /// Create new lyrics engine with shared font system
    ///
    /// The font system should be created once at app startup and shared
    /// to avoid the expensive FontSystem::new() call.
    pub fn new_with_font_system(config: LyricsEngineConfig, font_system: SharedFontSystem) -> Self {
        let mut physics = ScrollPhysics::new(800.0, config.line_height);
        physics.set_friction(config.scroll_friction);
        physics.set_snap_threshold(config.snap_threshold);
        physics.set_max_overscroll(config.max_overscroll);

        let mut lens = LensModel::new();
        lens.set_edge_scale_factor(config.inactive_scale);

        // Use provided font system for text shaping
        let text_shaper = TextShaper::new(font_system);

        // Create line animation manager with config
        let mut line_animations = LineAnimationManager::new();
        line_animations.set_align_position(config.align_position);
        // Convert AlignAnchor from config to line_animation's AlignAnchor
        line_animations.set_align_anchor(match config.align_anchor {
            AlignAnchor::Top => line_animation::AlignAnchor::Top,
            AlignAnchor::Center => line_animation::AlignAnchor::Center,
            AlignAnchor::Bottom => line_animation::AlignAnchor::Bottom,
        });
        line_animations.set_hide_passed_lines(config.hide_passed_lines);

        Self {
            physics,
            lens,
            interlude_dots: InterludeDots::new(),
            text_shaper,
            line_animations,
            animation_buffers: AnimationBuffers::new(),
            config,
            current_time_ms: 0.0,
            is_hovering: false,
            is_playing: true,
            scroll_to_index: 0,
            buffered_lines: std::collections::HashSet::new(),
            hot_lines: std::collections::HashSet::new(),
            last_update: Instant::now(),
            cached_shaped_lines: Vec::new(),
            cached_line_heights: Vec::new(),
            last_content_width: 0.0,
            last_font_size: 0.0,
            viewport_height: 800.0,
        }
    }

    /// Create new lyrics engine (creates its own FontSystem - slower, use new_with_font_system instead)
    #[allow(dead_code)]
    pub fn new(config: LyricsEngineConfig) -> Self {
        let font_system: SharedFontSystem = Arc::new(Mutex::new(FontSystem::new()));
        Self::new_with_font_system(config, font_system)
    }

    /// Update the engine state
    ///
    /// This must be called every frame with delta time in seconds.
    pub fn update(&mut self, dt: f32) {
        // Update physics simulation (for user scrolling)
        self.physics.update(dt, self.is_hovering);

        // Check if we should return to auto-play using configured timeout
        if self.physics.state() == ScrollState::Idle {
            if self.physics.time_since_interaction() > self.config.scroll_timeout
                && !self.is_hovering
            {
                self.physics.start_auto_play();
            }
        }

        // Update interlude dots animation
        self.interlude_dots.update(dt);

        // Update line animations
        self.line_animations.update(dt);

        // Update animation buffers from line animations (in-place, no allocations)
        // This prepares the data for LyricsEnginePrimitive::from_engine
        self.animation_buffers
            .update_from_manager(&self.line_animations);

        // Update current time for animations
        self.current_time_ms += (dt * 1000.0) as f64;
        self.last_update = Instant::now();
    }

    /// Get the current configuration
    pub fn config(&self) -> &LyricsEngineConfig {
        &self.config
    }

    /// Get mutable reference to configuration
    #[allow(dead_code)]
    pub fn config_mut(&mut self) -> &mut LyricsEngineConfig {
        &mut self.config
    }

    /// Update configuration at runtime
    #[allow(dead_code)]
    pub fn set_config(&mut self, config: LyricsEngineConfig) {
        self.physics.set_friction(config.scroll_friction);
        self.physics.set_snap_threshold(config.snap_threshold);
        self.physics.set_max_overscroll(config.max_overscroll);
        self.lens.set_edge_scale_factor(config.inactive_scale);
        self.config = config;
    }

    /// Handle mouse wheel event
    pub fn handle_wheel(&mut self, delta: f32) {
        self.physics.apply_impulse(delta);
    }

    /// Handle mouse move
    #[allow(dead_code)]
    pub fn handle_mouse_move(&mut self, _position: iced::Point) {
        self.is_hovering = true;
    }

    /// Handle mouse exit
    #[allow(dead_code)]
    pub fn handle_mouse_exit(&mut self) {
        self.is_hovering = false;
    }

    /// Get current scroll position (legacy, for compatibility)
    /// 使用逐行动画，滚动位置由 line_animations 管理
    pub fn scroll_position(&self) -> f32 {
        // 返回 0，实际位置在 line_animations 中
        0.0
    }

    /// Set current scroll position (legacy, for compatibility)
    #[allow(dead_code)]
    pub fn set_scroll_position(&mut self, _position: f32) {
        // No-op with per-line animations
    }

    /// Get current animated Y positions for all lines (in logical pixels)
    pub fn line_positions(&mut self) -> Vec<f32> {
        self.line_animations.current_positions()
    }

    /// Get current animated scales for all lines (0.0 - 1.0)
    pub fn line_scales(&mut self) -> Vec<f32> {
        self.line_animations.current_scales()
    }

    /// Get line animation manager (for rendering)
    pub fn line_animations(&self) -> &LineAnimationManager {
        &self.line_animations
    }

    /// Get mutable line animation manager
    pub fn line_animations_mut(&mut self) -> &mut LineAnimationManager {
        &mut self.line_animations
    }

    /// Invalidate layout cache to force re-calculation on next update
    /// Call this when viewport size changes
    pub fn invalidate_layout(&mut self) {
        self.last_content_width = 0.0;
        self.last_font_size = 0.0;
    }

    /// Get animation buffers reference (for efficient rendering)
    ///
    /// Returns pre-allocated buffers containing positions, scales, blur levels,
    /// and opacities. These are updated in-place during `update()` to avoid
    /// per-frame allocations.
    pub fn animation_buffers(&self) -> &AnimationBuffers {
        &self.animation_buffers
    }

    /// Get physics state
    #[allow(dead_code)]
    pub fn physics_state(&self) -> ScrollState {
        self.physics.state()
    }

    /// Get current time in milliseconds
    #[allow(dead_code)]
    pub fn current_time(&self) -> f64 {
        self.current_time_ms
    }

    /// 设置当前播放时间并更新行状态
    /// 与播放同步的主入口
    ///
    /// For accurate scroll positioning with text wrapping, call
    /// `set_viewport_info` first to update line height calculations.
    pub fn set_current_time(&mut self, time_ms: f64, lines: &[LyricLineData], is_seek: bool) {
        self.current_time_ms = time_ms;

        // 检查歌词是否变化（切歌或首次加载）
        // 如果歌词数量变化，需要重新初始化动画并立即排版
        let lyrics_changed = self.line_animations.len() != lines.len();

        // Update hot lines (currently playing)
        let scroll_changed = self.update_hot_lines(time_ms, lines, is_seek);

        // Check for interlude
        self.update_interlude(time_ms, lines);

        // Update scroll target if needed (auto-scroll)
        // 触发条件：
        // 1. buffered_lines 发生变化（scroll_changed）
        // 2. 显式 seek 操作（is_seek）
        // 3. 歌词变化（lyrics_changed）- 切歌或首次加载时立即排版
        if scroll_changed || is_seek || lyrics_changed {
            self.calc_scroll_target(lines, is_seek || lyrics_changed);
        }
    }

    /// Set viewport information and recalculate line heights if needed
    /// Call this before set_current_time when viewport size changes
    ///
    /// Parameters:
    /// - lines: The lyrics lines
    /// - content_width: Available width for text (in logical pixels)
    /// - font_size: Font size (in logical pixels, typically 48.0)
    /// - viewport_height: Viewport height (in logical pixels)
    pub fn set_viewport_info(
        &mut self,
        lines: &[LyricLineData],
        content_width: f32,
        font_size: f32,
        viewport_height: f32,
    ) {
        self.viewport_height = viewport_height;
        self.line_animations.set_viewport_height(viewport_height);
        self.calculate_line_heights(lines, content_width, font_size);
    }

    /// Calculate and set the scroll target position
    ///
    /// This now uses per-line animations instead of global scroll position.
    /// Each line has its own Spring for smooth, independent animation.
    ///
    /// Features:
    /// - Per-line spring animations with configurable parameters
    /// - Staggered delays for "waterfall" effect
    /// - Distance-based blur calculation
    fn calc_scroll_target(&mut self, lines: &[LyricLineData], is_seek: bool) {
        if lines.is_empty() {
            return;
        }

        // Ensure we have animations for all lines
        let is_bg_flags: Vec<bool> = lines.iter().map(|l| l.is_bg).collect();
        let was_reset = self
            .line_animations
            .ensure_capacity(lines.len(), &is_bg_flags);

        // If animations were reset (new song), force seek behavior
        let is_seek = is_seek || was_reset;

        // Use the same line_spacing formula as GPU pipeline
        let line_spacing = self.config.line_height * 0.5;

        // Get line heights (use cached if available)
        let line_heights: Vec<f32> = lines
            .iter()
            .enumerate()
            .map(|(idx, _)| {
                if idx < self.cached_line_heights.len() {
                    self.cached_line_heights[idx]
                } else {
                    self.config.line_height * 1.4
                }
            })
            .collect();

        // Calculate layout using LineAnimationManager with configurable stagger
        self.line_animations.calc_layout_with_stagger(
            &line_heights,
            line_spacing,
            self.scroll_to_index,
            &self.buffered_lines,
            self.is_playing,
            is_seek,
            self.config.enable_scale,
            self.config.inactive_scale,
            self.config.bg_line_scale,
            self.config.stagger_base_delay,
            self.config.stagger_reduction_factor,
        );
    }

    /// Calculate and cache line heights using text shaper
    /// Call this when lyrics change or viewport width changes
    ///
    /// 文本布局的唯一数据源
    /// All shaped line data (glyphs, positions, heights) is cached here
    /// and passed to GPU pipeline for rendering.
    ///
    /// Parameters:
    /// - lines: The lyrics lines to calculate heights for
    /// - content_width: Available width for text (in logical pixels)
    /// - font_size: Font size (in logical pixels)
    pub fn calculate_line_heights(
        &mut self,
        lines: &[LyricLineData],
        content_width: f32,
        font_size: f32,
    ) {
        // Check if we need to recalculate
        let width_changed = (self.last_content_width - content_width).abs() > 1.0;
        let font_changed = (self.last_font_size - font_size).abs() > 0.1;
        let lines_changed = self.cached_shaped_lines.len() != lines.len();

        // If nothing changed, skip the expensive shaping operation
        if !lines_changed && !width_changed && !font_changed && !self.cached_shaped_lines.is_empty()
        {
            return;
        }

        // Calculate font sizes for translation and romanized text
        let trans_font_size = (font_size * self.config.trans_height_ratio).max(10.0);
        let roman_font_size = (font_size * self.config.roman_height_ratio).max(10.0);

        // Shape all lines and cache the results (Single Source of Truth)
        self.cached_shaped_lines = lines
            .iter()
            .map(|line| {
                // Shape main lyrics
                let main_shaped =
                    self.text_shaper
                        .shape_line(&line.text, &line.words, font_size, content_width);
                let mut total_height = main_shaped.height;

                // Shape translation line if present
                let translation_shaped = if let Some(ref translated) = line.translated {
                    if !translated.is_empty() {
                        let shaped = self.text_shaper.shape_simple(
                            translated,
                            trans_font_size,
                            content_width,
                        );
                        total_height += shaped.height;
                        Some(shaped)
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Shape romanized line if present
                let romanized_shaped = if let Some(ref romanized) = line.romanized {
                    if !romanized.is_empty() {
                        let shaped = self.text_shaper.shape_simple(
                            romanized,
                            roman_font_size,
                            content_width,
                        );
                        total_height += shaped.height;
                        Some(shaped)
                    } else {
                        None
                    }
                } else {
                    None
                };

                CachedShapedLine {
                    main: main_shaped,
                    translation: translation_shaped,
                    romanized: romanized_shaped,
                    total_height,
                }
            })
            .collect();

        // Update cached line heights (for convenience)
        self.cached_line_heights = self
            .cached_shaped_lines
            .iter()
            .map(|s| s.total_height)
            .collect();

        self.last_content_width = content_width;
        self.last_font_size = font_size;
    }

    /// Get cached line heights (for external use)
    pub fn cached_line_heights(&self) -> &[f32] {
        &self.cached_line_heights
    }

    /// Get cached shaped lines (Single Source of Truth for GPU rendering)
    pub fn cached_shaped_lines(&self) -> &[CachedShapedLine] {
        &self.cached_shaped_lines
    }

    /// 设置异步任务预计算的 shaped lines
    /// 允许在后台线程进行文本 shaping
    pub fn set_cached_shaped_lines(&mut self, shaped_lines: Vec<CachedShapedLine>) {
        // Update cached line heights from shaped lines
        self.cached_line_heights = shaped_lines.iter().map(|s| s.total_height).collect();

        self.cached_shaped_lines = shaped_lines;

        // Mark that we have valid shaped data
        // last_content_width/last_font_size 不在这里更新，异步任务使用的是当时的视口尺寸
        // 视口变化时会重新调用 calculate_line_heights
    }

    /// Update hot lines based on current time
    /// Returns true if buffered_lines changed (need to recalculate layout/blur)
    ///
    /// 歌词选择状态定义：
    /// - 普通行：当前不处于时间范围内的歌词行
    /// - 热行(hotLines)：当前绝对处于播放时间内的歌词行
    /// - 缓冲行(bufferedLines)：一般处于播放时间后的歌词行，会因为当前播放状态的缘故推迟解除状态
    ///
    /// 关键行为：
    /// - 如果当前仍有缓冲行的情况下加入新热行，则不会解除当前缓冲行，且也不会修改当前滚动位置
    /// - 如果当前所有缓冲行都将被删除且没有新热行加入，则删除所有缓冲行
    /// - 如果当前所有缓冲行都将被删除且有新热行加入，则删除所有缓冲行并加入新热行
    fn update_hot_lines(&mut self, time_ms: f64, lines: &[LyricLineData], is_seek: bool) -> bool {
        let time = time_ms as u64;
        let old_buffered_lines = self.buffered_lines.clone();

        // Step 1: 先检索当前已经超出时间范围的热行，从 hot_lines 中移除
        let mut removed_hot_ids: std::collections::HashSet<usize> =
            std::collections::HashSet::new();
        let hot_lines_snapshot: Vec<usize> = self.hot_lines.iter().copied().collect();
        for last_hot_id in hot_lines_snapshot {
            if let Some(line) = lines.get(last_hot_id) {
                if line.is_bg {
                    continue;
                }
                // 检查是否有背景行
                let next_line = lines.get(last_hot_id + 1);
                if next_line.map(|l| l.is_bg).unwrap_or(false) {
                    // 有背景行的情况
                    let next_main_line = lines.get(last_hot_id + 2);
                    let start_time = line
                        .start_ms
                        .min(next_line.map(|l| l.start_ms).unwrap_or(line.start_ms));
                    let end_time = line
                        .end_ms
                        .max(next_main_line.map(|l| l.start_ms).unwrap_or(u64::MAX))
                        .min(
                            line.end_ms
                                .max(next_line.map(|l| l.end_ms).unwrap_or(line.end_ms)),
                        );

                    if start_time > time || end_time <= time {
                        self.hot_lines.remove(&last_hot_id);
                        removed_hot_ids.insert(last_hot_id);
                        self.hot_lines.remove(&(last_hot_id + 1));
                        removed_hot_ids.insert(last_hot_id + 1);
                    }
                } else if line.start_ms > time || line.end_ms <= time {
                    self.hot_lines.remove(&last_hot_id);
                    removed_hot_ids.insert(last_hot_id);
                }
            } else {
                self.hot_lines.remove(&last_hot_id);
                removed_hot_ids.insert(last_hot_id);
            }
        }

        // Step 2: 找到新的热行（当前时间范围内的行）
        let mut added_ids: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for (i, line) in lines.iter().enumerate() {
            if !line.is_bg && line.start_ms <= time && line.end_ms > time {
                if !self.hot_lines.contains(&i) {
                    self.hot_lines.insert(i);
                    added_ids.insert(i);
                    // 如果下一行是背景行，也加入
                    if let Some(next) = lines.get(i + 1) {
                        if next.is_bg {
                            self.hot_lines.insert(i + 1);
                            added_ids.insert(i + 1);
                        }
                    }
                }
            }
        }

        // Step 3: 计算需要从 buffered_lines 中移除的行
        let removed_ids: std::collections::HashSet<usize> = self
            .buffered_lines
            .iter()
            .filter(|&&idx| !self.hot_lines.contains(&idx))
            .copied()
            .collect();

        // Step 4: 根据逻辑更新 buffered_lines
        if is_seek {
            // Seek 操作：直接同步
            if !self.buffered_lines.is_empty() {
                self.scroll_to_index = *self.buffered_lines.iter().min().unwrap_or(&0);
            } else {
                self.scroll_to_index = lines.iter().position(|l| l.start_ms >= time).unwrap_or(0);
            }
            self.buffered_lines.clear();
            for &v in &self.hot_lines {
                self.buffered_lines.insert(v);
            }
        } else if !removed_ids.is_empty() || !added_ids.is_empty() {
            if removed_ids.is_empty() && !added_ids.is_empty() {
                // 只有新增，没有删除 -> 直接添加到 bufferedLines
                for &v in &added_ids {
                    self.buffered_lines.insert(v);
                }
                self.scroll_to_index = *self.buffered_lines.iter().min().unwrap_or(&0);
            } else if added_ids.is_empty() && !removed_ids.is_empty() {
                // 只有删除，没有新增
                // 关键逻辑：只有当 removedIds 等于 bufferedLines 时才删除
                // 这意味着如果还有其他行在 bufferedLines 中，不会删除任何行
                if removed_ids == self.buffered_lines {
                    for v in self.buffered_lines.clone() {
                        if !self.hot_lines.contains(&v) {
                            self.buffered_lines.remove(&v);
                        }
                    }
                }
                // 注意：这种情况下不更新 scroll_to_index
            } else {
                // 既有新增又有删除
                for &v in &added_ids {
                    self.buffered_lines.insert(v);
                }
                for &v in &removed_ids {
                    self.buffered_lines.remove(&v);
                }
                if !self.buffered_lines.is_empty() {
                    self.scroll_to_index = *self.buffered_lines.iter().min().unwrap_or(&0);
                }
            }
        }

        // 如果 buffered_lines 为空且没有热行，更新 scroll_to_index 到下一行
        if self.buffered_lines.is_empty() && self.hot_lines.is_empty() {
            self.scroll_to_index = self.find_next_line_index(time, lines);
        }

        // Return whether buffered_lines changed (need to recalculate layout/blur)
        // 关键修复：当 buffered_lines 发生任何变化时都需要重新计算布局
        // 这样当第二行开始播放时，即使 scroll_to_index 没变，也会更新模糊级别
        self.buffered_lines != old_buffered_lines
    }

    /// Find the index of the next line that will play after the given time
    /// Used during interludes to determine which line should be "active" (no blur)
    fn find_next_line_index(&self, time: u64, lines: &[LyricLineData]) -> usize {
        // Find the first non-BG line that starts after the current time
        for (i, line) in lines.iter().enumerate() {
            if !line.is_bg && line.start_ms > time {
                return i;
            }
        }
        // If no future line found, find the last line that ended before current time
        // This handles the case where we're past all lyrics
        for (i, line) in lines.iter().enumerate().rev() {
            if !line.is_bg && line.end_ms <= time {
                return i;
            }
        }
        // Fallback to 0
        0
    }

    /// Update interlude dots state
    fn update_interlude(&mut self, time_ms: f64, lines: &[LyricLineData]) {
        let time = time_ms as u64;

        // Check if we're in an interlude (no active lines)
        if !self.buffered_lines.is_empty() {
            self.interlude_dots.set_interlude(None);
            return;
        }

        // Find the interlude range
        let idx = self.scroll_to_index;
        if idx == 0 {
            // Before first line
            if let Some(first) = lines.first() {
                if first.start_ms > time {
                    let duration = first.start_ms - time;
                    if duration >= self.config.interlude_min_duration {
                        self.interlude_dots
                            .set_interlude(Some((time_ms as f32, first.start_ms as f32)));
                        return;
                    }
                }
            }
        } else if let (Some(current), Some(next)) = (lines.get(idx), lines.get(idx + 1)) {
            // Between lines
            if current.end_ms < time && next.start_ms > time {
                let duration = next.start_ms - current.end_ms;
                if duration >= self.config.interlude_min_duration {
                    self.interlude_dots
                        .set_interlude(Some((current.end_ms as f32, next.start_ms as f32)));
                    return;
                }
            }
        }

        self.interlude_dots.set_interlude(None);
    }

    /// Pause playback effects
    pub fn pause(&mut self) {
        self.is_playing = false;
        self.interlude_dots.pause();
    }

    /// Resume playback effects
    pub fn resume(&mut self) {
        self.is_playing = true;
        self.interlude_dots.resume();
    }

    /// Check if playing
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// Get interlude dots state for rendering
    pub fn interlude_dots(&self) -> &InterludeDots {
        &self.interlude_dots
    }

    /// Get buffered line indices
    pub fn buffered_lines(&self) -> &std::collections::HashSet<usize> {
        &self.buffered_lines
    }

    /// Get scroll target index
    pub fn scroll_to_index(&self) -> usize {
        self.scroll_to_index
    }

    /// Get scroll velocity
    #[allow(dead_code)]
    pub fn scroll_velocity(&self) -> f32 {
        self.physics.velocity()
    }

    /// Calculate computed styles for all lines using the lens model
    #[allow(dead_code)]
    pub fn compute_line_styles(
        &self,
        lines: &[LyricLineData],
        viewport_height: f32,
    ) -> Vec<ComputedLineStyle> {
        let mut styles = Vec::with_capacity(lines.len());
        let mut y_position = 0.0;

        // Calculate alignment position (default: 0.35 from top)
        let align_y = viewport_height * self.config.align_position;

        for (idx, line) in lines.iter().enumerate() {
            // Calculate total height for this line
            let line_height = self.config.line_height
                + if line.translated.is_some() {
                    self.config.line_height * self.config.trans_height_ratio
                } else {
                    0.0
                }
                + if line.romanized.is_some() {
                    self.config.line_height * self.config.roman_height_ratio
                } else {
                    0.0
                };

            // Distance from alignment point
            let distance_from_center = y_position - self.scroll_position() - align_y;

            // Use lens model to compute style
            let is_active = self.buffered_lines.contains(&idx);
            let (mut scale, blur) = self.lens.calculate(
                distance_from_center,
                viewport_height,
                self.physics.velocity(),
            );
            let opacity = self
                .lens
                .calculate_opacity(distance_from_center, viewport_height);
            let glow = self
                .lens
                .calculate_glow(distance_from_center, viewport_height, is_active);

            // Apply background line scale if applicable
            if line.is_bg && !is_active {
                scale *= self.config.bg_line_scale;
            }

            // Apply scale effect only if enabled
            if !self.config.enable_scale && !is_active {
                scale = 1.0;
            }

            // Apply hide passed lines
            let final_opacity =
                if self.config.hide_passed_lines && idx < self.scroll_to_index && self.is_playing {
                    0.0001 // Nearly invisible but not zero (for browser optimization)
                } else if is_active {
                    0.85
                } else {
                    opacity
                };

            styles.push(ComputedLineStyle {
                y_position: y_position - self.scroll_position(),
                scale,
                blur: if self.config.enable_blur { blur } else { 0.0 },
                opacity: final_opacity,
                glow,
                is_active,
            });

            y_position += line_height + self.config.line_spacing;
        }

        styles
    }
}
