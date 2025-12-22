//! SDF Glyph Cache and Texture Atlas
//!
//! 管理 SDF 字形的生成和 GPU 纹理图集。
//!
//! ## 关键设计
//!
//! - 使用 SDF 生成器在 base_size (64px) 下生成 SDF 纹理
//! - 渲染时根据实际字号进行缩放
//! - 位置使用 cosmic-text 的布局，尺寸使用 SDF 的度量

use crate::features::lyrics::engine::sdf_generator::{SdfBitmap, SdfConfig, SdfGenerator};
use cosmic_text::{CacheKey, FontSystem};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::{Device, Queue};

/// 全局预生成缓存
/// 用于在后台线程生成 SDF 位图后，在主线程导入到 SdfCache
static GLOBAL_PRE_GENERATED: once_cell::sync::Lazy<Mutex<HashMap<CacheKey, SdfBitmap>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

/// 导入预生成的位图到全局缓存
pub fn import_to_global_cache(bitmaps: HashMap<CacheKey, SdfBitmap>) {
    let mut cache = GLOBAL_PRE_GENERATED.lock();
    for (key, bitmap) in bitmaps {
        cache.insert(key, bitmap);
    }
}

/// 从全局缓存中获取预生成的位图
pub fn take_from_global_cache(key: &CacheKey) -> Option<SdfBitmap> {
    GLOBAL_PRE_GENERATED.lock().remove(key)
}

/// 清空全局预生成缓存
pub fn clear_global_cache() {
    GLOBAL_PRE_GENERATED.lock().clear();
}

/// 获取全局预生成缓存的大小
pub fn global_cache_size() -> usize {
    GLOBAL_PRE_GENERATED.lock().len()
}

/// 纹理图集大小
/// 4096x4096 可以容纳更多字形，减少清空重建的频率
/// 对于中文歌词，常用汉字约 3000-5000 个，加上标点和英文，4096x4096 足够
const ATLAS_SIZE: u32 = 4096;
/// 字形之间的间距（gutter），防止双线性插值时边缘渗透
/// 4 像素足够防止相邻字形的颜色混合（线性插值需要 1 像素，安全边距 3 像素）
const ATLAS_GUTTER: u32 = 4;

/// 缓存的字形信息
#[derive(Debug, Clone, Copy)]
pub struct SdfGlyphInfo {
    /// 在图集中的 UV 坐标（归一化 0-1）
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    /// 字形尺寸（像素，来自 cosmic-text Placement）
    pub width: u32,
    pub height: u32,
    /// 相对于基线的偏移（来自 cosmic-text Placement）
    /// left: 字形左边缘相对于笔触原点的偏移
    /// top: 字形顶边缘相对于基线的偏移（正值表示基线以上）
    pub offset_x: i32,
    pub offset_y: i32,
    /// 水平前进宽度
    pub advance: f32,
}

/// 字形缓存键
/// 使用 cosmic-text 的 CacheKey 以保持与 TextShaper 的兼容性
pub type SdfGlyphKey = CacheKey;

/// 图集中的行（用于 shelf packing 算法）
struct AtlasRow {
    y: u32,
    height: u32,
    x_cursor: u32,
}

/// SDF 纹理图集
pub struct SdfAtlas {
    /// GPU 纹理（RGB 格式）
    texture: wgpu::Texture,
    /// 纹理视图
    pub view: wgpu::TextureView,
    /// 缓存的字形信息
    glyphs: HashMap<SdfGlyphKey, SdfGlyphInfo>,
    /// Shelf packing 行
    rows: Vec<AtlasRow>,
    /// 当前 Y 游标
    y_cursor: u32,
    /// 图集尺寸
    width: u32,
    height: u32,
    /// 是否需要重建（溢出时）
    needs_rebuild: bool,
}

impl SdfAtlas {
    /// 创建新的 SDF 图集
    pub fn new(device: &Device) -> Self {
        // 使用 Rgba8Unorm 格式，因为 wgpu 不支持 Rgb8Unorm
        // 我们会在上传时将 RGB 数据转换为 RGBA
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SDF Glyph Atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture,
            view,
            glyphs: HashMap::new(),
            rows: Vec::new(),
            y_cursor: 0,
            width: ATLAS_SIZE,
            height: ATLAS_SIZE,
            needs_rebuild: false,
        }
    }

    /// 获取已缓存的字形
    pub fn get(&self, key: &SdfGlyphKey) -> Option<SdfGlyphInfo> {
        self.glyphs.get(key).copied()
    }

    /// 缓存字形
    pub fn cache(
        &mut self,
        queue: &Queue,
        key: SdfGlyphKey,
        bitmap: &SdfBitmap,
    ) -> Option<SdfGlyphInfo> {
        if bitmap.width == 0 || bitmap.height == 0 {
            // 空字形（空格等）
            let info = SdfGlyphInfo {
                uv_min: [0.0, 0.0],
                uv_max: [0.0, 0.0],
                width: 0,
                height: 0,
                offset_x: bitmap.bearing_x,
                offset_y: bitmap.bearing_y,
                advance: bitmap.advance,
            };
            self.glyphs.insert(key, info);
            return Some(info);
        }

        // 在图集中分配空间
        let (x, y) = self.allocate(bitmap.width, bitmap.height)?;

        // 将单通道 SDF 数据转换为 RGBA
        let rgba_data = sdf_to_rgba(&bitmap.data);

        // 上传到 GPU
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &rgba_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bitmap.width * 4),
                rows_per_image: Some(bitmap.height),
            },
            wgpu::Extent3d {
                width: bitmap.width,
                height: bitmap.height,
                depth_or_array_layers: 1,
            },
        );

        // 计算 UV 坐标
        let uv_min = [x as f32 / self.width as f32, y as f32 / self.height as f32];
        let uv_max = [
            (x + bitmap.width) as f32 / self.width as f32,
            (y + bitmap.height) as f32 / self.height as f32,
        ];

        let info = SdfGlyphInfo {
            uv_min,
            uv_max,
            width: bitmap.width,
            height: bitmap.height,
            offset_x: bitmap.bearing_x,
            offset_y: bitmap.bearing_y,
            advance: bitmap.advance,
        };

        self.glyphs.insert(key, info);
        Some(info)
    }

    /// 缓存字形（使用 cosmic-text 的 Placement 度量）
    ///
    /// 这个方法使用 cosmic-text 提供的度量，而不是 SDF 生成器的度量，
    /// 确保与 cosmic-text 的布局完全一致。
    pub fn cache_with_placement(
        &mut self,
        queue: &Queue,
        key: SdfGlyphKey,
        bitmap: &SdfBitmap,
        placement_info: &SdfGlyphInfo,
    ) -> Option<SdfGlyphInfo> {
        if bitmap.width == 0 || bitmap.height == 0 {
            // 空字形（空格等）
            let info = SdfGlyphInfo {
                uv_min: [0.0, 0.0],
                uv_max: [0.0, 0.0],
                width: placement_info.width,
                height: placement_info.height,
                offset_x: placement_info.offset_x,
                offset_y: placement_info.offset_y,
                advance: placement_info.advance,
            };
            self.glyphs.insert(key, info);
            return Some(info);
        }

        // 在图集中分配空间
        let (x, y) = self.allocate(bitmap.width, bitmap.height)?;

        // 将单通道 SDF 数据转换为 RGBA
        let rgba_data = sdf_to_rgba(&bitmap.data);

        // 上传到 GPU
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &rgba_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bitmap.width * 4),
                rows_per_image: Some(bitmap.height),
            },
            wgpu::Extent3d {
                width: bitmap.width,
                height: bitmap.height,
                depth_or_array_layers: 1,
            },
        );

        // 计算 UV 坐标
        let uv_min = [x as f32 / self.width as f32, y as f32 / self.height as f32];
        let uv_max = [
            (x + bitmap.width) as f32 / self.width as f32,
            (y + bitmap.height) as f32 / self.height as f32,
        ];

        // 使用 cosmic-text 的度量，但 UV 坐标基于 SDF 纹理
        let info = SdfGlyphInfo {
            uv_min,
            uv_max,
            width: placement_info.width,
            height: placement_info.height,
            offset_x: placement_info.offset_x,
            offset_y: placement_info.offset_y,
            advance: placement_info.advance,
        };

        self.glyphs.insert(key, info);
        Some(info)
    }

    /// 使用 shelf packing 算法分配空间
    fn allocate(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        let padded_width = width + ATLAS_GUTTER * 2;
        let padded_height = height + ATLAS_GUTTER * 2;

        // 尝试放入现有行
        for row in &mut self.rows {
            if row.height >= padded_height && row.x_cursor + padded_width <= self.width {
                let x = row.x_cursor + ATLAS_GUTTER;
                let y = row.y + ATLAS_GUTTER;
                row.x_cursor += padded_width;
                return Some((x, y));
            }
        }

        // 创建新行
        if self.y_cursor + padded_height <= self.height {
            let row = AtlasRow {
                y: self.y_cursor,
                height: padded_height,
                x_cursor: padded_width,
            };
            let x = ATLAS_GUTTER;
            let y = self.y_cursor + ATLAS_GUTTER;
            self.y_cursor += padded_height;
            self.rows.push(row);
            return Some((x, y));
        }

        // 图集已满
        self.needs_rebuild = true;
        None
    }

    /// 清空图集
    pub fn clear(&mut self, queue: &Queue) {
        self.glyphs.clear();
        self.rows.clear();
        self.y_cursor = 0;
        self.needs_rebuild = false;

        // 清空纹理
        let clear_data = vec![0u8; (self.width * self.height * 4) as usize];
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &clear_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.width * 4),
                rows_per_image: Some(self.height),
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// 检查是否需要重建
    pub fn needs_rebuild(&self) -> bool {
        self.needs_rebuild
    }

    /// 获取纹理
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
}

/// 将单通道 SDF 数据转换为 RGBA（R=G=B=SDF, A=255）
fn sdf_to_rgba(sdf: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(sdf.len() * 4);
    for &v in sdf {
        rgba.push(v); // R
        rgba.push(v); // G
        rgba.push(v); // B
        rgba.push(255); // A (完全不透明)
    }
    rgba
}

/// 字体数据存储
/// 存储加载的字体文件数据，供 MSDF 生成器使用
pub struct FontStore {
    /// 字体数据列表（font_id -> (font_data, face_index)）
    fonts: HashMap<cosmic_text::fontdb::ID, (Arc<Vec<u8>>, u32)>,
    /// Enable debug logging
    debug_logging: bool,
}

impl FontStore {
    /// 创建新的字体存储
    pub fn new() -> Self {
        Self {
            fonts: HashMap::new(),
            debug_logging: false,
        }
    }

    /// Create with debug logging enabled
    pub fn with_debug(debug_logging: bool) -> Self {
        Self {
            fonts: HashMap::new(),
            debug_logging,
        }
    }

    /// 获取或加载字体数据
    /// 返回 (font_data, face_index)
    pub fn get_or_load(
        &mut self,
        font_system: &FontSystem,
        font_id: cosmic_text::fontdb::ID,
    ) -> Option<(Arc<Vec<u8>>, u32)> {
        if let Some((data, index)) = self.fonts.get(&font_id) {
            return Some((Arc::clone(data), *index));
        }

        // 从 fontdb 加载字体数据
        let db = font_system.db();
        let face_info = db.face(font_id)?;

        // 获取 face index（对于 TTC 字体很重要）
        let face_index = face_info.index;

        // 读取字体文件
        let source = &face_info.source;
        let data = match source {
            cosmic_text::fontdb::Source::Binary(data) => {
                if self.debug_logging {
                    tracing::debug!(
                        "[SdfCache] Loading font from binary data, face_index={}",
                        face_index
                    );
                }
                Arc::new(data.as_ref().as_ref().to_vec())
            }
            cosmic_text::fontdb::Source::File(path) => {
                if self.debug_logging {
                    tracing::debug!(
                        "[SdfCache] Loading font from file: {:?}, face_index={}",
                        path,
                        face_index
                    );
                }
                let data = std::fs::read(path).ok()?;
                Arc::new(data)
            }
            cosmic_text::fontdb::Source::SharedFile(path, data) => {
                if self.debug_logging {
                    tracing::debug!(
                        "[SdfCache] Loading font from shared file: {:?}, face_index={}",
                        path,
                        face_index
                    );
                }
                Arc::new(data.as_ref().as_ref().to_vec())
            }
        };

        self.fonts.insert(font_id, (Arc::clone(&data), face_index));
        Some((data, face_index))
    }
}

impl Default for FontStore {
    fn default() -> Self {
        Self::new()
    }
}

/// 共享字体系统类型
pub type SharedFontSystem = Arc<Mutex<FontSystem>>;

/// 预生成的 SDF 位图（用于异步生成）
#[derive(Clone)]
pub struct PreGeneratedSdf {
    pub bitmap: SdfBitmap,
}

/// 线程安全的 SDF 缓存管理器
pub struct SdfCache {
    /// SDF 生成器
    generator: SdfGenerator,
    /// 字体存储
    font_store: Mutex<FontStore>,
    /// 纹理图集
    atlas: Mutex<SdfAtlas>,
    /// 共享字体系统（必须与 TextShaper 使用同一实例）
    font_system: SharedFontSystem,
    /// Enable debug logging
    debug_logging: bool,
    /// 预生成的 SDF 位图缓存（用于异步生成后在主线程上传）
    pre_generated: Mutex<HashMap<CacheKey, PreGeneratedSdf>>,
}

impl SdfCache {
    /// 创建新的 SDF 缓存
    ///
    /// IMPORTANT: font_system 必须与 TextShaper 使用同一实例！
    /// CacheKey 包含 font_id，必须匹配才能正确查找字形。
    pub fn new(device: &Device, font_system: SharedFontSystem) -> Self {
        Self::with_debug(device, font_system, false)
    }

    /// Create with debug logging enabled
    pub fn with_debug(device: &Device, font_system: SharedFontSystem, debug_logging: bool) -> Self {
        Self {
            // base_size = 64px, buffer = 4px
            // 64px 是速度和质量的平衡点：
            // - 比 96px 快约 2 倍
            // - 质量足够好，适合大多数显示器
            // - buffer = 4 保持笔画清晰不粘连
            generator: SdfGenerator::new(64, 4),
            font_store: Mutex::new(FontStore::with_debug(debug_logging)),
            atlas: Mutex::new(SdfAtlas::new(device)),
            font_system,
            debug_logging,
            pre_generated: Mutex::new(HashMap::new()),
        }
    }

    /// 使用自定义配置创建
    pub fn with_config(device: &Device, font_system: SharedFontSystem, config: SdfConfig) -> Self {
        Self {
            generator: SdfGenerator::with_config(config),
            font_store: Mutex::new(FontStore::new()),
            atlas: Mutex::new(SdfAtlas::new(device)),
            font_system,
            debug_logging: false,
            pre_generated: Mutex::new(HashMap::new()),
        }
    }

    /// 获取或缓存字形
    ///
    /// 使用 msdfgen 生成 SDF 纹理和度量。
    /// 度量是在 base_size (48px) 下计算的，需要在渲染时缩放。
    ///
    /// 当图集空间不足时，会自动清空图集并重试。
    pub fn get_glyph(&self, queue: &Queue, cache_key: CacheKey) -> Option<SdfGlyphInfo> {
        // 先检查图集缓存
        {
            let atlas = self.atlas.lock();
            if let Some(info) = atlas.get(&cache_key) {
                return Some(info);
            }
        }

        // 检查本地预生成缓存
        let pre_gen_bitmap = {
            let mut pre_gen = self.pre_generated.lock();
            pre_gen.remove(&cache_key)
        };

        // 如果本地缓存没有，检查全局预生成缓存
        let pre_gen_bitmap = pre_gen_bitmap.or_else(|| {
            take_from_global_cache(&cache_key).map(|bitmap| PreGeneratedSdf { bitmap })
        });

        let bitmap = if let Some(pre_gen) = pre_gen_bitmap {
            // 使用预生成的位图（快速路径）
            pre_gen.bitmap
        } else {
            // 需要同步生成（慢速路径）
            // 获取字体数据和 face index
            let font_system = self.font_system.lock();
            let mut font_store = self.font_store.lock();

            // Check if font_id exists in the font system
            let font_info = font_system.db().face(cache_key.font_id);
            if font_info.is_none() && self.debug_logging {
                tracing::warn!(
                    "[SdfCache] Font mismatch: font_id {:?} not found in font system",
                    cache_key.font_id
                );
            }

            let (font_data, _face_index) =
                font_store.get_or_load(&font_system, cache_key.font_id)?;
            drop(font_system); // 释放锁

            // 生成 SDF
            self.generator.generate(&font_data, cache_key.glyph_id)?
        };

        // 缓存到图集
        let mut atlas = self.atlas.lock();

        // 尝试缓存，如果失败（图集满了），清空后重试
        match atlas.cache(queue, cache_key, &bitmap) {
            Some(info) => Some(info),
            None => {
                // 图集空间不足，清空后重试
                if self.debug_logging {
                    tracing::info!("[SdfCache] Atlas full, clearing and retrying...");
                }
                atlas.clear(queue);
                atlas.cache(queue, cache_key, &bitmap)
            }
        }
    }

    /// 预生成 MSDF 位图（不需要 GPU，可在后台线程调用）
    ///
    /// 这个方法只生成位图并缓存，不上传到 GPU。
    /// 后续调用 get_glyph 时会使用预生成的位图，只需要上传到 GPU（快速操作）。
    ///
    /// 返回 true 如果成功生成或已经在缓存中
    pub fn pre_generate_glyph(&self, cache_key: CacheKey) -> bool {
        // 先检查图集缓存
        {
            let atlas = self.atlas.lock();
            if atlas.get(&cache_key).is_some() {
                return true; // 已经在图集中
            }
        }

        // 检查预生成缓存
        {
            let pre_gen = self.pre_generated.lock();
            if pre_gen.contains_key(&cache_key) {
                return true; // 已经预生成
            }
        }

        // 获取字体数据和 face index
        let font_system = self.font_system.lock();
        let mut font_store = self.font_store.lock();

        let font_info = font_system.db().face(cache_key.font_id);
        if font_info.is_none() {
            if self.debug_logging {
                tracing::warn!(
                    "[SdfCache] Font mismatch: font_id {:?} not found in font system",
                    cache_key.font_id
                );
            }
            return false;
        }

        let Some((font_data, _face_index)) =
            font_store.get_or_load(&font_system, cache_key.font_id)
        else {
            return false;
        };
        drop(font_system); // 释放锁
        drop(font_store);

        // 生成 SDF
        let Some(bitmap) = self.generator.generate(&font_data, cache_key.glyph_id) else {
            return false;
        };

        // 缓存预生成的位图
        let mut pre_gen = self.pre_generated.lock();
        pre_gen.insert(cache_key, PreGeneratedSdf { bitmap });

        true
    }

    /// 批量预生成 MSDF 位图
    ///
    /// 用于在后台线程中预生成所有需要的字形
    pub fn pre_generate_glyphs(&self, cache_keys: &[CacheKey]) -> usize {
        let mut generated = 0;
        for key in cache_keys {
            if self.pre_generate_glyph(*key) {
                generated += 1;
            }
        }
        generated
    }

    /// 清空预生成缓存
    pub fn clear_pre_generated(&self) {
        self.pre_generated.lock().clear();
    }

    /// 获取预生成缓存的大小
    pub fn pre_generated_count(&self) -> usize {
        self.pre_generated.lock().len()
    }

    /// 获取图集纹理视图
    pub fn atlas_view(&self) -> wgpu::TextureView {
        let atlas = self.atlas.lock();
        atlas
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// 清空缓存
    pub fn clear(&self, queue: &Queue) {
        self.atlas.lock().clear(queue);
    }

    /// 检查是否需要重建
    pub fn needs_rebuild(&self) -> bool {
        self.atlas.lock().needs_rebuild()
    }

    /// 获取 SDF buffer（用于 shader）
    pub fn sdf_buffer(&self) -> usize {
        self.generator.config().buffer
    }

    /// 获取基准字号（用于 shader）
    pub fn base_size(&self) -> u32 {
        self.generator.config().base_size
    }
}

/// 独立的 SDF 预生成器
///
/// 这个结构体可以在后台线程中使用，不需要 GPU 资源。
/// 生成的位图可以通过 `take_all` 方法获取，
/// 然后在主线程中上传到 GPU。
///
/// ## 使用方式
///
/// ```ignore
/// // 在后台线程中
/// let pre_gen = SdfPreGenerator::new(font_system.clone());
/// let cache_keys: Vec<CacheKey> = shaped_lines.iter()
///     .flat_map(|line| line.main.glyphs.iter().map(|g| g.cache_key))
///     .collect();
/// pre_gen.generate_all(&cache_keys);
/// let bitmaps = pre_gen.take_all();
///
/// // 在主线程中
/// sdf_cache.import_pre_generated(bitmaps);
/// ```
pub struct SdfPreGenerator {
    /// SDF 生成器
    generator: SdfGenerator,
    /// 字体存储
    font_store: Mutex<FontStore>,
    /// 共享字体系统
    font_system: SharedFontSystem,
    /// 预生成的位图
    pre_generated: Mutex<HashMap<CacheKey, SdfBitmap>>,
}

impl SdfPreGenerator {
    /// 创建新的预生成器
    pub fn new(font_system: SharedFontSystem) -> Self {
        Self {
            // 使用与 SdfCache 相同的配置
            generator: SdfGenerator::new(64, 4),
            font_store: Mutex::new(FontStore::new()),
            font_system,
            pre_generated: Mutex::new(HashMap::new()),
        }
    }

    /// 预生成单个字形
    pub fn generate(&self, cache_key: CacheKey) -> bool {
        // 检查是否已经生成
        {
            let pre_gen = self.pre_generated.lock();
            if pre_gen.contains_key(&cache_key) {
                return true;
            }
        }

        // 获取字体数据
        let font_system = self.font_system.lock();
        let mut font_store = self.font_store.lock();

        let Some((font_data, _face_index)) =
            font_store.get_or_load(&font_system, cache_key.font_id)
        else {
            return false;
        };
        drop(font_system);
        drop(font_store);

        // 生成 SDF
        let Some(bitmap) = self.generator.generate(&font_data, cache_key.glyph_id) else {
            return false;
        };

        // 缓存
        let mut pre_gen = self.pre_generated.lock();
        pre_gen.insert(cache_key, bitmap);

        true
    }

    /// 批量预生成字形
    pub fn generate_all(&self, cache_keys: &[CacheKey]) -> usize {
        let mut generated = 0;
        for key in cache_keys {
            if self.generate(*key) {
                generated += 1;
            }
        }
        generated
    }

    /// 获取所有预生成的位图并清空缓存
    pub fn take_all(&self) -> HashMap<CacheKey, SdfBitmap> {
        let mut pre_gen = self.pre_generated.lock();
        std::mem::take(&mut *pre_gen)
    }

    /// 获取预生成的位图数量
    pub fn count(&self) -> usize {
        self.pre_generated.lock().len()
    }
}

impl SdfCache {
    /// 导入预生成的位图
    ///
    /// 将后台线程生成的位图导入到预生成缓存中，
    /// 后续调用 get_glyph 时会使用这些位图。
    pub fn import_pre_generated(&self, bitmaps: HashMap<CacheKey, SdfBitmap>) {
        let mut pre_gen = self.pre_generated.lock();
        for (key, bitmap) in bitmaps {
            pre_gen.insert(key, PreGeneratedSdf { bitmap });
        }
    }
}
