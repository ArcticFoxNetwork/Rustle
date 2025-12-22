//! GPU Pipeline for Apple Music-style lyrics rendering (SDF Version)
//!
//! Custom rendering pipeline that:
//! 1. Uses cosmic-text for text shaping
//! 2. Manages SDF glyph atlas (SDF)
//! 3. Passes timing data per-vertex to GPU
//! 4. Implements word-by-word highlighting in shader
//! 5. Renders interlude dots with breathing animation
//! 6. Supports translation and romanized text
//! 7. Implements virtualization (isInSight) for performance
//! 8. Single-pass SDF rendering with built-in blur effects
//!
//! ## SDF Rendering Architecture
//!
//! ```text
//! cosmic-text (text shaping)
//!     ↓
//! SDF generator (8SSEDT algorithm)
//!     ↓
//! SDF Atlas (4096x4096 RGBA texture)
//!     ↓
//! lyrics_sdf.wgsl (SDF math + fwidth AA)
//!     ↓
//! Single pass with all effects
//! ```

use bytemuck::{Pod, Zeroable};
use cosmic_text::FontSystem;
use parking_lot::{Mutex, RwLock};
use std::sync::Arc;
use wgpu::{Device, Queue, TextureFormat};

use super::CachedShapedLine;
use super::interlude_dots::InterludeDots;
use super::per_line_blur::{LineRenderInfo, PerLineBlurRenderer};
use super::sdf_cache::SdfCache;
use super::text_shaper::{ShapedLine, TextShaper};
use super::types::{ComputedLineStyle, FontConfig, LyricLineData};
use super::vertex::{GlobalUniform, LineUniform, LyricGlyphVertex};

/// Uniform data for interlude dots rendering
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DotsUniform {
    /// Position in physical pixels (relative to widget)
    pub position: [f32; 2],
    /// Overall scale (0.0 - 1.0, includes breathing animation)
    pub scale: f32,
    /// Dot size in pixels
    pub dot_size: f32,
    /// Dot spacing in pixels
    pub dot_spacing: f32,
    /// Individual dot opacities (0.0 - 1.0)
    pub dot0_opacity: f32,
    pub dot1_opacity: f32,
    pub dot2_opacity: f32,
    /// Whether dots are enabled
    pub enabled: f32,
    /// Padding to align viewport_size to 8 bytes (WGSL vec2 alignment)
    pub _pad1: f32,
    /// Viewport info
    pub viewport_size: [f32; 2],
    pub bounds_offset: [f32; 2],
    /// Padding to align _padding to 16 bytes (WGSL vec4 alignment)
    pub _pad2: [f32; 2],
    /// Final padding (vec4<f32>)
    pub _padding: [f32; 4],
}

impl Default for DotsUniform {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            scale: 0.0,
            dot_size: 12.0,
            dot_spacing: 20.0,
            dot0_opacity: 0.0,
            dot1_opacity: 0.0,
            dot2_opacity: 0.0,
            enabled: 0.0,
            _pad1: 0.0,
            viewport_size: [800.0, 600.0],
            bounds_offset: [0.0, 0.0],
            _pad2: [0.0, 0.0],
            _padding: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

impl DotsUniform {
    /// Create from InterludeDots state
    pub fn from_interlude_dots(
        dots: &InterludeDots,
        viewport_size: [f32; 2],
        bounds_offset: [f32; 2],
        scale_factor: f32,
    ) -> Self {
        Self {
            position: [dots.left * scale_factor, dots.top * scale_factor],
            scale: dots.scale,
            dot_size: 12.0 * scale_factor,
            dot_spacing: 20.0 * scale_factor,
            dot0_opacity: dots.dot_opacities[0],
            dot1_opacity: dots.dot_opacities[1],
            dot2_opacity: dots.dot_opacities[2],
            enabled: if dots.enabled { 1.0 } else { 0.0 },
            _pad1: 0.0,
            viewport_size,
            bounds_offset,
            _pad2: [0.0, 0.0],
            _padding: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

/// Composite shader uniform
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CompositeUniform {
    pub viewport_size: [f32; 2],
    pub current_time_ms: f32,
    pub font_size: f32,
}

/// Shared font system for both text shaping and glyph caching
/// This is critical - CacheKey contains font_id which must match!
pub type SharedFontSystem = Arc<Mutex<FontSystem>>;

/// Maximum number of glyphs per frame
const MAX_GLYPHS: usize = 8192;
/// Maximum number of lines
const MAX_LINES: usize = 128;

/// GPU resources for lyrics rendering
pub struct LyricsGpuPipeline {
    // === Direct rendering pipeline (single output) ===
    pipeline: wgpu::RenderPipeline,

    // === MRT rendering pipeline (color + blur_info) ===
    mrt_pipeline: wgpu::RenderPipeline,

    // Buffers for lyrics
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    global_uniform_buffer: wgpu::Buffer,
    line_uniform_buffer: wgpu::Buffer,

    // Bind groups for lyrics
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: Option<wgpu::BindGroup>,

    // Interlude dots rendering
    dots_pipeline: wgpu::RenderPipeline,
    dots_uniform_buffer: wgpu::Buffer,
    dots_bind_group_layout: wgpu::BindGroupLayout,
    dots_bind_group: Option<wgpu::BindGroup>,
    dots_enabled: bool,

    // Glyph management (SDF)
    sdf_cache: SdfCache,
    text_shaper: TextShaper,

    // Font configuration
    font_config: FontConfig,

    // Intermediate textures (using RwLock for interior mutability)
    lyrics_texture: RwLock<Option<RenderTexture>>,
    blur_info_texture: RwLock<Option<RenderTexture>>,

    // === 逐行模糊渲染器 (正确的 Apple Music 风格模糊) ===
    per_line_blur: RwLock<PerLineBlurRenderer>,

    // Composite pipeline
    composite_pipeline: wgpu::RenderPipeline,
    composite_bind_group_layout: wgpu::BindGroupLayout,
    composite_uniform_buffer: wgpu::Buffer,
    composite_sampler: wgpu::Sampler,
    // Pre-created composite bind group (created in prepare, used in render)
    composite_bind_group: RwLock<Option<wgpu::BindGroup>>,

    // State
    enable_blur: bool,
    vertex_count: u32,
    index_count: u32,
    format: TextureFormat,
    texture_size: RwLock<(u32, u32)>,

    // Cached viewport info for render pass
    cached_viewport: RwLock<(u32, u32)>,

    // === 逐行渲染索引范围 ===
    // 每行的索引范围 (start_index, index_count)
    // 用于逐行模糊渲染
    line_index_ranges: RwLock<Vec<(u32, u32)>>,

    // === 逐行渲染信息 ===
    // 缓存的行渲染信息，用于逐行模糊
    cached_line_render_info: RwLock<Vec<LineRenderInfo>>,
}

/// Intermediate render texture
struct RenderTexture {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl RenderTexture {
    fn new(device: &Device, width: u32, height: u32, format: TextureFormat, label: &str) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self { texture, view }
    }
}

impl LyricsGpuPipeline {
    /// Create a new GPU pipeline with default font configuration
    pub fn new(device: &Device, format: TextureFormat) -> Self {
        Self::with_config(device, format, FontConfig::default())
    }

    /// Create a new GPU pipeline with custom font configuration
    pub fn with_config(device: &Device, format: TextureFormat, font_config: FontConfig) -> Self {
        // Create SHARED font system - critical for CacheKey matching!
        let mut font_system = FontSystem::new();

        // Load custom fonts from assets directory
        Self::load_custom_fonts(&mut font_system, font_config.debug_logging);

        let font_system: SharedFontSystem = Arc::new(Mutex::new(font_system));

        // Both SdfCache and TextShaper must use the SAME FontSystem instance
        // Pass debug_logging to SdfCache
        let sdf_cache =
            SdfCache::with_debug(device, Arc::clone(&font_system), font_config.debug_logging);
        // Pass font config to TextShaper
        let text_shaper = TextShaper::with_config(Arc::clone(&font_system), font_config.clone());

        // Create bind group layout for lyrics rendering
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lyrics Bind Group Layout"),
            entries: &[
                // Global uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Line uniforms (storage buffer)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Glyph atlas texture
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Lyrics Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Load SDF shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lyrics SDF Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/lyrics_sdf.wgsl").into()),
        });

        // Create direct render pipeline (single output)
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Lyrics Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[LyricGlyphVertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create MRT render pipeline (same as direct pipeline for SDF)
        // SDF doesn't need MRT - blur is done via smoothstep in shader
        let mrt_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Lyrics MRT Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[LyricGlyphVertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[
                    // Single color output (SDF doesn't need MRT)
                    Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                ],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create buffers
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lyrics Vertex Buffer"),
            size: (std::mem::size_of::<LyricGlyphVertex>() * MAX_GLYPHS * 4) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lyrics Index Buffer"),
            size: (std::mem::size_of::<u32>() * MAX_GLYPHS * 6) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let global_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lyrics Global Uniform Buffer"),
            size: std::mem::size_of::<GlobalUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let line_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lyrics Line Uniform Buffer"),
            size: (std::mem::size_of::<LineUniform>() * MAX_LINES) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // === Interlude Dots Pipeline ===
        let (dots_pipeline, dots_bind_group_layout, dots_uniform_buffer) =
            Self::create_dots_pipeline(device, format);

        // Create composite pipeline
        let (
            composite_pipeline,
            composite_bind_group_layout,
            composite_uniform_buffer,
            composite_sampler,
        ) = Self::create_composite_pipeline(device, format);

        Self {
            pipeline,
            mrt_pipeline,
            vertex_buffer,
            index_buffer,
            global_uniform_buffer,
            line_uniform_buffer,
            bind_group_layout,
            bind_group: None,
            dots_pipeline,
            dots_uniform_buffer,
            dots_bind_group_layout,
            dots_bind_group: None,
            dots_enabled: false,
            sdf_cache,
            text_shaper,
            font_config,
            lyrics_texture: RwLock::new(None),
            blur_info_texture: RwLock::new(None),
            per_line_blur: RwLock::new(PerLineBlurRenderer::new(device, format)),
            composite_pipeline,
            composite_bind_group_layout,
            composite_uniform_buffer,
            composite_sampler,
            composite_bind_group: RwLock::new(None),
            // 启用逐行模糊渲染（正确的 Apple Music 风格）
            enable_blur: true,
            vertex_count: 0,
            index_count: 0,
            format,
            texture_size: RwLock::new((0, 0)),
            cached_viewport: RwLock::new((0, 0)),
            line_index_ranges: RwLock::new(Vec::new()),
            cached_line_render_info: RwLock::new(Vec::new()),
        }
    }

    /// Load custom fonts from assets/fonts directory
    fn load_custom_fonts(font_system: &mut FontSystem, debug_logging: bool) {
        let font_paths = [
            "assets/fonts/NotoSansCJKsc-Regular.otf",
            "assets/fonts/Inter-Regular.ttf",
        ];

        for path in &font_paths {
            match std::fs::read(path) {
                Ok(data) => {
                    font_system.db_mut().load_font_data(data);
                    if debug_logging {
                        tracing::debug!("[LyricsGpuPipeline] Loaded custom font: {}", path);
                    }
                }
                Err(e) => {
                    if debug_logging {
                        tracing::warn!("[LyricsGpuPipeline] Failed to load font {}: {}", path, e);
                    }
                }
            }
        }

        if debug_logging {
            // Log available font families
            let db = font_system.db();
            let families: Vec<_> = db.faces().map(|f| f.families.clone()).collect();
            tracing::debug!(
                "[LyricsGpuPipeline] Available font families: {:?}",
                families.len()
            );
        }
    }

    /// Create interlude dots pipeline
    fn create_dots_pipeline(
        device: &Device,
        format: TextureFormat,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout, wgpu::Buffer) {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Interlude Dots Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Interlude Dots Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Interlude Dots Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/interlude_dots.wgsl").into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Interlude Dots Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Interlude Dots Uniform Buffer"),
            size: std::mem::size_of::<DotsUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        (pipeline, bind_group_layout, uniform_buffer)
    }

    /// Create composite pipeline for multi-pass blur
    fn create_composite_pipeline(
        device: &Device,
        format: TextureFormat,
    ) -> (
        wgpu::RenderPipeline,
        wgpu::BindGroupLayout,
        wgpu::Buffer,
        wgpu::Sampler,
    ) {
        // Bind group layout: uniform + 6 blur levels + blur_info + sampler
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Composite Bind Group Layout"),
            entries: &[
                // Uniform buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Blur level 0 (original)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Blur level 1
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Blur level 2
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Blur level 3
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Blur level 4
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Blur level 5
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Blur info texture
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Composite Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Composite Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/lyrics_composite.wgsl").into()),
        });

        // Use full composite shader with blur_info texture
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Composite Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Composite Uniform Buffer"),
            size: std::mem::size_of::<CompositeUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Composite Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        (pipeline, bind_group_layout, uniform_buffer, sampler)
    }

    /// Ensure intermediate textures exist with correct size
    fn ensure_textures(&self, device: &Device, width: u32, height: u32) {
        let current_size = *self.texture_size.read();
        if current_size == (width, height)
            && self.lyrics_texture.read().is_some()
            && self.blur_info_texture.read().is_some()
        {
            return;
        }

        *self.texture_size.write() = (width, height);

        // Create lyrics render texture (color)
        *self.lyrics_texture.write() = Some(RenderTexture::new(
            device,
            width,
            height,
            self.format,
            "Lyrics Render Texture",
        ));

        // Create blur info texture (Rgba16Float for precision)
        *self.blur_info_texture.write() = Some(RenderTexture::new(
            device,
            width,
            height,
            wgpu::TextureFormat::Rgba16Float,
            "Blur Info Texture",
        ));
    }

    /// Prepare rendering data
    ///
    /// `line_heights` must be pre-calculated by LyricsEngine (Single Source of Truth)
    /// to avoid double computation and ensure layout consistency.
    ///
    /// IMPORTANT: To ensure consistent text wrapping between LyricsEngine and GPU Pipeline,
    /// we use LOGICAL pixels for shape_line calls, then scale to physical pixels for rendering.
    /// This avoids floating-point precision issues that cause different wrap results.
    ///
    /// DEPRECATED: Use prepare_with_shaped_lines instead for Single Source of Truth architecture.
    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn prepare(
        &mut self,
        device: &Device,
        queue: &Queue,
        viewport_width: f32,
        viewport_height: f32,
        bounds_x: f32,
        bounds_y: f32,
        bounds_width: f32,
        bounds_height: f32,
        lines: &[LyricLineData],
        line_styles: &[ComputedLineStyle],
        line_heights: &[f32], // Pre-calculated by LyricsEngine (in PHYSICAL pixels)
        current_time_ms: f32,
        scroll_y: f32,
        font_size: f32, // Physical pixels
        word_fade_width: f32,
        scale: f32, // Scale factor for logical to physical conversion
    ) {
        // Update global uniforms
        let globals = GlobalUniform {
            viewport_size: [viewport_width, viewport_height],
            bounds_offset: [bounds_x, bounds_y],
            bounds_size: [bounds_width, bounds_height],
            current_time_ms,
            word_fade_width,
            font_size,
            scroll_y,
            align_position: 0.35,
            sdf_range: 4.0, // Default SDF range for distance extrapolation
        };
        queue.write_buffer(&self.global_uniform_buffer, 0, bytemuck::bytes_of(&globals));

        // Use pre-calculated line_heights from LyricsEngine (Single Source of Truth)
        // No more duplicate calculation here!

        // Update line uniforms
        let line_uniforms: Vec<LineUniform> = line_styles
            .iter()
            .enumerate()
            .take(MAX_LINES)
            .map(|(idx, style)| {
                let actual_height = line_heights.get(idx).copied().unwrap_or(font_size * 1.4);
                LineUniform {
                    y_position: style.y_position,
                    scale: style.scale,
                    blur: style.blur,
                    opacity: style.opacity,
                    glow: style.glow,
                    is_active: if style.is_active { 1 } else { 0 },
                    line_height: actual_height,
                    _padding: 0.0,
                }
            })
            .collect();

        if !line_uniforms.is_empty() {
            queue.write_buffer(
                &self.line_uniform_buffer,
                0,
                bytemuck::cast_slice(&line_uniforms),
            );
        }

        // Build geometry (使用逐行组织，支持逐行模糊)
        // Pass scale factor so shape_line can use logical pixels for consistent wrapping
        let (vertices, indices) = self.build_geometry_per_line(
            queue,
            lines,
            line_styles,
            bounds_width,
            bounds_height,
            font_size,
            current_time_ms,
            scale,
        );

        self.vertex_count = vertices.len() as u32;
        self.index_count = indices.len() as u32;

        if !vertices.is_empty() {
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        }
        if !indices.is_empty() {
            queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&indices));
        }

        // Update bind group
        self.update_bind_group(device);

        // 更新逐行模糊渲染器的视口尺寸
        self.per_line_blur
            .write()
            .set_viewport_size(bounds_width as u32, bounds_height as u32);
    }

    /// Prepare rendering data using pre-shaped lines (Single Source of Truth)
    ///
    /// This is the preferred method that uses CachedShapedLine from LyricsEngine,
    /// ensuring consistent text layout between CPU and GPU.
    ///
    /// The shaped_lines contain all glyph positions calculated by LyricsEngine,
    /// so we don't need to call shape_line again here.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_with_shaped_lines(
        &mut self,
        device: &Device,
        queue: &Queue,
        viewport_width: f32,
        viewport_height: f32,
        bounds_x: f32,
        bounds_y: f32,
        bounds_width: f32,
        bounds_height: f32,
        lines: &[LyricLineData],
        shaped_lines: &[CachedShapedLine],
        line_styles: &[ComputedLineStyle],
        line_heights: &[f32], // Pre-calculated by LyricsEngine (in PHYSICAL pixels)
        current_time_ms: f32,
        scroll_y: f32,
        font_size: f32, // Physical pixels
        word_fade_width: f32,
        scale: f32, // Scale factor for logical to physical conversion
    ) {
        // Update global uniforms
        let globals = GlobalUniform {
            viewport_size: [viewport_width, viewport_height],
            bounds_offset: [bounds_x, bounds_y],
            bounds_size: [bounds_width, bounds_height],
            current_time_ms,
            word_fade_width,
            font_size,
            scroll_y,
            align_position: 0.35,
            sdf_range: 4.0, // Default SDF range for distance extrapolation
        };
        queue.write_buffer(&self.global_uniform_buffer, 0, bytemuck::bytes_of(&globals));

        // Update line uniforms
        let line_uniforms: Vec<LineUniform> = line_styles
            .iter()
            .enumerate()
            .take(MAX_LINES)
            .map(|(idx, style)| {
                let actual_height = line_heights.get(idx).copied().unwrap_or(font_size * 1.4);
                LineUniform {
                    y_position: style.y_position,
                    scale: style.scale,
                    blur: style.blur,
                    opacity: style.opacity,
                    glow: style.glow,
                    is_active: if style.is_active { 1 } else { 0 },
                    line_height: actual_height,
                    _padding: 0.0,
                }
            })
            .collect();

        if !line_uniforms.is_empty() {
            queue.write_buffer(
                &self.line_uniform_buffer,
                0,
                bytemuck::cast_slice(&line_uniforms),
            );
        }

        // Build geometry from pre-shaped lines (Single Source of Truth)
        // No more duplicate shape_line calls!
        let (vertices, indices) = self.build_geometry_from_shaped(
            queue,
            lines,
            shaped_lines,
            line_styles,
            bounds_width,
            bounds_height,
            font_size,
            current_time_ms,
            scale,
        );

        self.vertex_count = vertices.len() as u32;
        self.index_count = indices.len() as u32;

        if !vertices.is_empty() {
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        }
        if !indices.is_empty() {
            queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&indices));
        }

        // Update bind group
        self.update_bind_group(device);

        // 更新逐行模糊渲染器的视口尺寸
        self.per_line_blur
            .write()
            .set_viewport_size(bounds_width as u32, bounds_height as u32);
    }

    /// Build geometry for all visible lines, organized by line (for per-line blur)
    ///
    /// 为了实现正确的逐行模糊渲染，顶点按行组织：
    /// 每行的顶点是连续的，这样可以单独渲染每行并应用模糊。
    ///
    /// IMPORTANT: To ensure consistent text wrapping with LyricsEngine:
    /// - shape_line is called with LOGICAL pixels (viewport_width/scale, font_size/scale)
    /// - Glyph positions are then scaled back to physical pixels for rendering
    /// This ensures the same wrap points as LyricsEngine's calculate_line_heights.
    ///
    /// 返回值包含所有顶点和索引，同时更新 line_index_ranges 和 cached_line_render_info
    fn build_geometry_per_line(
        &mut self,
        queue: &Queue,
        lines: &[LyricLineData],
        line_styles: &[ComputedLineStyle],
        viewport_width: f32,  // Physical pixels
        viewport_height: f32, // Physical pixels
        font_size: f32,       // Physical pixels
        current_time_ms: f32,
        scale: f32, // Scale factor (physical / logical)
    ) -> (Vec<LyricGlyphVertex>, Vec<u32>) {
        let mut all_vertices = Vec::with_capacity(MAX_GLYPHS * 4);
        let mut all_indices = Vec::with_capacity(MAX_GLYPHS * 6);
        let mut line_index_ranges = Vec::with_capacity(lines.len());
        let mut line_render_info = Vec::with_capacity(lines.len());

        let has_duet_line = lines.iter().any(|l| l.is_duet);
        let base_padding = viewport_width * 0.05;
        let overscan_px = 300.0;

        // Convert to logical pixels for shape_line (to match LyricsEngine)
        let logical_viewport_width = viewport_width / scale;
        let logical_font_size = font_size / scale;

        for (line_idx, line) in lines.iter().enumerate() {
            let style = line_styles.get(line_idx).cloned().unwrap_or_default();

            let line_height = font_size * 1.4;
            let visible = style.opacity >= 0.01
                && LyricLineData::is_in_sight(
                    style.y_position,
                    line_height,
                    viewport_height,
                    overscan_px,
                );

            // 记录这行的起始索引
            let start_index = all_indices.len() as u32;

            if visible {
                // Calculate padding in logical pixels (to match LyricsEngine)
                let logical_base_padding = logical_viewport_width * 0.05;
                let (logical_padding_left, logical_padding_right) = if has_duet_line {
                    if line.is_duet {
                        (logical_viewport_width * 0.15, logical_base_padding)
                    } else {
                        (logical_base_padding, logical_viewport_width * 0.15)
                    }
                } else {
                    (logical_base_padding, logical_base_padding)
                };

                // Content width in logical pixels (same as LyricsEngine)
                let logical_content_width =
                    logical_viewport_width - logical_padding_left - logical_padding_right;

                // Shape text using LOGICAL pixels (same as LyricsEngine)
                let shaped = self.text_shaper.shape_line(
                    &line.text,
                    &line.words,
                    logical_font_size,
                    logical_content_width,
                );

                // Convert padding to physical pixels for rendering
                let (padding_left, padding_right) = if has_duet_line {
                    if line.is_duet {
                        (viewport_width * 0.15, base_padding)
                    } else {
                        (base_padding, viewport_width * 0.15)
                    }
                } else {
                    (base_padding, base_padding)
                };

                // Line X position in physical pixels
                let line_x = if line.is_duet {
                    viewport_width - shaped.width * scale - padding_right
                } else {
                    padding_left
                };

                // Add glyphs for main text
                // SDF 纹理是在 base_size (64px) 下生成的，需要缩放到实际字号
                // 所有 SDF 度量（width, height, bearing_x, bearing_y）都乘以 sdf_scale
                let sdf_base_size = 64.0_f32;
                let sdf_scale = font_size / sdf_base_size;

                for glyph in &shaped.glyphs {
                    let glyph_info = match self.sdf_cache.get_glyph(queue, glyph.cache_key) {
                        Some(info) => info,
                        None => continue,
                    };

                    if glyph_info.width == 0 || glyph_info.height == 0 {
                        continue;
                    }

                    // 所有 SDF 度量统一乘以 sdf_scale
                    let scaled_width = glyph_info.width as f32 * sdf_scale;
                    let scaled_height = glyph_info.height as f32 * sdf_scale;
                    let scaled_bearing_x = glyph_info.offset_x as f32 * sdf_scale;
                    let scaled_bearing_y = glyph_info.offset_y as f32 * sdf_scale;

                    // 字形位置计算：
                    // glyph.x 和 glyph.y 是逻辑像素，需要乘以 scale 转换为物理像素
                    // cosmic-text 的 glyph.x 是字形原点的 X 位置
                    // bearing_x 是纹理左边缘相对于字形原点的偏移
                    let glyph_x = line_x + glyph.x * scale + scaled_bearing_x;
                    // glyph.y 是基线位置，减去 bearing_y 得到字形顶边缘位置
                    let glyph_y = glyph.y * scale - scaled_bearing_y;

                    let word = line.words.get(glyph.word_index);
                    let (word_start, word_end) = word
                        .map(|w| (w.start_ms as f32, w.end_ms as f32))
                        .unwrap_or((0.0, 0.0));

                    let (word_pixel_width, word_start_x) =
                        if glyph.word_index < shaped.word_bounds.len() {
                            let (start, end) = shaped.word_bounds[glyph.word_index];
                            (end - start, start)
                        } else {
                            (glyph.advance, glyph.x)
                        };

                    let emphasize = word
                        .map(|w| w.emphasize || w.should_emphasize())
                        .unwrap_or(false);

                    let emphasis_progress = if emphasize && word_end > word_start {
                        let progress = (current_time_ms - word_start) / (word_end - word_start);
                        progress.clamp(0.0, 1.0)
                    } else {
                        0.0
                    };

                    let base_vertex = all_vertices.len() as u32;

                    let word_text = word.map(|w| &w.text).map(|t| t.as_str()).unwrap_or("");
                    let char_count = word_text.chars().count().max(1) as f32;
                    let char_index = (glyph.pos_in_word * char_count).floor();
                    let word_duration = word_end - word_start;
                    let word_delay = word_start;
                    let char_delay_offset = if char_count > 1.0 {
                        (word_duration / 2.5 / char_count) * char_index
                    } else {
                        0.0
                    };
                    let char_delay_ms = word_delay + char_delay_offset;

                    // Calculate glyph position within word for pixel-level gradient
                    let glyph_left_x = glyph.x;
                    let glyph_start_in_word = if word_pixel_width > 0.0 {
                        ((glyph_left_x - word_start_x) / word_pixel_width).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    // 使用 advance 宽度计算渐变比例
                    let glyph_width_ratio = if word_pixel_width > 0.0 {
                        (glyph.advance / word_pixel_width).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };

                    let mut base = LyricGlyphVertex {
                        pos_x: glyph_x,
                        pos_y: glyph_y,
                        width: scaled_width,
                        height: scaled_height,
                        uv_min: glyph_info.uv_min,
                        uv_max: glyph_info.uv_max,
                        word_start_ms: word_start,
                        word_end_ms: word_end,
                        glyph_start_in_word,
                        glyph_width_ratio,
                        line_index: line_idx as u32,
                        flags: 0,
                        color: 0xFFFFFFFF,
                        emphasis_progress,
                        corner_x: 0.0,
                        corner_y: 0.0,
                        char_index,
                        char_count,
                        char_delay_ms,
                        word_duration_ms: word_duration,
                        // Pack visual line info: lower 16 bits = index, upper 16 bits = count
                        visual_line_info: (glyph.visual_line_index & 0xFFFF)
                            | ((glyph.visual_line_count & 0xFFFF) << 16),
                        pos_in_visual_line: glyph.pos_in_visual_line,
                    };

                    base.set_active(style.is_active);
                    base.set_emphasize(emphasize);
                    base.set_bg(line.is_bg);
                    base.set_duet(line.is_duet);

                    for (cx, cy) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                        let mut v = base;
                        v.corner_x = cx;
                        v.corner_y = cy;
                        all_vertices.push(v);
                    }

                    all_indices.extend_from_slice(&[
                        base_vertex,
                        base_vertex + 1,
                        base_vertex + 2,
                        base_vertex,
                        base_vertex + 2,
                        base_vertex + 3,
                    ]);
                }

                // Add translation text
                if let Some(ref translated) = line.translated {
                    if !translated.is_empty() {
                        // Use FontSizeConfig default ratio (0.55) for translation
                        // Use LOGICAL font size for shape_simple (same as LyricsEngine)
                        let logical_trans_font_size = (logical_font_size * 0.55).max(10.0);
                        let trans_shaped = self.text_shaper.shape_simple(
                            translated,
                            logical_trans_font_size,
                            logical_content_width,
                        );

                        // Y offset in physical pixels (shaped.height is logical, multiply by scale)
                        let trans_y_offset = shaped.height * scale;
                        // X position in physical pixels
                        let trans_x = if line.is_duet {
                            viewport_width - trans_shaped.width * scale - padding_right
                        } else {
                            padding_left
                        };

                        // Physical font size for rendering
                        let trans_font_size = logical_trans_font_size * scale;

                        self.add_simple_text_glyphs_to_line(
                            queue,
                            &mut all_vertices,
                            &mut all_indices,
                            &trans_shaped,
                            trans_x,
                            trans_y_offset,
                            line_idx,
                            &style,
                            true,
                            false,
                            trans_font_size,
                            scale,
                        );
                    }
                }

                // Add romanized text
                if let Some(ref romanized) = line.romanized {
                    if !romanized.is_empty() {
                        // Use FontSizeConfig default ratio (0.45) for romanized
                        // Use LOGICAL font size for shape_simple (same as LyricsEngine)
                        let logical_roman_font_size = (logical_font_size * 0.45).max(10.0);
                        let roman_shaped = self.text_shaper.shape_simple(
                            romanized,
                            logical_roman_font_size,
                            logical_content_width,
                        );

                        // Calculate Y offset: main height + translation height (if any)
                        // Use actual shaped height for translation, not fixed value
                        let logical_trans_font_size = (logical_font_size * 0.55).max(10.0);
                        let trans_height = if let Some(ref translated) = line.translated {
                            if !translated.is_empty() {
                                let trans_shaped = self.text_shaper.shape_simple(
                                    translated,
                                    logical_trans_font_size,
                                    logical_content_width,
                                );
                                trans_shaped.height * scale // Convert to physical pixels
                            } else {
                                0.0
                            }
                        } else {
                            0.0
                        };
                        // Y offset in physical pixels
                        let roman_y_offset = shaped.height * scale + trans_height;
                        // X position in physical pixels
                        let roman_x = if line.is_duet {
                            viewport_width - roman_shaped.width * scale - padding_right
                        } else {
                            padding_left
                        };

                        // Physical font size for rendering
                        let roman_font_size = logical_roman_font_size * scale;

                        self.add_simple_text_glyphs_to_line(
                            queue,
                            &mut all_vertices,
                            &mut all_indices,
                            &roman_shaped,
                            roman_x,
                            roman_y_offset,
                            line_idx,
                            &style,
                            false,
                            true,
                            roman_font_size,
                            scale,
                        );
                    }
                }
            }

            // 记录这行的索引范围
            let index_count = all_indices.len() as u32 - start_index;
            line_index_ranges.push((start_index, index_count));

            // 记录行渲染信息
            line_render_info.push(LineRenderInfo {
                line_index: line_idx,
                blur_level: style.blur,
                y_position: style.y_position,
                height: line_height,
                visible,
                index_range: (start_index, index_count),
            });
        }

        // 更新缓存
        *self.line_index_ranges.write() = line_index_ranges;
        *self.cached_line_render_info.write() = line_render_info;

        (all_vertices, all_indices)
    }

    /// Build geometry from pre-shaped lines (Single Source of Truth)
    ///
    /// This method uses CachedShapedLine from LyricsEngine instead of calling shape_line.
    /// This ensures consistent text layout between CPU layout and GPU rendering.
    ///
    /// Key differences from build_geometry_per_line:
    /// - No shape_line calls - uses pre-computed glyph positions
    /// - Glyph positions are in LOGICAL pixels, scaled to physical for rendering
    /// - Translation and romanized text also use pre-shaped data
    #[allow(clippy::too_many_arguments)]
    fn build_geometry_from_shaped(
        &mut self,
        queue: &Queue,
        lines: &[LyricLineData],
        shaped_lines: &[CachedShapedLine],
        line_styles: &[ComputedLineStyle],
        viewport_width: f32,  // Physical pixels
        viewport_height: f32, // Physical pixels
        font_size: f32,       // Physical pixels
        current_time_ms: f32,
        scale: f32, // Scale factor (physical / logical)
    ) -> (Vec<LyricGlyphVertex>, Vec<u32>) {
        let mut all_vertices = Vec::with_capacity(MAX_GLYPHS * 4);
        let mut all_indices = Vec::with_capacity(MAX_GLYPHS * 6);
        let mut line_index_ranges = Vec::with_capacity(lines.len());
        let mut line_render_info = Vec::with_capacity(lines.len());

        let has_duet_line = lines.iter().any(|l| l.is_duet);
        let base_padding = viewport_width * 0.05;
        let overscan_px = 300.0;

        // SDF base size for scaling
        let sdf_base_size = 64.0_f32;
        let sdf_scale = font_size / sdf_base_size;

        // Translation and romanized font size ratios (must match LyricsEngine)
        let trans_ratio = 0.55_f32;
        let roman_ratio = 0.45_f32;

        for (line_idx, line) in lines.iter().enumerate() {
            let style = line_styles.get(line_idx).cloned().unwrap_or_default();

            // Get pre-shaped data for this line
            let shaped_line = shaped_lines.get(line_idx);

            let line_height = shaped_line
                .map(|s| s.total_height * scale)
                .unwrap_or(font_size * 1.4);

            let visible = style.opacity >= 0.01
                && LyricLineData::is_in_sight(
                    style.y_position,
                    line_height,
                    viewport_height,
                    overscan_px,
                );

            // 记录这行的起始索引
            let start_index = all_indices.len() as u32;

            if visible {
                if let Some(cached) = shaped_line {
                    // Calculate padding in physical pixels
                    let (padding_left, padding_right) = if has_duet_line {
                        if line.is_duet {
                            (viewport_width * 0.15, base_padding)
                        } else {
                            (base_padding, viewport_width * 0.15)
                        }
                    } else {
                        (base_padding, base_padding)
                    };

                    // Line X position in physical pixels
                    // shaped.width is in logical pixels, multiply by scale
                    let line_x = if line.is_duet {
                        viewport_width - cached.main.width * scale - padding_right
                    } else {
                        padding_left
                    };

                    // Debug logging for first line only
                    let should_log_debug = self.font_config.debug_logging && line_idx == 0;

                    // Add glyphs for main text using pre-shaped data
                    for glyph in &cached.main.glyphs {
                        let glyph_info = match self.sdf_cache.get_glyph(queue, glyph.cache_key) {
                            Some(info) => info,
                            None => continue,
                        };

                        if glyph_info.width == 0 || glyph_info.height == 0 {
                            continue;
                        }

                        // SDF metrics scaled to actual font size
                        let scaled_width = glyph_info.width as f32 * sdf_scale;
                        let scaled_height = glyph_info.height as f32 * sdf_scale;
                        let scaled_bearing_x = glyph_info.offset_x as f32 * sdf_scale;
                        let scaled_bearing_y = glyph_info.offset_y as f32 * sdf_scale;

                        // Glyph position: logical pixels * scale = physical pixels
                        let glyph_x = line_x + glyph.x * scale + scaled_bearing_x;
                        let glyph_y = glyph.y * scale - scaled_bearing_y;

                        if should_log_debug {
                            tracing::debug!(
                                "[build_from_shaped] glyph.x={:.2}, bearing_x={:.2}, glyph_x={:.2}",
                                glyph.x,
                                scaled_bearing_x,
                                glyph_x
                            );
                        }

                        let word = line.words.get(glyph.word_index);
                        let (word_start, word_end) = word
                            .map(|w| (w.start_ms as f32, w.end_ms as f32))
                            .unwrap_or((0.0, 0.0));

                        let (word_pixel_width, word_start_x) =
                            if glyph.word_index < cached.main.word_bounds.len() {
                                let (start, end) = cached.main.word_bounds[glyph.word_index];
                                (end - start, start)
                            } else {
                                (glyph.advance, glyph.x)
                            };

                        let emphasize = word
                            .map(|w| w.emphasize || w.should_emphasize())
                            .unwrap_or(false);

                        let emphasis_progress = if emphasize && word_end > word_start {
                            let progress = (current_time_ms - word_start) / (word_end - word_start);
                            progress.clamp(0.0, 1.0)
                        } else {
                            0.0
                        };

                        let base_vertex = all_vertices.len() as u32;

                        let word_text = word.map(|w| &w.text).map(|t| t.as_str()).unwrap_or("");
                        let char_count = word_text.chars().count().max(1) as f32;
                        let char_index = (glyph.pos_in_word * char_count).floor();
                        let word_duration = word_end - word_start;
                        let word_delay = word_start;
                        let char_delay_offset = if char_count > 1.0 {
                            (word_duration / 2.5 / char_count) * char_index
                        } else {
                            0.0
                        };
                        let char_delay_ms = word_delay + char_delay_offset;

                        let glyph_left_x = glyph.x;
                        let glyph_start_in_word = if word_pixel_width > 0.0 {
                            ((glyph_left_x - word_start_x) / word_pixel_width).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        let glyph_width_ratio = if word_pixel_width > 0.0 {
                            (glyph.advance / word_pixel_width).clamp(0.0, 1.0)
                        } else {
                            1.0
                        };

                        let mut base = LyricGlyphVertex {
                            pos_x: glyph_x,
                            pos_y: glyph_y,
                            width: scaled_width,
                            height: scaled_height,
                            uv_min: glyph_info.uv_min,
                            uv_max: glyph_info.uv_max,
                            word_start_ms: word_start,
                            word_end_ms: word_end,
                            glyph_start_in_word,
                            glyph_width_ratio,
                            line_index: line_idx as u32,
                            flags: 0,
                            color: 0xFFFFFFFF,
                            emphasis_progress,
                            corner_x: 0.0,
                            corner_y: 0.0,
                            char_index,
                            char_count,
                            char_delay_ms,
                            word_duration_ms: word_duration,
                            visual_line_info: (glyph.visual_line_index & 0xFFFF)
                                | ((glyph.visual_line_count & 0xFFFF) << 16),
                            pos_in_visual_line: glyph.pos_in_visual_line,
                        };

                        base.set_active(style.is_active);
                        base.set_emphasize(emphasize);
                        base.set_bg(line.is_bg);
                        base.set_duet(line.is_duet);

                        for (cx, cy) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                            let mut v = base;
                            v.corner_x = cx;
                            v.corner_y = cy;
                            all_vertices.push(v);
                        }

                        all_indices.extend_from_slice(&[
                            base_vertex,
                            base_vertex + 1,
                            base_vertex + 2,
                            base_vertex,
                            base_vertex + 2,
                            base_vertex + 3,
                        ]);
                    }

                    // Add translation text using pre-shaped data
                    if let Some(ref trans_shaped) = cached.translation {
                        let trans_y_offset = cached.main.height * scale;
                        let trans_x = if line.is_duet {
                            viewport_width - trans_shaped.width * scale - padding_right
                        } else {
                            padding_left
                        };

                        let trans_font_size = font_size * trans_ratio;
                        let trans_sdf_scale = trans_font_size / sdf_base_size;

                        self.add_shaped_glyphs_to_line(
                            queue,
                            &mut all_vertices,
                            &mut all_indices,
                            trans_shaped,
                            trans_x,
                            trans_y_offset,
                            line_idx,
                            &style,
                            true,
                            false,
                            trans_sdf_scale,
                            scale,
                        );
                    }

                    // Add romanized text using pre-shaped data
                    if let Some(ref roman_shaped) = cached.romanized {
                        let trans_height = cached
                            .translation
                            .as_ref()
                            .map(|t| t.height * scale)
                            .unwrap_or(0.0);
                        let roman_y_offset = cached.main.height * scale + trans_height;
                        let roman_x = if line.is_duet {
                            viewport_width - roman_shaped.width * scale - padding_right
                        } else {
                            padding_left
                        };

                        let roman_font_size = font_size * roman_ratio;
                        let roman_sdf_scale = roman_font_size / sdf_base_size;

                        self.add_shaped_glyphs_to_line(
                            queue,
                            &mut all_vertices,
                            &mut all_indices,
                            roman_shaped,
                            roman_x,
                            roman_y_offset,
                            line_idx,
                            &style,
                            false,
                            true,
                            roman_sdf_scale,
                            scale,
                        );
                    }
                }
            }

            // 记录这行的索引范围
            let index_count = all_indices.len() as u32 - start_index;
            line_index_ranges.push((start_index, index_count));

            // 记录行渲染信息
            line_render_info.push(LineRenderInfo {
                line_index: line_idx,
                blur_level: style.blur,
                y_position: style.y_position,
                height: line_height,
                visible,
                index_range: (start_index, index_count),
            });
        }

        // 更新缓存
        *self.line_index_ranges.write() = line_index_ranges;
        *self.cached_line_render_info.write() = line_render_info;

        (all_vertices, all_indices)
    }

    /// Add glyphs from pre-shaped line data (for translation/romanized)
    #[allow(clippy::too_many_arguments)]
    fn add_shaped_glyphs_to_line(
        &mut self,
        queue: &Queue,
        vertices: &mut Vec<LyricGlyphVertex>,
        indices: &mut Vec<u32>,
        shaped: &ShapedLine,
        base_x: f32,   // Physical pixels
        y_offset: f32, // Physical pixels
        line_idx: usize,
        style: &ComputedLineStyle,
        is_translation: bool,
        is_romanized: bool,
        sdf_scale: f32, // SDF scale factor (font_size / 64.0)
        scale: f32,     // Logical to physical scale factor
    ) {
        for glyph in &shaped.glyphs {
            let glyph_info = match self.sdf_cache.get_glyph(queue, glyph.cache_key) {
                Some(info) => info,
                None => continue,
            };

            if glyph_info.width == 0 || glyph_info.height == 0 {
                continue;
            }

            let scaled_width = glyph_info.width as f32 * sdf_scale;
            let scaled_height = glyph_info.height as f32 * sdf_scale;
            let scaled_bearing_x = glyph_info.offset_x as f32 * sdf_scale;
            let scaled_bearing_y = glyph_info.offset_y as f32 * sdf_scale;

            // Glyph position: logical pixels * scale = physical pixels
            let glyph_x = base_x + glyph.x * scale + scaled_bearing_x;
            let glyph_y = y_offset + glyph.y * scale - scaled_bearing_y;

            let base_vertex = vertices.len() as u32;

            let mut base = LyricGlyphVertex {
                pos_x: glyph_x,
                pos_y: glyph_y,
                width: scaled_width,
                height: scaled_height,
                uv_min: glyph_info.uv_min,
                uv_max: glyph_info.uv_max,
                word_start_ms: 0.0,
                word_end_ms: 0.0,
                glyph_start_in_word: 0.0,
                glyph_width_ratio: 1.0,
                line_index: line_idx as u32,
                flags: 0,
                color: 0xCCCCCCFF,
                emphasis_progress: 0.0,
                corner_x: 0.0,
                corner_y: 0.0,
                char_index: 0.0,
                char_count: 1.0,
                char_delay_ms: 0.0,
                word_duration_ms: 0.0,
                visual_line_info: (glyph.visual_line_index & 0xFFFF)
                    | ((glyph.visual_line_count & 0xFFFF) << 16),
                pos_in_visual_line: glyph.pos_in_visual_line,
            };

            base.set_active(style.is_active);
            base.set_translation(is_translation);
            base.set_romanized(is_romanized);

            for (cx, cy) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                let mut v = base;
                v.corner_x = cx;
                v.corner_y = cy;
                vertices.push(v);
            }

            indices.extend_from_slice(&[
                base_vertex,
                base_vertex + 1,
                base_vertex + 2,
                base_vertex,
                base_vertex + 2,
                base_vertex + 3,
            ]);
        }
    }

    /// Add glyphs for simple text (translation/romanized) to a line
    #[allow(clippy::too_many_arguments)]
    fn add_simple_text_glyphs_to_line(
        &mut self,
        queue: &Queue,
        vertices: &mut Vec<LyricGlyphVertex>,
        indices: &mut Vec<u32>,
        shaped: &super::text_shaper::ShapedLine,
        base_x: f32,   // Physical pixels
        y_offset: f32, // Physical pixels
        line_idx: usize,
        style: &ComputedLineStyle,
        is_translation: bool,
        is_romanized: bool,
        font_size: f32, // Physical pixels
        scale: f32,     // Scale factor for logical to physical conversion
    ) {
        // SDF 纹理是在 base_size (64px) 下生成的，需要缩放到实际字号
        let sdf_base_size = 64.0_f32;
        let sdf_scale = font_size / sdf_base_size;

        for glyph in &shaped.glyphs {
            let glyph_info = match self.sdf_cache.get_glyph(queue, glyph.cache_key) {
                Some(info) => info,
                None => continue,
            };

            if glyph_info.width == 0 || glyph_info.height == 0 {
                continue;
            }

            // 所有 SDF 度量统一乘以 sdf_scale
            let scaled_width = glyph_info.width as f32 * sdf_scale;
            let scaled_height = glyph_info.height as f32 * sdf_scale;
            let scaled_bearing_x = glyph_info.offset_x as f32 * sdf_scale;
            let scaled_bearing_y = glyph_info.offset_y as f32 * sdf_scale;

            // 字形位置：glyph.x/y 是逻辑像素
            let glyph_x = base_x + glyph.x * scale + scaled_bearing_x;
            let glyph_y = y_offset + glyph.y * scale - scaled_bearing_y;

            let base_vertex = vertices.len() as u32;

            let mut base = LyricGlyphVertex {
                pos_x: glyph_x,
                pos_y: glyph_y,
                width: scaled_width,
                height: scaled_height,
                uv_min: glyph_info.uv_min,
                uv_max: glyph_info.uv_max,
                word_start_ms: 0.0,
                word_end_ms: 0.0,
                glyph_start_in_word: 0.0,
                glyph_width_ratio: 1.0,
                line_index: line_idx as u32,
                flags: 0,
                color: 0xCCCCCCFF,
                emphasis_progress: 0.0,
                corner_x: 0.0,
                corner_y: 0.0,
                char_index: 0.0,
                char_count: 1.0,
                char_delay_ms: 0.0,
                word_duration_ms: 0.0,
                // Pack visual line info: lower 16 bits = index, upper 16 bits = count
                visual_line_info: (glyph.visual_line_index & 0xFFFF)
                    | ((glyph.visual_line_count & 0xFFFF) << 16),
                pos_in_visual_line: glyph.pos_in_visual_line,
            };

            base.set_active(style.is_active);
            base.set_translation(is_translation);
            base.set_romanized(is_romanized);

            for (cx, cy) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                let mut v = base;
                v.corner_x = cx;
                v.corner_y = cy;
                vertices.push(v);
            }

            indices.extend_from_slice(&[
                base_vertex,
                base_vertex + 1,
                base_vertex + 2,
                base_vertex,
                base_vertex + 2,
                base_vertex + 3,
            ]);
        }
    }

    /// Add glyphs for simple text (translation/romanized) to a specific blur_group
    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    fn add_simple_text_glyphs_to_group(
        &mut self,
        queue: &Queue,
        vertices: &mut Vec<LyricGlyphVertex>,
        indices: &mut Vec<u32>,
        shaped: &super::text_shaper::ShapedLine,
        base_x: f32,
        y_offset: f32,
        line_idx: usize,
        style: &ComputedLineStyle,
        is_translation: bool,
        is_romanized: bool,
        font_size: f32,
    ) {
        // SDF 纹理是在 base_size (64px) 下生成的，需要缩放到实际字号
        let sdf_base_size = 64.0_f32;
        let sdf_scale = font_size / sdf_base_size;

        for glyph in &shaped.glyphs {
            let glyph_info = match self.sdf_cache.get_glyph(queue, glyph.cache_key) {
                Some(info) => info,
                None => continue,
            };

            if glyph_info.width == 0 || glyph_info.height == 0 {
                continue;
            }

            // 所有 SDF 度量统一乘以 sdf_scale
            let scaled_width = glyph_info.width as f32 * sdf_scale;
            let scaled_height = glyph_info.height as f32 * sdf_scale;
            let scaled_bearing_x = glyph_info.offset_x as f32 * sdf_scale;
            let scaled_bearing_y = glyph_info.offset_y as f32 * sdf_scale;

            // 字形位置
            let glyph_x = base_x + glyph.x + scaled_bearing_x;
            let glyph_y = y_offset + glyph.y - scaled_bearing_y;

            let base_vertex = vertices.len() as u32;

            let mut base = LyricGlyphVertex {
                pos_x: glyph_x,
                pos_y: glyph_y,
                width: scaled_width,
                height: scaled_height,
                uv_min: glyph_info.uv_min,
                uv_max: glyph_info.uv_max,
                word_start_ms: 0.0,
                word_end_ms: 0.0,
                glyph_start_in_word: 0.0,
                glyph_width_ratio: 1.0,
                line_index: line_idx as u32,
                flags: 0,
                color: 0xCCCCCCFF,
                emphasis_progress: 0.0,
                corner_x: 0.0,
                corner_y: 0.0,
                char_index: 0.0,
                char_count: 1.0,
                char_delay_ms: 0.0,
                word_duration_ms: 0.0,
                // Pack visual line info: lower 16 bits = index, upper 16 bits = count
                visual_line_info: (glyph.visual_line_index & 0xFFFF)
                    | ((glyph.visual_line_count & 0xFFFF) << 16),
                pos_in_visual_line: glyph.pos_in_visual_line,
            };

            base.set_active(style.is_active);
            base.set_translation(is_translation);
            base.set_romanized(is_romanized);

            for (cx, cy) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                let mut v = base;
                v.corner_x = cx;
                v.corner_y = cy;
                vertices.push(v);
            }

            indices.extend_from_slice(&[
                base_vertex,
                base_vertex + 1,
                base_vertex + 2,
                base_vertex,
                base_vertex + 2,
                base_vertex + 3,
            ]);
        }
    }

    /// Update bind group with current atlas
    fn update_bind_group(&mut self, device: &Device) {
        let atlas_view = self.sdf_cache.atlas_view();

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SDF Glyph Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lyrics Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.global_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.line_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        }));
    }

    /// Direct render to target (no blur)
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if self.index_count == 0 {
            return;
        }

        let Some(bind_group) = &self.bind_group else {
            return;
        };

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..self.index_count, 0, 0..1);
    }

    /// Render to MRT targets (color + blur_info)
    fn render_mrt<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if self.index_count == 0 {
            return;
        }

        let Some(bind_group) = &self.bind_group else {
            return;
        };

        render_pass.set_pipeline(&self.mrt_pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..self.index_count, 0, 0..1);
    }

    /// Prepare interlude dots for rendering
    pub fn prepare_interlude_dots(
        &mut self,
        device: &Device,
        queue: &Queue,
        dots: &InterludeDots,
        viewport_width: f32,
        viewport_height: f32,
        bounds_x: f32,
        bounds_y: f32,
        scale_factor: f32,
    ) {
        self.dots_enabled = dots.enabled && dots.scale > 0.01;

        if !self.dots_enabled {
            return;
        }

        let dots_uniform = DotsUniform::from_interlude_dots(
            dots,
            [viewport_width, viewport_height],
            [bounds_x, bounds_y],
            scale_factor,
        );
        queue.write_buffer(
            &self.dots_uniform_buffer,
            0,
            bytemuck::bytes_of(&dots_uniform),
        );

        self.dots_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Interlude Dots Bind Group"),
            layout: &self.dots_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.dots_uniform_buffer.as_entire_binding(),
            }],
        }));
    }

    /// Render interlude dots
    pub fn render_interlude_dots<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if !self.dots_enabled {
            return;
        }

        let Some(bind_group) = &self.dots_bind_group else {
            return;
        };

        render_pass.set_pipeline(&self.dots_pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.draw(0..4, 0..3);
    }

    /// Clear glyph cache
    #[allow(dead_code)]
    pub fn clear_cache(&mut self, queue: &Queue) {
        self.sdf_cache.clear(queue);
    }

    /// Enable or disable blur effect
    #[allow(dead_code)]
    pub fn set_enable_blur(&mut self, enable: bool) {
        self.enable_blur = enable;
    }

    /// Check if blur is enabled
    #[allow(dead_code)]
    pub fn is_blur_enabled(&self) -> bool {
        self.enable_blur
    }

    /// Prepare blur rendering resources
    ///
    /// This must be called in the prepare phase to set up textures and bind groups
    /// that will be used in the render phase.
    pub fn prepare_blur(
        &mut self,
        device: &Device,
        _queue: &Queue,
        viewport_width: u32,
        viewport_height: u32,
        _current_time_ms: f32,
        _font_size: f32,
    ) {
        // Cache viewport info
        *self.cached_viewport.write() = (viewport_width, viewport_height);

        // Ensure textures exist for per-line blur rendering
        self.ensure_textures(device, viewport_width, viewport_height);
    }

    /// Render with per-line blur effect (correct Apple Music-style blur)
    ///
    /// 正确的 Apple Music 风格逐行模糊渲染：
    /// 1. 每行歌词独立渲染到单独的纹理
    /// 2. 对每行纹理独立应用高斯模糊
    /// 3. 按从远到近的顺序合成到最终目标
    ///
    /// 这与 的 CSS `filter: blur(Npx)` 效果完全一致，
    /// 因为每行的模糊是独立的，不会与其他行混合。
    pub fn render_with_per_line_blur(
        &self,
        device: &Device,
        queue: &Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &iced::Rectangle<u32>,
        viewport_width: u32,
        viewport_height: u32,
    ) {
        if self.index_count == 0 {
            return;
        }

        let Some(bind_group) = &self.bind_group else {
            return;
        };

        // 获取缓存的行渲染信息
        let line_render_info = self.cached_line_render_info.read().clone();

        if line_render_info.is_empty() {
            return;
        }

        // 更新逐行模糊渲染器的视口尺寸
        self.per_line_blur
            .write()
            .set_viewport_size(viewport_width, viewport_height);

        // 使用逐行模糊渲染器
        self.per_line_blur.write().render_with_blur(
            device,
            queue,
            encoder,
            target,
            clip_bounds,
            &line_render_info,
            &self.pipeline,
            bind_group,
            &self.vertex_buffer,
            &self.index_buffer,
        );

        // 渲染间奏点（无模糊）
        if self.dots_enabled {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Interlude Dots Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_scissor_rect(
                clip_bounds.x,
                clip_bounds.y,
                clip_bounds.width,
                clip_bounds.height,
            );

            self.render_interlude_dots(&mut render_pass);
        }
    }

    /// 预生成 MSDF 字形（不需要 GPU，可在后台线程调用）
    ///
    /// 这个方法只生成位图并缓存，不上传到 GPU。
    /// 后续渲染时会使用预生成的位图，只需要上传到 GPU（快速操作）。
    ///
    /// 返回成功预生成的字形数量
    pub fn pre_generate_glyphs(&self, cache_keys: &[cosmic_text::CacheKey]) -> usize {
        self.sdf_cache.pre_generate_glyphs(cache_keys)
    }

    /// 获取 SDF 缓存的引用（用于外部预生成）
    pub fn sdf_cache(&self) -> &super::sdf_cache::SdfCache {
        &self.sdf_cache
    }
}
