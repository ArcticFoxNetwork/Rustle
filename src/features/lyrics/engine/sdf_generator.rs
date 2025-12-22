//! SDF (Signed Distance Field) Generator
//!
//! 使用 ab_glyph + sdf_glyph_renderer 生成高质量 SDF 位图
//! 纯 Rust 实现，无 C++ 依赖
//!
//! ## 使用方式
//!
//! ```ignore
//! let generator = SdfGenerator::new(72, 4);
//! let bitmap = generator.generate_char(&font_data, 'A')?;
//! ```

use ab_glyph::{Font, FontRef, GlyphId, PxScale, ScaleFont};
use sdf_glyph_renderer::{BitmapGlyph, clamp_to_u8};

/// SDF 生成器配置
#[derive(Debug, Clone, Copy)]
pub struct SdfConfig {
    /// 生成 SDF 时的基准字号（像素）
    pub base_size: u32,
    /// SDF buffer（像素）- 字形周围的边距，用于捕获外部距离
    pub buffer: usize,
    /// SDF radius（像素）- 距离场的有效范围
    pub radius: usize,
    /// clamp_to_u8 的 cutoff 值（0.0-1.0）
    pub cutoff: f64,
}

impl Default for SdfConfig {
    fn default() -> Self {
        Self {
            base_size: 64,
            buffer: 4,
            radius: 8,
            // cutoff 决定 SDF 距离如何映射到 0-255
            // 0.5 表示：距离 = 0（边缘）映射到 128
            // 这与 shader 中的 edge_threshold = 0.5 完美匹配
            cutoff: 0.5,
        }
    }
}

/// SDF 生成结果
#[derive(Debug, Clone)]
pub struct SdfBitmap {
    /// 单通道 SDF 数据（每像素 1 字节）
    pub data: Vec<u8>,
    /// 位图宽度（包含 buffer）
    pub width: u32,
    /// 位图高度（包含 buffer）
    pub height: u32,
    /// 字形相对于原点的水平偏移（考虑 buffer）
    pub bearing_x: i32,
    /// 字形相对于基线的垂直偏移（考虑 buffer）
    pub bearing_y: i32,
    /// 字形的水平前进宽度
    pub advance: f32,
}

/// SDF 生成器
pub struct SdfGenerator {
    config: SdfConfig,
}

impl SdfGenerator {
    /// 创建新的 SDF 生成器
    pub fn new(base_size: u32, buffer: usize) -> Self {
        Self {
            config: SdfConfig {
                base_size,
                buffer,
                ..Default::default()
            },
        }
    }

    /// 使用自定义配置创建生成器
    pub fn with_config(config: SdfConfig) -> Self {
        Self { config }
    }

    /// 获取配置
    pub fn config(&self) -> &SdfConfig {
        &self.config
    }

    /// 为指定字形生成 SDF 位图
    pub fn generate(&self, font_data: &[u8], glyph_id: u16) -> Option<SdfBitmap> {
        let font = FontRef::try_from_slice(font_data).ok()?;
        let glyph_id = GlyphId(glyph_id);
        self.generate_from_font(&font, glyph_id)
    }

    /// 为字符生成 SDF 位图
    pub fn generate_char(&self, font_data: &[u8], c: char) -> Option<SdfBitmap> {
        let font = FontRef::try_from_slice(font_data).ok()?;
        let glyph_id = font.glyph_id(c);
        self.generate_from_font(&font, glyph_id)
    }

    /// 从已加载的字体生成 SDF
    fn generate_from_font(&self, font: &FontRef, glyph_id: GlyphId) -> Option<SdfBitmap> {
        let scale = PxScale::from(self.config.base_size as f32);
        let scaled_font = font.as_scaled(scale);

        // 获取字形轮廓
        let glyph = glyph_id.with_scale(scale);
        let outlined = font.outline_glyph(glyph)?;

        // 获取边界
        let bounds = outlined.px_bounds();
        let glyph_width = bounds.width().ceil() as usize;
        let glyph_height = bounds.height().ceil() as usize;

        // 空字形检查
        if glyph_width == 0 || glyph_height == 0 {
            return None;
        }

        // 光栅化为 alpha 位图
        let mut alpha = vec![0u8; glyph_width * glyph_height];
        outlined.draw(|x, y, coverage| {
            let idx = y as usize * glyph_width + x as usize;
            if idx < alpha.len() {
                alpha[idx] = (coverage * 255.0) as u8;
            }
        });

        // 创建带 buffer 的 BitmapGlyph
        let bitmap =
            BitmapGlyph::from_unbuffered(&alpha, glyph_width, glyph_height, self.config.buffer)
                .ok()?;

        // 生成 SDF
        let sdf_f64 = bitmap.render_sdf(self.config.radius);

        // 转换为 u8
        let data = clamp_to_u8(&sdf_f64, self.config.cutoff).ok()?;

        // 计算最终尺寸（包含 buffer）
        let width = (glyph_width + self.config.buffer * 2) as u32;
        let height = (glyph_height + self.config.buffer * 2) as u32;

        // 计算度量信息
        // bearing_x: 纹理左边缘相对于笔触原点的 X 偏移
        // bounds.min.x 是字形左边缘，减去 buffer 得到纹理左边缘
        let bearing_x = bounds.min.x.floor() as i32 - self.config.buffer as i32;

        // bearing_y: 纹理顶边缘相对于基线的 Y 偏移
        // ab_glyph 的 bounds.min.y 是字形顶部（Y 向下为正，所以 min.y 是顶部）
        // 需要取负值并加上 buffer
        // 注意：ab_glyph 的坐标系是 Y 向下，所以 bounds.max.y 是底部，bounds.min.y 是顶部
        // 但 bounds.min.y 通常是负数（基线以上），所以 -bounds.min.y 是正数
        let bearing_y = -bounds.min.y.floor() as i32 + self.config.buffer as i32;

        let advance = scaled_font.h_advance(glyph_id);

        Some(SdfBitmap {
            data,
            width,
            height,
            bearing_x,
            bearing_y,
            advance,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SdfConfig::default();
        assert_eq!(config.base_size, 64);
        assert_eq!(config.buffer, 4);
        assert_eq!(config.radius, 8);
        assert!((config.cutoff - 0.25).abs() < 0.001);
    }

    #[test]
    fn test_generator_new() {
        let generator = SdfGenerator::new(64, 8);
        assert_eq!(generator.config().base_size, 64);
        assert_eq!(generator.config().buffer, 8);
    }

    #[test]
    fn test_generate_char_a() {
        let font_data = std::fs::read("assets/fonts/Inter-Regular.ttf").unwrap();
        let generator = SdfGenerator::new(64, 4);
        let bitmap = generator.generate_char(&font_data, 'A');

        assert!(bitmap.is_some());
        let bitmap = bitmap.unwrap();
        assert!(bitmap.width > 0);
        assert!(bitmap.height > 0);
        assert_eq!(bitmap.data.len(), (bitmap.width * bitmap.height) as usize);
    }

    #[test]
    fn test_space_returns_none() {
        let font_data = std::fs::read("assets/fonts/Inter-Regular.ttf").unwrap();
        let generator = SdfGenerator::new(64, 4);
        let bitmap = generator.generate_char(&font_data, ' ');

        // 空格没有轮廓，应该返回 None
        assert!(bitmap.is_none());
    }
}
