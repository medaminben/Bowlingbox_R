//! Bind group layouts, shader modules and render pipelines.

use super::meshes::VERTEX_STRIDE;
use std::borrow::Cow;
use std::num::NonZeroU64;

/// The scene's own colour target: the shaders write display-ready sRGB bytes, as the C++ basic
/// path wrote them to the default framebuffer.
pub const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Floats in the WGSL `Frame` struct (common.wgsl), in its field order.
pub const FRAME_FLOATS: usize = 16 + 16 + 9 * 4 + 4 * 4 + 4 * 4 + 4 + 4;
/// Floats in the WGSL `Draw` struct.
pub const DRAW_FLOATS: usize = 16 + 16 + 3 * 4;
/// Distance between draws in the dynamic-offset uniform buffer: the largest
/// `min_uniform_buffer_offset_alignment` wgpu allows, so it suits every adapter.
pub const DRAW_STRIDE: u64 = 256;

pub const TONEMAP: &str = include_str!("../shaders/tonemap.wgsl");
const COMMON: &str = include_str!("../shaders/common.wgsl");

fn shader(device: &wgpu::Device, label: &str, body: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(Cow::Owned(format!("{TONEMAP}\n{COMMON}\n{body}"))),
    })
}

fn uniform_layout(
    device: &wgpu::Device,
    label: &str,
    floats: usize,
    dynamic: bool,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: dynamic,
                min_binding_size: NonZeroU64::new(floats as u64 * 4),
            },
            count: None,
        }],
    })
}

pub struct Pipelines {
    pub frame_layout: wgpu::BindGroupLayout,
    pub draw_layout: wgpu::BindGroupLayout,
    pub blit_layout: wgpu::BindGroupLayout,
    /// The frame and draw uniforms, for every pipeline that draws the scene.
    pub scene_layout: wgpu::PipelineLayout,
    /// The basic path's scene pipelines, which tone map each fragment.
    pub scene: ScenePipelines,
    pub blit: wgpu::RenderPipeline,
}

/// The pipelines that draw the scene's geometry and sky into one colour format.
pub struct ScenePipelines {
    pub lit: wgpu::RenderPipeline,
    pub sky: wgpu::RenderPipeline,
    pub guide: wgpu::RenderPipeline,
}

/// Where the scene pipelines write.
#[derive(Clone, Copy)]
pub enum SceneOutput {
    /// Tone mapped sRGB bytes in `SCENE_FORMAT` (basic path).
    Display,
    /// Linear HDR (full path).
    Hdr(wgpu::TextureFormat),
}

/// One pipeline that draws into the scene targets.
struct ScenePass<'a> {
    label: &'a str,
    module: &'a wgpu::ShaderModule,
    vertex: &'a str,
    fragment: &'a str,
    /// Mesh vertices, or none for a generated triangle.
    meshes: bool,
    cull: Option<wgpu::Face>,
    depth_write: bool,
    depth_compare: wgpu::CompareFunction,
    blend: Option<wgpu::BlendState>,
}

fn scene_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    samples: u32,
    p: &ScenePass<'_>,
) -> wgpu::RenderPipeline {
    let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
    let mesh_buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: VERTEX_STRIDE,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &attributes,
    })];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(p.label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: p.module,
            entry_point: Some(p.vertex),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: if p.meshes { &mesh_buffers } else { &[] },
        },
        // Meshes wind counter-clockwise seen from outside in world space; the left-handed camera
        // mirrors that to clockwise on screen.
        primitive: wgpu::PrimitiveState {
            front_face: wgpu::FrontFace::Cw,
            cull_mode: p.cull,
            ..wgpu::PrimitiveState::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(p.depth_write),
            depth_compare: Some(p.depth_compare),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: samples,
            ..wgpu::MultisampleState::default()
        },
        fragment: Some(wgpu::FragmentState {
            module: p.module,
            entry_point: Some(p.fragment),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: p.blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// The copy of the scene into the caller's pass, and the layout of its one texture.
fn blit_pipeline(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
    target_samples: u32,
) -> (wgpu::BindGroupLayout, wgpu::RenderPipeline) {
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("scene texture"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("blit"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("blit"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/blit.wgsl"))),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("blit"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_blit"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: target_samples,
            ..wgpu::MultisampleState::default()
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some(if target_format.is_srgb() {
                "fs_blit_linear"
            } else {
                "fs_blit_encoded"
            }),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    (layout, pipeline)
}

impl ScenePipelines {
    pub fn new(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        output: SceneOutput,
        samples: u32,
    ) -> Self {
        let (format, hdr) = match output {
            SceneOutput::Display => (SCENE_FORMAT, ""),
            SceneOutput::Hdr(format) => (format, "_hdr"),
        };
        let lit = shader(device, "lit", include_str!("../shaders/lit.wgsl"));
        let sky = shader(device, "sky", include_str!("../shaders/sky.wgsl"));
        let guide = shader(device, "guide", include_str!("../shaders/guide.wgsl"));
        let build = |p: &ScenePass<'_>| scene_pipeline(device, layout, format, samples, p);
        Self {
            lit: build(&ScenePass {
                label: "lit",
                module: &lit,
                vertex: "vs_mesh",
                fragment: &format!("fs_lit{hdr}"),
                meshes: true,
                cull: Some(wgpu::Face::Back),
                depth_write: true,
                depth_compare: wgpu::CompareFunction::Less,
                blend: None,
            }),
            sky: build(&ScenePass {
                label: "sky",
                module: &sky,
                vertex: "vs_sky",
                fragment: &format!("fs_sky{hdr}"),
                meshes: false,
                cull: None,
                depth_write: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                blend: None,
            }),
            // No culling, as in C++: the thin box's far faces blend in too.
            guide: build(&ScenePass {
                label: "aim guide",
                module: &guide,
                vertex: "vs_mesh",
                fragment: &format!("fs_guide{hdr}"),
                meshes: true,
                cull: None,
                depth_write: false,
                depth_compare: wgpu::CompareFunction::Less,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            }),
        }
    }
}

impl Pipelines {
    /// `scene_samples` is the scene targets' sample count; `target_format` and `target_samples`
    /// describe the caller's render pass that `paint` draws into.
    pub fn new(
        device: &wgpu::Device,
        scene_samples: u32,
        target_format: wgpu::TextureFormat,
        target_samples: u32,
    ) -> Self {
        let frame_layout = uniform_layout(device, "frame uniforms", FRAME_FLOATS, false);
        let draw_layout = uniform_layout(device, "draw uniforms", DRAW_FLOATS, true);
        let scene_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene"),
            bind_group_layouts: &[Some(&frame_layout), Some(&draw_layout)],
            immediate_size: 0,
        });
        let scene = ScenePipelines::new(device, &scene_layout, SceneOutput::Display, scene_samples);
        let (blit_layout, blit) = blit_pipeline(device, target_format, target_samples);
        Self {
            frame_layout,
            draw_layout,
            blit_layout,
            scene_layout,
            scene,
            blit,
        }
    }
}
