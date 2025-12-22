//! 逐行渲染器 (Per-Line Renderer)
//!
//! SDF 版本：不再需要多 pass 模糊，模糊效果通过 shader 中的 smoothstep 实现。
//!
//! ## SDF 模糊原理
//!
//! SDF 的"模糊"不是传统的高斯模糊，而是边缘软化：
//! - 通过调整 smoothstep 的范围来控制边缘的锐利程度
//! - blur_level 越大，smoothstep 范围越大，边缘越模糊
//! - 这种方法在单 pass 中完成，性能更好
//!
//! ## 保留的功能
//!
//! - 逐行渲染信息结构（用于传递 blur_level 到 shader）
//! - 纹理池（用于合成）
//! - 合成管线（用于层叠渲染）

use wgpu::{Device, TextureFormat};

/// 单行渲染纹理
struct LineTexture {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// 纹理尺寸
    width: u32,
    height: u32,
    /// 是否正在使用
    in_use: bool,
}

impl LineTexture {
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
        Self {
            texture,
            view,
            width,
            height,
            in_use: false,
        }
    }

    fn matches_size(&self, width: u32, height: u32) -> bool {
        self.width == width && self.height == height
    }
}

/// 行渲染信息
#[derive(Debug, Clone)]
pub struct LineRenderInfo {
    /// 行索引
    pub line_index: usize,
    /// 模糊级别 (像素) - SDF 中用于控制 smoothstep 范围
    pub blur_level: f32,
    /// Y 位置 (物理像素)
    pub y_position: f32,
    /// 行高度 (物理像素)
    pub height: f32,
    /// 是否可见
    pub visible: bool,
    /// 顶点索引范围 (start, count)
    pub index_range: (u32, u32),
}

/// 逐行渲染器 (SDF 版本)
///
/// SDF 渲染不需要多 pass 模糊，模糊效果在 shader 中通过 smoothstep 实现。
/// 这个结构保留用于：
/// - 管理渲染纹理池
/// - 提供合成管线
/// - 传递行渲染信息
pub struct PerLineBlurRenderer {
    /// 纹理池
    texture_pool: Vec<LineTexture>,
    /// 合成管线
    composite_pipeline: wgpu::RenderPipeline,
    composite_bind_group_layout: wgpu::BindGroupLayout,
    composite_sampler: wgpu::Sampler,
    /// 纹理格式
    format: TextureFormat,
    /// 当前视口尺寸
    viewport_size: (u32, u32),
}

impl PerLineBlurRenderer {
    pub fn new(device: &Device, format: TextureFormat) -> Self {
        // 创建合成管线
        let composite_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Per-Line Composite Bind Group Layout"),
                entries: &[
                    // 纹理
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    // 采样器
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let composite_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Per-Line Composite Pipeline Layout"),
                bind_group_layouts: &[&composite_bind_group_layout],
                push_constant_ranges: &[],
            });

        let composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Per-Line Composite Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/layer_composite.wgsl").into()),
        });

        let composite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Per-Line Composite Pipeline"),
            layout: Some(&composite_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &composite_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &composite_shader,
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
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let composite_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Per-Line Composite Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            texture_pool: Vec::new(),
            composite_pipeline,
            composite_bind_group_layout,
            composite_sampler,
            format,
            viewport_size: (0, 0),
        }
    }

    /// 从池中获取或创建纹理
    fn acquire_texture(
        pool: &mut Vec<LineTexture>,
        device: &Device,
        width: u32,
        height: u32,
        format: TextureFormat,
        label_prefix: &str,
    ) -> usize {
        // 查找可用的匹配尺寸的纹理
        for (i, tex) in pool.iter_mut().enumerate() {
            if !tex.in_use && tex.matches_size(width, height) {
                tex.in_use = true;
                return i;
            }
        }

        // 查找可用的任意纹理（需要重新创建）
        for (i, tex) in pool.iter_mut().enumerate() {
            if !tex.in_use {
                *tex = LineTexture::new(
                    device,
                    width,
                    height,
                    format,
                    &format!("{} {}", label_prefix, i),
                );
                tex.in_use = true;
                return i;
            }
        }

        // 创建新纹理
        let idx = pool.len();
        let mut tex = LineTexture::new(
            device,
            width,
            height,
            format,
            &format!("{} {}", label_prefix, idx),
        );
        tex.in_use = true;
        pool.push(tex);
        idx
    }

    /// 释放所有纹理（标记为未使用）
    fn release_all_textures(&mut self) {
        for tex in &mut self.texture_pool {
            tex.in_use = false;
        }
    }

    /// 更新视口尺寸
    pub fn set_viewport_size(&mut self, width: u32, height: u32) {
        self.viewport_size = (width, height);
    }

    /// SDF 渲染所有行
    ///
    /// SDF 版本：所有行直接渲染到目标，模糊效果在 shader 中通过 smoothstep 实现。
    /// blur_level 通过 LineUniform 传递给 shader。
    ///
    /// 参数：
    /// - lines: 行渲染信息列表
    /// - pipeline: 渲染管线
    /// - bind_group: 绑定组
    /// - vertex_buffer: 顶点缓冲区
    /// - index_buffer: 索引缓冲区
    #[allow(clippy::too_many_arguments)]
    pub fn render_with_blur(
        &mut self,
        _device: &Device,
        _queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &iced::Rectangle<u32>,
        lines: &[LineRenderInfo],
        pipeline: &wgpu::RenderPipeline,
        bind_group: &wgpu::BindGroup,
        vertex_buffer: &wgpu::Buffer,
        index_buffer: &wgpu::Buffer,
    ) {
        let viewport_width = self.viewport_size.0;
        let viewport_height = self.viewport_size.1;

        if viewport_width == 0 || viewport_height == 0 {
            return;
        }

        // 释放之前的纹理
        self.release_all_textures();

        // 收集可见行
        let visible_lines: Vec<&LineRenderInfo> = lines
            .iter()
            .filter(|line| line.visible && line.index_range.1 > 0)
            .collect();

        if visible_lines.is_empty() {
            return;
        }

        // SDF 渲染：所有行直接渲染到目标
        // 模糊效果在 shader 中通过 LineUniform.blur 和 smoothstep 实现
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("SDF Lines Render Pass"),
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

        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        // 按 blur_level 从大到小排序（远的先画，近的后画覆盖）
        let mut sorted_lines: Vec<_> = visible_lines.iter().collect();
        sorted_lines.sort_by(|a, b| {
            b.blur_level
                .partial_cmp(&a.blur_level)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for line in sorted_lines {
            let (start, count) = line.index_range;
            render_pass.draw_indexed(start..(start + count), 0, 0..1);
        }
    }

    /// 获取纹理池大小（用于调试）
    #[allow(dead_code)]
    pub fn pool_size(&self) -> usize {
        self.texture_pool.len()
    }

    /// 获取合成管线（用于外部合成）
    #[allow(dead_code)]
    pub fn composite_pipeline(&self) -> &wgpu::RenderPipeline {
        &self.composite_pipeline
    }

    /// 获取合成绑定组布局
    #[allow(dead_code)]
    pub fn composite_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.composite_bind_group_layout
    }

    /// 获取合成采样器
    #[allow(dead_code)]
    pub fn composite_sampler(&self) -> &wgpu::Sampler {
        &self.composite_sampler
    }

    /// 获取纹理视图（用于外部合成）
    #[allow(dead_code)]
    pub fn get_texture_view(&self, index: usize) -> Option<&wgpu::TextureView> {
        self.texture_pool.get(index).map(|t| &t.view)
    }

    /// 获取或创建纹理（公开方法）
    #[allow(dead_code)]
    pub fn acquire_texture_public(&mut self, device: &Device, width: u32, height: u32) -> usize {
        Self::acquire_texture(
            &mut self.texture_pool,
            device,
            width,
            height,
            self.format,
            "Line Texture",
        )
    }
}
