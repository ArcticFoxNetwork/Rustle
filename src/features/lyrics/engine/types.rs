//! Core data types for the lyrics engine
//!
//! This module contains the fundamental data structures used throughout
//! the lyrics rendering pipeline.
//!
//! ## Features
//!
//! - Per-character animation data for wave effects
//! - Gradient mask parameters for word highlighting
//! - Translation and romanized text support (per-word and per-line)
//! - Virtualization support (isInSight)
//! - Pre-computed mask animation keyframes

use cosmic_text::Weight;

/// Font configuration for lyrics rendering
///
/// 控制字体族、字重和调试日志
/// 确保 TextShaper 和 SdfCache 使用一致的字体
#[derive(Debug, Clone)]
pub struct FontConfig {
    /// Font family name (e.g., "Noto Sans CJK SC")
    /// When None, falls back to system sans-serif
    pub font_family: Option<String>,
    /// Font weight (e.g., Weight::MEDIUM)
    pub font_weight: Weight,
    /// Enable debug logging for font metrics
    pub debug_logging: bool,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            // Use Inter as default font (loaded from assets/fonts/)
            // Falls back to system sans-serif if not found
            font_family: Some("Inter".to_string()),
            // Use NORMAL weight to match Regular font files
            font_weight: Weight::NORMAL,
            debug_logging: false,
        }
    }
}

impl FontConfig {
    /// Create a new FontConfig with a specific font family
    pub fn with_family(family: impl Into<String>) -> Self {
        Self {
            font_family: Some(family.into()),
            ..Default::default()
        }
    }

    /// Set the font weight
    pub fn weight(mut self, weight: Weight) -> Self {
        self.font_weight = weight;
        self
    }

    /// Enable debug logging
    pub fn with_debug(mut self) -> Self {
        self.debug_logging = true;
        self
    }
}

/// Font size configuration for lyrics rendering
///
/// Controls font size calculation parameters including min/max bounds,
/// multiplier, and ratios for translation/romanized text.
#[derive(Debug, Clone)]
pub struct FontSizeConfig {
    /// Minimum font size in logical pixels (default: 36.0)
    pub min_font_size: f32,
    /// Maximum font size in logical pixels (default: 72.0)
    pub max_font_size: f32,
    /// Font size multiplier (default: 1.0)
    pub font_size_multiplier: f32,
    /// Translation text size ratio relative to main text (default: 0.55)
    pub translation_ratio: f32,
    /// Romanized text size ratio relative to main text (default: 0.45)
    pub romanized_ratio: f32,
}

impl Default for FontSizeConfig {
    fn default() -> Self {
        Self {
            min_font_size: 48.0,
            max_font_size: 96.0,
            font_size_multiplier: 1.5,
            translation_ratio: 0.55,
            romanized_ratio: 0.45,
        }
    }
}

impl FontSizeConfig {
    /// Create a new FontSizeConfig with custom min/max font sizes
    pub fn with_bounds(min: f32, max: f32) -> Self {
        Self {
            min_font_size: min,
            max_font_size: max,
            ..Default::default()
        }
    }

    /// Set the font size multiplier
    pub fn multiplier(mut self, multiplier: f32) -> Self {
        self.font_size_multiplier = if multiplier > 0.0 { multiplier } else { 1.0 };
        self
    }

    /// Set the translation text ratio
    pub fn translation_ratio(mut self, ratio: f32) -> Self {
        self.translation_ratio = ratio.clamp(0.3, 0.8);
        self
    }

    /// Set the romanized text ratio
    pub fn romanized_ratio(mut self, ratio: f32) -> Self {
        self.romanized_ratio = ratio.clamp(0.3, 0.8);
        self
    }

    /// Calculate the main font size for a given viewport height
    pub fn calculate_font_size(&self, viewport_height: f32) -> f32 {
        let base_size = viewport_height * 0.055;
        base_size.clamp(self.min_font_size, self.max_font_size) * self.font_size_multiplier
    }

    /// Calculate the translation font size
    pub fn calculate_translation_size(&self, main_font_size: f32) -> f32 {
        main_font_size * self.translation_ratio
    }

    /// Calculate the romanized font size
    pub fn calculate_romanized_size(&self, main_font_size: f32) -> f32 {
        main_font_size * self.romanized_ratio
    }
}

/// Data for a single lyric line
#[derive(Debug, Clone)]
pub struct LyricLineData {
    /// Text content
    pub text: String,
    /// Words for word-by-word highlighting
    pub words: Vec<WordData>,
    /// Translation text (optional, line-level)
    pub translated: Option<String>,
    /// Romanized text (optional, line-level)
    pub romanized: Option<String>,
    /// Start time in milliseconds
    pub start_ms: u64,
    /// End time in milliseconds
    pub end_ms: u64,
    /// Whether this is a duet line (right-aligned)
    pub is_duet: bool,
    /// Whether this is a background vocal
    pub is_bg: bool,
    /// Pre-computed mask animation data (calculated once when lyrics are set)
    pub mask_animation: Option<LineMaskAnimation>,
}

impl LyricLineData {
    /// Check if this line is within the visible viewport (the isInSight)
    ///
    /// Parameters:
    /// - y_position: Current Y position of the line (from spring animation)
    /// - line_height: Height of this line
    /// - viewport_height: Total viewport height
    /// - overscan_px: Extra buffer distance for pre-rendering
    ///
    /// Returns true if the line should be rendered
    pub fn is_in_sight(
        y_position: f32,
        line_height: f32,
        viewport_height: f32,
        overscan_px: f32,
    ) -> bool {
        let top = y_position;
        let bottom = top + line_height;
        // Line is in sight if it's within viewport + overscan buffer
        !(top > viewport_height + line_height + overscan_px || bottom < -line_height - overscan_px)
    }
}

impl Default for LyricLineData {
    fn default() -> Self {
        Self {
            text: String::new(),
            words: Vec::new(),
            translated: None,
            romanized: None,
            start_ms: 0,
            end_ms: 0,
            is_duet: false,
            is_bg: false,
            mask_animation: None,
        }
    }
}

impl LyricLineData {
    /// Calculate and cache mask animation data for this line
    /// Call this after setting words and before rendering
    pub fn compute_mask_animation(&mut self) {
        self.mask_animation = Some(LineMaskAnimation::compute(
            &self.words,
            self.start_ms,
            self.end_ms,
        ));
    }

    /// 获取总淡入时长（用于 mask 动画）
    /// 从行开始到最后一个词结束的时间
    pub fn total_fade_duration(&self) -> u64 {
        let word_end = self
            .words
            .iter()
            .map(|w| w.end_ms)
            .max()
            .unwrap_or(self.end_ms);
        word_end.max(self.end_ms).saturating_sub(self.start_ms)
    }
}

/// Pre-computed mask animation data for a line
///
/// 预计算每个词的 mask 位置动画关键帧
/// 避免实时计算，确保平滑准确的动画
#[derive(Debug, Clone)]
pub struct LineMaskAnimation {
    /// Total duration of the fade animation (ms)
    pub total_duration: f32,
    /// Per-word animation keyframes
    pub word_keyframes: Vec<WordMaskKeyframes>,
}

impl LineMaskAnimation {
    /// Compute mask animation data from words
    pub fn compute(words: &[WordData], line_start_ms: u64, line_end_ms: u64) -> Self {
        // Total fade duration: max of all word end times or line end
        let total_duration = words
            .iter()
            .map(|w| w.end_ms)
            .max()
            .unwrap_or(line_end_ms)
            .max(line_end_ms)
            .saturating_sub(line_start_ms) as f32;

        let word_keyframes = words
            .iter()
            .enumerate()
            .map(|(i, word)| {
                WordMaskKeyframes::compute(i, word, words, line_start_ms, total_duration)
            })
            .collect();

        Self {
            total_duration,
            word_keyframes,
        }
    }
}

/// Pre-computed keyframes for a single word's mask animation
///
/// Generates keyframes that account for:
/// - Static periods (gaps between words)
/// - Movement periods (during word playback)
/// - Proper clamping at boundaries
#[derive(Debug, Clone)]
pub struct WordMaskKeyframes {
    /// Keyframes: (time_offset 0-1, mask_position in pixels)
    pub frames: Vec<MaskKeyframe>,
}

/// A single keyframe in the mask animation
#[derive(Debug, Clone, Copy)]
pub struct MaskKeyframe {
    /// Time offset (0.0 to 1.0, relative to total_duration)
    pub offset: f32,
    /// Mask position in pixels (negative = not yet revealed, 0 = fully revealed)
    pub mask_position: f32,
}

impl WordMaskKeyframes {
    /// Compute keyframes for a word (the generateWebAnimationBasedMaskImage logic)
    fn compute(
        word_idx: usize,
        word: &WordData,
        all_words: &[WordData],
        line_start_ms: u64,
        total_duration: f32,
    ) -> Self {
        if total_duration <= 0.0 {
            return Self { frames: vec![] };
        }

        let word_width = word.x_end - word.x_start;
        let fade_width = 0.5; // In em units, will be converted to pixels during rendering

        // Calculate width before this word (for initial mask position)
        let width_before: f32 = all_words[..word_idx]
            .iter()
            .map(|w| w.x_end - w.x_start)
            .sum();

        // Initial mask position (fully hidden)
        // min_offset is the leftmost position (fully hidden)
        let min_offset = -(word_width + fade_width);

        // Current position starts at the left edge
        let mut cur_pos = -width_before - word_width - fade_width;
        let mut time_offset = 0.0f32;
        let mut frames = Vec::new();

        // Helper to clamp mask position
        let clamp_offset = |x: f32| x.max(min_offset).min(0.0);

        // Initial frame
        frames.push(MaskKeyframe {
            offset: 0.0,
            mask_position: clamp_offset(cur_pos),
        });

        let mut last_timestamp = 0u64;

        // Generate keyframes for each word's timing
        for (j, other_word) in all_words.iter().enumerate() {
            // Static period (gap before this word)
            let cur_timestamp = other_word.start_ms.saturating_sub(line_start_ms);
            let static_duration = cur_timestamp.saturating_sub(last_timestamp);

            if static_duration > 0 {
                time_offset += static_duration as f32 / total_duration;
                let time = time_offset.clamp(0.0, 1.0);
                frames.push(MaskKeyframe {
                    offset: time,
                    mask_position: clamp_offset(cur_pos),
                });
            }
            last_timestamp = cur_timestamp;

            // Movement period (during word playback)
            let fade_duration = other_word.end_ms.saturating_sub(other_word.start_ms);
            if fade_duration > 0 {
                time_offset += fade_duration as f32 / total_duration;
                let other_width = other_word.x_end - other_word.x_start;
                cur_pos += other_width;

                // Add extra fade width at boundaries
                if j == 0 {
                    cur_pos += fade_width * 1.5;
                }
                if j == all_words.len() - 1 {
                    cur_pos += fade_width * 0.5;
                }

                let time = time_offset.clamp(0.0, 1.0);
                frames.push(MaskKeyframe {
                    offset: time,
                    mask_position: clamp_offset(cur_pos),
                });

                last_timestamp += fade_duration;
            }
        }

        Self { frames }
    }

    /// Interpolate mask position at a given time (0-1)
    pub fn interpolate(&self, time: f32) -> f32 {
        if self.frames.is_empty() {
            return 0.0;
        }

        // Find surrounding keyframes
        let mut prev = &self.frames[0];
        for frame in &self.frames {
            if frame.offset > time {
                // Interpolate between prev and frame
                let t = if frame.offset > prev.offset {
                    (time - prev.offset) / (frame.offset - prev.offset)
                } else {
                    0.0
                };
                return prev.mask_position + t * (frame.mask_position - prev.mask_position);
            }
            prev = frame;
        }

        // Past the last keyframe
        prev.mask_position
    }
}

/// Data for a single word within a lyric line
#[derive(Debug, Clone)]
pub struct WordData {
    /// Word text
    pub text: String,
    /// Start time in milliseconds
    pub start_ms: u64,
    /// End time in milliseconds
    pub end_ms: u64,
    /// 逐词罗马音（romanWord）
    /// 强调时显示在词下方
    pub roman_word: Option<String>,
    /// 是否强调（发光效果）
    /// 由 chunk_and_split_words 根据时长和长度计算
    pub emphasize: bool,
    /// X position start (normalized 0-1, calculated during layout)
    pub x_start: f32,
    /// X position end (normalized 0-1, calculated during layout)
    pub x_end: f32,
    /// Whether this is the last word in the line (for emphasis boost)
    /// Applies 1.6x amount, 1.5x blur, 1.2x duration for last word
    pub is_last_word: bool,
}

#[allow(dead_code)]
impl WordData {
    /// Check if this word should have emphasis effect (style)
    ///
    /// the emphasis criteria:
    /// - Word duration >= 1000ms
    /// - For non-CJK: word length <= 7 and > 1
    /// - For CJK: only duration check
    pub fn should_emphasize(&self) -> bool {
        let duration = self.end_ms.saturating_sub(self.start_ms);
        if duration < 1000 {
            return false;
        }

        let trimmed = self.text.trim();
        if is_cjk_text(trimmed) {
            // CJK text: only check duration
            true
        } else {
            // Non-CJK: check length
            let len = trimmed.len();
            len > 1 && len <= 7
        }
    }

    /// Get word duration in milliseconds
    pub fn duration_ms(&self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }

    /// Calculate per-character animation delay (style)
    ///
    /// Formula: wordDe = de + (du / 2.5 / arr.length) * i
    /// where:
    /// - de: base delay (word start time - line start time)
    /// - du: word duration
    /// - arr.length: character count
    /// - i: character index
    pub fn char_delay(&self, char_index: usize, line_start_ms: u64) -> f32 {
        let de = self.start_ms.saturating_sub(line_start_ms) as f32;
        let du = self.duration_ms() as f32;
        let char_count = self.text.chars().count().max(1) as f32;
        de + (du / 2.5 / char_count) * char_index as f32
    }

    /// Calculate per-character X offset for wave effect (style)
    ///
    /// Formula: offsetX = -transX * 0.03 * amount * (arr.length / 2 - i)
    /// where:
    /// - transX: emphasis easing value (0-1)
    /// - amount: emphasis amount based on duration
    /// - arr.length: character count
    /// - i: character index
    ///
    /// Returns offset in em units
    pub fn char_x_offset(&self, char_index: usize, emphasis_progress: f32) -> f32 {
        let char_count = self.text.chars().count().max(1) as f32;
        let amount = self.emphasis_amount();
        let emp = emphasis_easing(emphasis_progress);
        -emp * 0.03 * amount * (char_count / 2.0 - char_index as f32)
    }

    /// Calculate emphasis amount based on word duration
    ///
    /// Formula:
    /// - amount = du / 2000
    /// - if amount > 1: amount = sqrt(amount)
    /// - else: amount = amount^3
    /// - amount *= 0.6
    /// - For last word: amount *= 1.6
    /// - amount = min(1.2, amount)
    pub fn emphasis_amount(&self) -> f32 {
        // Uses original duration for amount calculation, not effective_duration
        let du = self.duration_ms() as f32;
        let mut amount = du / 2000.0;
        amount = if amount > 1.0 {
            amount.sqrt()
        } else {
            amount.powi(3)
        };
        amount *= 0.6;
        // default: Last word gets 1.6x emphasis boost
        if self.is_last_word {
            amount *= 1.6;
        }
        amount.min(1.2)
    }

    /// Calculate blur amount for emphasis (style)
    ///
    /// Formula:
    /// - blur = du / 3000
    /// - if blur > 1: blur = sqrt(blur)
    /// - else: blur = blur^3
    /// - blur *= 0.5
    /// - For last word: blur *= 1.5
    /// - blur = min(0.8, blur)
    pub fn emphasis_blur(&self) -> f32 {
        // Uses original duration for blur calculation, not effective_duration
        let du = self.duration_ms() as f32;
        let mut blur = du / 3000.0;
        blur = if blur > 1.0 {
            blur.sqrt()
        } else {
            blur.powi(3)
        };
        blur *= 0.5;
        // default: Last word gets 1.5x blur boost
        if self.is_last_word {
            blur *= 1.5;
        }
        blur.min(0.8)
    }

    /// Get effective duration for emphasis calculations
    /// default: Last word gets 1.2x duration
    pub fn effective_duration_ms(&self) -> u64 {
        let du = self.duration_ms();
        if self.is_last_word {
            (du as f32 * 1.2) as u64
        } else {
            du
        }
    }
}

/// Apple Music-style emphasis easing function
///
/// Uses bezier-easing library with:
/// - bezIn: cubic-bezier(0.2, 0.4, 0.58, 1.0)
/// - bezOut: cubic-bezier(0.3, 0.0, 0.58, 1.0)
///
/// The easing function:
/// - First half (0 to 0.5): bezIn(x / 0.5) - ramps up
/// - Second half (0.5 to 1): 1 - bezOut((x - 0.5) / 0.5) - ramps down
#[allow(dead_code)]
pub fn emphasis_easing(x: f32) -> f32 {
    const EMP_EASING_MID: f32 = 0.5;

    // Normalize functions
    let begin_num = |v: f32| (v / EMP_EASING_MID).clamp(0.0, 1.0);
    let end_num = |v: f32| ((v - EMP_EASING_MID) / (1.0 - EMP_EASING_MID)).clamp(0.0, 1.0);

    if x < EMP_EASING_MID {
        // bezIn: cubic-bezier(0.2, 0.4, 0.58, 1.0)
        cubic_bezier(begin_num(x), 0.2, 0.4, 0.58, 1.0)
    } else {
        // 1 - bezOut: cubic-bezier(0.3, 0.0, 0.58, 1.0)
        1.0 - cubic_bezier(end_num(x), 0.3, 0.0, 0.58, 1.0)
    }
}

/// Approximate cubic-bezier easing
///
/// Parameters: t (0-1), x1, y1, x2, y2 (control points)
/// Returns: y value at parameter t
fn cubic_bezier(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    // For a cubic bezier curve B(t) = (1-t)³P0 + 3(1-t)²tP1 + 3(1-t)t²P2 + t³P3
    // With P0=(0,0), P1=(x1,y1), P2=(x2,y2), P3=(1,1)
    // 根据 x 找到 t，然后返回 y
    //
    // For simplicity, we use Newton-Raphson iteration to find t from x
    // then calculate y

    let t = t.clamp(0.0, 1.0);

    // Calculate x(t) and y(t) for cubic bezier
    let calc_bezier = |t: f32, p1: f32, p2: f32| -> f32 {
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        // B(t) = (1-t)³*0 + 3(1-t)²t*p1 + 3(1-t)t²*p2 + t³*1
        // (1-t)³ term is multiplied by P0=0, so it's omitted
        3.0 * mt2 * t * p1 + 3.0 * mt * t2 * p2 + t3
    };

    // For CSS cubic-bezier, we need to find t such that x(t) = input_t
    // Then return y(t)
    // Use Newton-Raphson to solve for t
    let mut guess_t = t;
    for _ in 0..8 {
        let x_at_t = calc_bezier(guess_t, x1, x2);
        let error = x_at_t - t;
        if error.abs() < 0.0001 {
            break;
        }
        // Derivative of bezier x with respect to t
        let dx = 3.0 * (1.0 - guess_t).powi(2) * x1
            + 6.0 * (1.0 - guess_t) * guess_t * (x2 - x1)
            + 3.0 * guess_t.powi(2) * (1.0 - x2);
        if dx.abs() < 0.0001 {
            break;
        }
        guess_t -= error / dx;
        guess_t = guess_t.clamp(0.0, 1.0);
    }

    calc_bezier(guess_t, y1, y2)
}

/// Check if text is entirely CJK characters (the isCJK)
///
/// Regex: /^[\p{Unified_Ideograph}\u0800-\u9FFC]+$/u
/// This checks if the ENTIRE string consists of CJK characters only.
///
/// The regex is quite broad. The \u0800-\u9FFC range includes:
/// - \u0800-\u0FFF: Various scripts (Samaritan, Mandaic, Syriac, etc.)
/// - \u1000-\u109F: Myanmar
/// - \u3000-\u303F: CJK Symbols and Punctuation
/// - \u3040-\u309F: Hiragana
/// - \u30A0-\u30FF: Katakana
/// - \u3100-\u312F: Bopomofo
/// - \u3130-\u318F: Hangul Compatibility Jamo
/// - \u4E00-\u9FFF: CJK Unified Ideographs
///
/// For practical purposes, we focus on the most common CJK ranges.
pub fn is_cjk_text(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    text.chars().all(|c| {
        // Match the regex: Unified_Ideograph property + \u0800-\u9FFC range
        // We use the broad range to match the behavior exactly
        matches!(c,
            '\u{0800}'..='\u{9FFC}' |  // the explicit range (very broad)
            '\u{AC00}'..='\u{D7AF}'    // Hangul Syllables (outside the range but commonly needed)
        )
    })
}

/// Check if text contains any CJK characters
#[allow(dead_code)]
fn contains_cjk(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c,
            '\u{4E00}'..='\u{9FFF}' |  // CJK Unified Ideographs
            '\u{3400}'..='\u{4DBF}' |  // CJK Unified Ideographs Extension A
            '\u{20000}'..='\u{2A6DF}' | // CJK Unified Ideographs Extension B
            '\u{2A700}'..='\u{2B73F}' | // CJK Unified Ideographs Extension C
            '\u{2B740}'..='\u{2B81F}' | // CJK Unified Ideographs Extension D
            '\u{2B820}'..='\u{2CEAF}' | // CJK Unified Ideographs Extension E
            '\u{F900}'..='\u{FAFF}' |   // CJK Compatibility Ideographs
            '\u{2F800}'..='\u{2FA1F}' | // CJK Compatibility Ideographs Supplement
            '\u{3000}'..='\u{303F}' |   // CJK Symbols and Punctuation
            '\u{3040}'..='\u{309F}' |   // Hiragana
            '\u{30A0}'..='\u{30FF}' |   // Katakana
            '\u{AC00}'..='\u{D7AF}'     // Hangul Syllables
        )
    })
}

impl Default for WordData {
    fn default() -> Self {
        Self {
            text: String::new(),
            start_ms: 0,
            end_ms: 0,
            roman_word: None,
            emphasize: false,
            x_start: 0.0,
            x_end: 0.0,
            is_last_word: false,
        }
    }
}

/// Computed visual style for a lyric line
///
/// These values are calculated based on scroll position, active state,
/// and distance from the alignment point.
#[derive(Debug, Clone)]
pub struct ComputedLineStyle {
    /// Y position in pixels (after scroll offset)
    pub y_position: f32,
    /// Scale factor (1.0 = normal, 0.97 for inactive)
    pub scale: f32,
    /// Blur amount (0.0 = none)
    pub blur: f32,
    /// Opacity (0.0 = transparent, 1.0 = opaque)
    pub opacity: f32,
    /// Glow intensity for active lines
    pub glow: f32,
    /// Whether this line is currently active (being sung)
    pub is_active: bool,
}

impl Default for ComputedLineStyle {
    fn default() -> Self {
        Self {
            y_position: 0.0,
            scale: 1.0,
            blur: 0.0,
            opacity: 1.0,
            glow: 0.0,
            is_active: false,
        }
    }
}

// ========== Mask Alpha Calculation ==========

/// Calculate bright mask alpha based on scale
///
/// Formula: `clamp((scale - 0.97) / 0.03, 0, 1) * 0.8 + 0.2`
///
/// This controls the brightness of the highlighted (played) portion of lyrics.
/// - At scale 1.0 (active line): bright_alpha = 1.0
/// - At scale 0.97 or below (inactive line): bright_alpha = 0.2
pub fn calculate_bright_mask_alpha(scale: f32) -> f32 {
    let normalized = ((scale - 0.97) / 0.03).clamp(0.0, 1.0);
    normalized * 0.8 + 0.2
}

/// Calculate dark mask alpha based on scale
///
/// Formula: `clamp((scale - 0.97) / 0.03, 0, 1) * 0.2 + 0.2`
///
/// This controls the brightness of the unhighlighted (unplayed) portion of lyrics.
/// - At scale 1.0 (active line): dark_alpha = 0.4
/// - At scale 0.97 or below (inactive line): dark_alpha = 0.2
pub fn calculate_dark_mask_alpha(scale: f32) -> f32 {
    let normalized = ((scale - 0.97) / 0.03).clamp(0.0, 1.0);
    normalized * 0.2 + 0.2
}

/// Interpolate between dark and bright alpha based on highlight progress
///
/// Uses CSS linear-gradient with mask-position animation.
/// This function provides the equivalent brightness calculation.
///
/// Parameters:
/// - scale: Line scale (0.97 for inactive, 1.0 for active)
/// - highlight: Highlight progress (0.0 = unplayed, 1.0 = fully played)
///
/// Returns: Final brightness value (0.0 - 1.0)
pub fn interpolate_brightness(scale: f32, highlight: f32) -> f32 {
    let bright_alpha = calculate_bright_mask_alpha(scale);
    let dark_alpha = calculate_dark_mask_alpha(scale);
    // mix(dark_alpha, bright_alpha, highlight) = dark_alpha * (1 - highlight) + bright_alpha * highlight
    dark_alpha * (1.0 - highlight) + bright_alpha * highlight
}

// ========== Highlight Glow Calculation (Apple Music-style) ==========

/// Check if highlight glow should be applied (Apple Music-style)
///
/// Triggers highlight glow when:
/// - highlight progress > 0.3
/// - line is active
///
/// Parameters:
/// - highlight: Highlight progress (0.0 = unplayed, 1.0 = fully played)
/// - is_active: Whether the line is currently active
///
/// Returns: true if highlight glow should be applied
pub fn should_apply_highlight_glow(highlight: f32, is_active: bool) -> bool {
    highlight > 0.3 && is_active
}

/// Calculate highlight glow strength (Apple Music-style)
///
/// Formula: `glow_strength = (highlight - 0.3) / 0.7`
///
/// Parameters:
/// - highlight: Highlight progress (0.0 - 1.0)
///
/// Returns: Glow strength (0.0 - 1.0), or 0.0 if highlight <= 0.3
pub fn calculate_highlight_glow_strength(highlight: f32) -> f32 {
    if highlight <= 0.3 {
        0.0
    } else {
        (highlight - 0.3) / 0.7
    }
}

/// Calculate highlight glow color addition (Apple Music-style)
///
/// Adds `(0.15, 0.15, 0.2) * glow_strength * 0.5` to the base color
///
/// Parameters:
/// - glow_strength: Glow strength from calculate_highlight_glow_strength
///
/// Returns: (r, g, b) color values to add to base color
pub fn calculate_highlight_glow_color(glow_strength: f32) -> (f32, f32, f32) {
    let factor = glow_strength * 0.5;
    (0.15 * factor, 0.15 * factor, 0.2 * factor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_config_default() {
        let config = FontConfig::default();
        assert_eq!(config.font_family, Some("Inter".to_string()));
        assert_eq!(config.font_weight, Weight::NORMAL);
        assert!(!config.debug_logging);
    }

    #[test]
    fn test_font_config_with_family() {
        let config = FontConfig::with_family("Noto Sans CJK SC");
        assert_eq!(config.font_family, Some("Noto Sans CJK SC".to_string()));
        assert_eq!(config.font_weight, Weight::NORMAL); // Default weight
        assert!(!config.debug_logging);
    }

    #[test]
    fn test_font_config_builder() {
        let config = FontConfig::with_family("Arial")
            .weight(Weight::BOLD)
            .with_debug();
        assert_eq!(config.font_family, Some("Arial".to_string()));
        assert_eq!(config.font_weight, Weight::BOLD);
        assert!(config.debug_logging);
    }

    #[test]
    fn test_font_config_clone() {
        let config1 = FontConfig::with_family("Test Font").with_debug();
        let config2 = config1.clone();
        assert_eq!(config1.font_family, config2.font_family);
        assert_eq!(config1.font_weight, config2.font_weight);
        assert_eq!(config1.debug_logging, config2.debug_logging);
    }

    // ========== Property 5: Emphasis amount calculation ==========

    fn create_word_data(start_ms: u64, end_ms: u64, is_last_word: bool) -> WordData {
        WordData {
            text: "test".to_string(),
            start_ms,
            end_ms,
            roman_word: None,
            emphasize: true,
            x_start: 0.0,
            x_end: 1.0,
            is_last_word,
        }
    }

    #[test]
    fn test_emphasis_amount_short_duration() {
        // duration = 1000ms, amount = (1000/2000)^3 * 0.6 = 0.5^3 * 0.6 = 0.075
        let word = create_word_data(0, 1000, false);
        let amount = word.emphasis_amount();
        assert!(
            (amount - 0.075).abs() < 0.001,
            "Expected 0.075, got {}",
            amount
        );
    }

    #[test]
    fn test_emphasis_amount_medium_duration() {
        // duration = 2000ms, amount = (2000/2000)^3 * 0.6 = 1^3 * 0.6 = 0.6
        let word = create_word_data(0, 2000, false);
        let amount = word.emphasis_amount();
        assert!((amount - 0.6).abs() < 0.001, "Expected 0.6, got {}", amount);
    }

    #[test]
    fn test_emphasis_amount_long_duration() {
        // duration = 4000ms, amount = sqrt(4000/2000) * 0.6 = sqrt(2) * 0.6 ≈ 0.849
        let word = create_word_data(0, 4000, false);
        let amount = word.emphasis_amount();
        let expected = 2.0_f32.sqrt() * 0.6;
        assert!(
            (amount - expected).abs() < 0.001,
            "Expected {}, got {}",
            expected,
            amount
        );
    }

    #[test]
    fn test_emphasis_amount_last_word_multiplier() {
        // duration = 2000ms, amount = 1^3 * 0.6 * 1.6 = 0.96
        let word = create_word_data(0, 2000, true);
        let amount = word.emphasis_amount();
        assert!(
            (amount - 0.96).abs() < 0.001,
            "Expected 0.96, got {}",
            amount
        );
    }

    #[test]
    fn test_emphasis_amount_capped_at_1_2() {
        // Very long duration should be capped at 1.2
        let word = create_word_data(0, 10000, true);
        let amount = word.emphasis_amount();
        assert!(
            amount <= 1.2,
            "Amount should be capped at 1.2, got {}",
            amount
        );
    }

    // ========== Property 6: Emphasis blur calculation ==========

    #[test]
    fn test_emphasis_blur_short_duration() {
        // duration = 1500ms, blur = (1500/3000)^3 * 0.5 = 0.5^3 * 0.5 = 0.0625
        let word = create_word_data(0, 1500, false);
        let blur = word.emphasis_blur();
        assert!(
            (blur - 0.0625).abs() < 0.001,
            "Expected 0.0625, got {}",
            blur
        );
    }

    #[test]
    fn test_emphasis_blur_medium_duration() {
        // duration = 3000ms, blur = (3000/3000)^3 * 0.5 = 1^3 * 0.5 = 0.5
        let word = create_word_data(0, 3000, false);
        let blur = word.emphasis_blur();
        assert!((blur - 0.5).abs() < 0.001, "Expected 0.5, got {}", blur);
    }

    #[test]
    fn test_emphasis_blur_long_duration() {
        // duration = 6000ms, blur = sqrt(6000/3000) * 0.5 = sqrt(2) * 0.5 ≈ 0.707
        let word = create_word_data(0, 6000, false);
        let blur = word.emphasis_blur();
        let expected = 2.0_f32.sqrt() * 0.5;
        assert!(
            (blur - expected).abs() < 0.001,
            "Expected {}, got {}",
            expected,
            blur
        );
    }

    #[test]
    fn test_emphasis_blur_last_word_multiplier() {
        // duration = 3000ms, blur = 1^3 * 0.5 * 1.5 = 0.75
        let word = create_word_data(0, 3000, true);
        let blur = word.emphasis_blur();
        assert!((blur - 0.75).abs() < 0.001, "Expected 0.75, got {}", blur);
    }

    #[test]
    fn test_emphasis_blur_capped_at_0_8() {
        // Very long duration should be capped at 0.8
        let word = create_word_data(0, 10000, true);
        let blur = word.emphasis_blur();
        assert!(blur <= 0.8, "Blur should be capped at 0.8, got {}", blur);
    }

    // ========== Property 7: Mask alpha calculation ==========
    // **Feature: amll-blur-glow-effects, Property 7: Mask alpha calculation**
    // **Validates: Requirements 3.1, 3.2**

    #[test]
    fn test_bright_mask_alpha_at_full_scale() {
        // At scale 1.0: normalized = (1.0 - 0.97) / 0.03 = 1.0
        // bright_alpha = 1.0 * 0.8 + 0.2 = 1.0
        let alpha = super::calculate_bright_mask_alpha(1.0);
        assert!((alpha - 1.0).abs() < 0.001, "Expected 1.0, got {}", alpha);
    }

    #[test]
    fn test_bright_mask_alpha_at_inactive_scale() {
        // At scale 0.97: normalized = (0.97 - 0.97) / 0.03 = 0.0
        // bright_alpha = 0.0 * 0.8 + 0.2 = 0.2
        let alpha = super::calculate_bright_mask_alpha(0.97);
        assert!((alpha - 0.2).abs() < 0.001, "Expected 0.2, got {}", alpha);
    }

    #[test]
    fn test_bright_mask_alpha_below_threshold() {
        // At scale 0.95: normalized = clamp((0.95 - 0.97) / 0.03, 0, 1) = 0.0
        // bright_alpha = 0.0 * 0.8 + 0.2 = 0.2
        let alpha = super::calculate_bright_mask_alpha(0.95);
        assert!((alpha - 0.2).abs() < 0.001, "Expected 0.2, got {}", alpha);
    }

    #[test]
    fn test_bright_mask_alpha_midpoint() {
        // At scale 0.985: normalized = (0.985 - 0.97) / 0.03 = 0.5
        // bright_alpha = 0.5 * 0.8 + 0.2 = 0.6
        let alpha = super::calculate_bright_mask_alpha(0.985);
        assert!((alpha - 0.6).abs() < 0.001, "Expected 0.6, got {}", alpha);
    }

    #[test]
    fn test_dark_mask_alpha_at_full_scale() {
        // At scale 1.0: normalized = 1.0
        // dark_alpha = 1.0 * 0.2 + 0.2 = 0.4
        let alpha = super::calculate_dark_mask_alpha(1.0);
        assert!((alpha - 0.4).abs() < 0.001, "Expected 0.4, got {}", alpha);
    }

    #[test]
    fn test_dark_mask_alpha_at_inactive_scale() {
        // At scale 0.97: normalized = 0.0
        // dark_alpha = 0.0 * 0.2 + 0.2 = 0.2
        let alpha = super::calculate_dark_mask_alpha(0.97);
        assert!((alpha - 0.2).abs() < 0.001, "Expected 0.2, got {}", alpha);
    }

    #[test]
    fn test_dark_mask_alpha_below_threshold() {
        // At scale 0.90: normalized = clamp((0.90 - 0.97) / 0.03, 0, 1) = 0.0
        // dark_alpha = 0.0 * 0.2 + 0.2 = 0.2
        let alpha = super::calculate_dark_mask_alpha(0.90);
        assert!((alpha - 0.2).abs() < 0.001, "Expected 0.2, got {}", alpha);
    }

    #[test]
    fn test_dark_mask_alpha_midpoint() {
        // At scale 0.985: normalized = 0.5
        // dark_alpha = 0.5 * 0.2 + 0.2 = 0.3
        let alpha = super::calculate_dark_mask_alpha(0.985);
        assert!((alpha - 0.3).abs() < 0.001, "Expected 0.3, got {}", alpha);
    }

    // ========== Property 8: Brightness interpolation ==========
    // **Feature: amll-blur-glow-effects, Property 8: Brightness interpolation**
    // **Validates: Requirements 3.3**

    #[test]
    fn test_brightness_interpolation_at_zero_highlight() {
        // At highlight = 0: brightness = dark_alpha
        // scale = 1.0: dark_alpha = 0.4
        let brightness = super::interpolate_brightness(1.0, 0.0);
        assert!(
            (brightness - 0.4).abs() < 0.001,
            "Expected 0.4, got {}",
            brightness
        );
    }

    #[test]
    fn test_brightness_interpolation_at_full_highlight() {
        // At highlight = 1: brightness = bright_alpha
        // scale = 1.0: bright_alpha = 1.0
        let brightness = super::interpolate_brightness(1.0, 1.0);
        assert!(
            (brightness - 1.0).abs() < 0.001,
            "Expected 1.0, got {}",
            brightness
        );
    }

    #[test]
    fn test_brightness_interpolation_at_half_highlight() {
        // At highlight = 0.5: brightness = mix(dark_alpha, bright_alpha, 0.5)
        // scale = 1.0: dark_alpha = 0.4, bright_alpha = 1.0
        // brightness = 0.4 * 0.5 + 1.0 * 0.5 = 0.7
        let brightness = super::interpolate_brightness(1.0, 0.5);
        assert!(
            (brightness - 0.7).abs() < 0.001,
            "Expected 0.7, got {}",
            brightness
        );
    }

    #[test]
    fn test_brightness_interpolation_inactive_line() {
        // At scale = 0.97 (inactive): dark_alpha = 0.2, bright_alpha = 0.2
        // Any highlight value should give 0.2
        let brightness_0 = super::interpolate_brightness(0.97, 0.0);
        let brightness_1 = super::interpolate_brightness(0.97, 1.0);
        let brightness_half = super::interpolate_brightness(0.97, 0.5);

        assert!(
            (brightness_0 - 0.2).abs() < 0.001,
            "Expected 0.2, got {}",
            brightness_0
        );
        assert!(
            (brightness_1 - 0.2).abs() < 0.001,
            "Expected 0.2, got {}",
            brightness_1
        );
        assert!(
            (brightness_half - 0.2).abs() < 0.001,
            "Expected 0.2, got {}",
            brightness_half
        );
    }

    #[test]
    fn test_brightness_interpolation_partial_scale() {
        // At scale = 0.985 (midpoint): dark_alpha = 0.3, bright_alpha = 0.6
        // At highlight = 0.5: brightness = 0.3 * 0.5 + 0.6 * 0.5 = 0.45
        let brightness = super::interpolate_brightness(0.985, 0.5);
        assert!(
            (brightness - 0.45).abs() < 0.001,
            "Expected 0.45, got {}",
            brightness
        );
    }

    // ========== Property 9: Highlight glow trigger and calculation ==========
    // **Feature: amll-blur-glow-effects, Property 9: Highlight glow trigger and calculation**
    // **Validates: Requirements 5.1, 5.2, 5.3, 5.4**

    #[test]
    fn test_highlight_glow_trigger_active_above_threshold() {
        // highlight > 0.3 and is_active = true -> should apply glow
        assert!(super::should_apply_highlight_glow(0.31, true));
        assert!(super::should_apply_highlight_glow(0.5, true));
        assert!(super::should_apply_highlight_glow(1.0, true));
    }

    #[test]
    fn test_highlight_glow_trigger_active_at_threshold() {
        // highlight = 0.3 exactly -> should NOT apply glow (> not >=)
        assert!(!super::should_apply_highlight_glow(0.3, true));
    }

    #[test]
    fn test_highlight_glow_trigger_active_below_threshold() {
        // highlight <= 0.3 and is_active = true -> should NOT apply glow
        assert!(!super::should_apply_highlight_glow(0.0, true));
        assert!(!super::should_apply_highlight_glow(0.1, true));
        assert!(!super::should_apply_highlight_glow(0.29, true));
    }

    #[test]
    fn test_highlight_glow_trigger_inactive() {
        // is_active = false -> should NOT apply glow regardless of highlight
        assert!(!super::should_apply_highlight_glow(0.0, false));
        assert!(!super::should_apply_highlight_glow(0.5, false));
        assert!(!super::should_apply_highlight_glow(1.0, false));
    }

    #[test]
    fn test_highlight_glow_strength_at_threshold() {
        // At highlight = 0.3: glow_strength = 0
        let strength = super::calculate_highlight_glow_strength(0.3);
        assert!(
            (strength - 0.0).abs() < 0.001,
            "Expected 0.0, got {}",
            strength
        );
    }

    #[test]
    fn test_highlight_glow_strength_just_above_threshold() {
        // At highlight = 0.31: glow_strength = (0.31 - 0.3) / 0.7 ≈ 0.0143
        let strength = super::calculate_highlight_glow_strength(0.31);
        let expected = (0.31 - 0.3) / 0.7;
        assert!(
            (strength - expected).abs() < 0.001,
            "Expected {}, got {}",
            expected,
            strength
        );
    }

    #[test]
    fn test_highlight_glow_strength_at_midpoint() {
        // At highlight = 0.65: glow_strength = (0.65 - 0.3) / 0.7 = 0.5
        let strength = super::calculate_highlight_glow_strength(0.65);
        assert!(
            (strength - 0.5).abs() < 0.001,
            "Expected 0.5, got {}",
            strength
        );
    }

    #[test]
    fn test_highlight_glow_strength_at_full() {
        // At highlight = 1.0: glow_strength = (1.0 - 0.3) / 0.7 = 1.0
        let strength = super::calculate_highlight_glow_strength(1.0);
        assert!(
            (strength - 1.0).abs() < 0.001,
            "Expected 1.0, got {}",
            strength
        );
    }

    #[test]
    fn test_highlight_glow_strength_below_threshold() {
        // At highlight < 0.3: glow_strength = 0
        assert_eq!(super::calculate_highlight_glow_strength(0.0), 0.0);
        assert_eq!(super::calculate_highlight_glow_strength(0.1), 0.0);
        assert_eq!(super::calculate_highlight_glow_strength(0.29), 0.0);
    }

    #[test]
    fn test_highlight_glow_color_at_zero_strength() {
        // At glow_strength = 0: color = (0, 0, 0)
        let (r, g, b) = super::calculate_highlight_glow_color(0.0);
        assert!((r - 0.0).abs() < 0.001);
        assert!((g - 0.0).abs() < 0.001);
        assert!((b - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_highlight_glow_color_at_full_strength() {
        // At glow_strength = 1.0: color = (0.15, 0.15, 0.2) * 0.5 = (0.075, 0.075, 0.1)
        let (r, g, b) = super::calculate_highlight_glow_color(1.0);
        assert!((r - 0.075).abs() < 0.001, "Expected r=0.075, got {}", r);
        assert!((g - 0.075).abs() < 0.001, "Expected g=0.075, got {}", g);
        assert!((b - 0.1).abs() < 0.001, "Expected b=0.1, got {}", b);
    }

    #[test]
    fn test_highlight_glow_color_at_half_strength() {
        // At glow_strength = 0.5: color = (0.15, 0.15, 0.2) * 0.5 * 0.5 = (0.0375, 0.0375, 0.05)
        let (r, g, b) = super::calculate_highlight_glow_color(0.5);
        assert!((r - 0.0375).abs() < 0.001, "Expected r=0.0375, got {}", r);
        assert!((g - 0.0375).abs() < 0.001, "Expected g=0.0375, got {}", g);
        assert!((b - 0.05).abs() < 0.001, "Expected b=0.05, got {}", b);
    }

    // ========== FontSizeConfig tests ==========
    // **Feature: lyrics-font-clarity**
    // **Validates: Requirements 1.1, 3.1, 4.1, 4.2**

    #[test]
    fn test_font_size_config_default() {
        let config = super::FontSizeConfig::default();
        assert!((config.min_font_size - 36.0).abs() < 0.001);
        assert!((config.max_font_size - 72.0).abs() < 0.001);
        assert!((config.font_size_multiplier - 1.0).abs() < 0.001);
        assert!((config.translation_ratio - 0.55).abs() < 0.001);
        assert!((config.romanized_ratio - 0.45).abs() < 0.001);
    }

    #[test]
    fn test_font_size_config_with_bounds() {
        let config = super::FontSizeConfig::with_bounds(24.0, 96.0);
        assert!((config.min_font_size - 24.0).abs() < 0.001);
        assert!((config.max_font_size - 96.0).abs() < 0.001);
    }

    #[test]
    fn test_font_size_config_multiplier() {
        let config = super::FontSizeConfig::default().multiplier(1.5);
        assert!((config.font_size_multiplier - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_font_size_config_negative_multiplier_defaults_to_one() {
        let config = super::FontSizeConfig::default().multiplier(-1.0);
        assert!((config.font_size_multiplier - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_font_size_config_calculate_font_size() {
        let config = super::FontSizeConfig::default();
        // viewport_height = 1080, base = 1080 * 0.055 = 59.4
        // clamped to [36, 72] = 59.4, * 1.0 = 59.4
        let size = config.calculate_font_size(1080.0);
        assert!((size - 59.4).abs() < 0.1, "Expected ~59.4, got {}", size);
    }

    #[test]
    fn test_font_size_config_calculate_font_size_min_clamp() {
        let config = super::FontSizeConfig::default();
        // viewport_height = 400, base = 400 * 0.055 = 22
        // clamped to [36, 72] = 36
        let size = config.calculate_font_size(400.0);
        assert!((size - 36.0).abs() < 0.001, "Expected 36.0, got {}", size);
    }

    #[test]
    fn test_font_size_config_calculate_font_size_max_clamp() {
        let config = super::FontSizeConfig::default();
        // viewport_height = 2000, base = 2000 * 0.055 = 110
        // clamped to [36, 72] = 72
        let size = config.calculate_font_size(2000.0);
        assert!((size - 72.0).abs() < 0.001, "Expected 72.0, got {}", size);
    }

    #[test]
    fn test_font_size_config_calculate_translation_size() {
        let config = super::FontSizeConfig::default();
        let main_size = 60.0;
        let trans_size = config.calculate_translation_size(main_size);
        // 60 * 0.55 = 33
        assert!(
            (trans_size - 33.0).abs() < 0.001,
            "Expected 33.0, got {}",
            trans_size
        );
    }

    #[test]
    fn test_font_size_config_calculate_romanized_size() {
        let config = super::FontSizeConfig::default();
        let main_size = 60.0;
        let roman_size = config.calculate_romanized_size(main_size);
        // 60 * 0.45 = 27
        assert!(
            (roman_size - 27.0).abs() < 0.001,
            "Expected 27.0, got {}",
            roman_size
        );
    }
}
