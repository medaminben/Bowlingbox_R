//! The full render path: the scene in linear HDR with MSAA, resolved and then tone mapped by a
//! composite pass into the same 8-bit output the basic path fills, with the key light's shadow
//! map and the lane reflection. Bloom comes later.

use super::capabilities::{HDR_FORMAT, catch_errors};
use super::pipelines::{
    Pipelines, SCENE_FORMAT, SceneOutput, ScenePipelines, TONEMAP, mirror_pipeline, shadow_pipeline,
};
use super::targets::{HdrTargets, Reflection, ShadowMap, TargetLayouts};
use std::borrow::Cow;
use std::num::NonZeroU64;

/// Floats in the WGSL `Post` struct (composite.wgsl).
const POST_FLOATS: usize = 4;

/// What the full path draws. Every pass is on by default; turning one off is for comparisons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FullPathOptions {
    /// The key light's shadow map.
    pub shadows: bool,
    /// The half-resolution mirror of the scene that the main lane shows.
    pub reflection: bool,
}

impl Default for FullPathOptions {
    fn default() -> Self {
        Self {
            shadows: true,
            reflection: true,
        }
    }
}

pub struct FullPath {
    samples: u32,
    /// Draws the scene into the HDR target; the lit pipeline samples the shadow map (group 2) and
    /// the reflection (group 3).
    scene: ScenePipelines,
    shadow: wgpu::RenderPipeline,
    shadow_map: ShadowMap,
    /// Draws the mirrored scene into the reflection target.
    mirror: wgpu::RenderPipeline,
    reflection_layout: wgpu::BindGroupLayout,
    reflection_sampler: wgpu::Sampler,
    resolve_layout: wgpu::BindGroupLayout,
    resolve: wgpu::RenderPipeline,
    composite_layout: wgpu::BindGroupLayout,
    composite: wgpu::RenderPipeline,
    post_buffer: wgpu::Buffer,
    targets: Option<HdrTargets>,
}

/// A pipeline that covers its target with one generated triangle.
fn post_pipeline(
    device: &wgpu::Device,
    label: &str,
    group: &wgpu::BindGroupLayout,
    source: String,
    fragment: &str,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(group)],
        immediate_size: 0,
    });
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(Cow::Owned(source)),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_post"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some(fragment),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn texture_entry(binding: u32, multisampled: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled,
        },
        count: None,
    }
}

impl FullPath {
    /// Builds the full path's pipelines for `samples` (1, 2 or 4) in the main pass.
    ///
    /// # Errors
    ///
    /// wgpu's error, on one line, when a pipeline fails to build.
    pub fn new(device: &wgpu::Device, pipelines: &Pipelines, samples: u32) -> Result<Self, String> {
        catch_errors(device, || {
            let shadow_layout = ShadowMap::layout(device);
            let reflection_layout = Reflection::layout(device);
            let scene_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("hdr scene"),
                bind_group_layouts: &[
                    Some(&pipelines.frame_layout),
                    Some(&pipelines.draw_layout),
                    Some(&shadow_layout),
                    Some(&reflection_layout),
                ],
                immediate_size: 0,
            });
            let scene =
                ScenePipelines::new(device, &scene_layout, SceneOutput::Hdr(HDR_FORMAT), samples);
            let shadow = shadow_pipeline(device, &pipelines.scene_layout);
            let shadow_map = ShadowMap::new(device, &shadow_layout);
            let mirror = mirror_pipeline(device, &pipelines.scene_layout, HDR_FORMAT);
            let reflection_sampler = Reflection::sampler(device);
            let resolve_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("hdr resolve"),
                    entries: &[texture_entry(0, true)],
                });
            let resolve = post_pipeline(
                device,
                "hdr resolve",
                &resolve_layout,
                include_str!("../shaders/resolve.wgsl").to_owned(),
                "fs_resolve",
                HDR_FORMAT,
            );
            let composite_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("composite"),
                    entries: &[
                        texture_entry(0, false),
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: NonZeroU64::new((POST_FLOATS * 4) as u64),
                            },
                            count: None,
                        },
                    ],
                });
            let composite = post_pipeline(
                device,
                "composite",
                &composite_layout,
                format!("{TONEMAP}\n{}", include_str!("../shaders/composite.wgsl")),
                "fs_composite",
                SCENE_FORMAT,
            );
            let post_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("post uniforms"),
                size: (POST_FLOATS * 4) as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            Self {
                samples,
                scene,
                shadow,
                shadow_map,
                mirror,
                reflection_layout,
                reflection_sampler,
                resolve_layout,
                resolve,
                composite_layout,
                composite,
                post_buffer,
                targets: None,
            }
        })
    }

    pub fn scene(&self) -> &ScenePipelines {
        &self.scene
    }

    pub fn targets(&self) -> Option<&HdrTargets> {
        self.targets.as_ref()
    }

    /// Makes the targets `size`, keeping them when they already are.
    ///
    /// # Errors
    ///
    /// wgpu's error, on one line, when a target fails to build; the old targets are gone then.
    pub fn ensure_targets(
        &mut self,
        device: &wgpu::Device,
        size: [u32; 2],
        blit_layout: &wgpu::BindGroupLayout,
    ) -> Result<(), String> {
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return Ok(());
        }
        self.targets = None;
        let layouts = TargetLayouts {
            resolve: &self.resolve_layout,
            composite: &self.composite_layout,
            blit: blit_layout,
            reflection: &self.reflection_layout,
        };
        let targets = catch_errors(device, || {
            HdrTargets::new(
                device,
                size,
                self.samples,
                &layouts,
                &self.post_buffer,
                &self.reflection_sampler,
            )
        })?;
        self.targets = Some(targets);
        Ok(())
    }

    pub fn write_post(&self, queue: &wgpu::Queue, exposure: f32) {
        let post: [f32; POST_FLOATS] = [exposure, 0.0, 0.0, 0.0];
        queue.write_buffer(&self.post_buffer, 0, bytemuck::cast_slice(&post));
    }

    /// The shadow pass (`casters` draws them with the pipeline it is given) and the reflection
    /// pass (`mirrored` likewise) when `options` turn them on, the main pass (`draw` fills it), the
    /// resolve and the composite into the output target.
    pub fn record(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        options: FullPathOptions,
        casters: impl FnOnce(&mut wgpu::RenderPass<'_>, &wgpu::RenderPipeline),
        mirrored: impl FnOnce(&mut wgpu::RenderPass<'_>, &wgpu::RenderPipeline),
        draw: impl FnOnce(&mut wgpu::RenderPass<'_>),
    ) {
        let Some(t) = &self.targets else {
            return;
        };
        if options.shadows {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_map.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            casters(&mut pass, &self.shadow);
        }
        if options.reflection {
            // Cleared to transparent black: alpha marks the covered pixels (C++ reflectionPass).
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("reflection"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &t.reflection.color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &t.reflection.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            mirrored(&mut pass, &self.mirror);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hdr main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: t.msaa.as_ref().unwrap_or(&t.resolved),
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &t.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            // Bound even when the passes are off: the lit pipeline's layout has the groups.
            pass.set_bind_group(2, &self.shadow_map.group, &[]);
            pass.set_bind_group(3, &t.reflection.group, &[]);
            draw(&mut pass);
        }
        if let Some(group) = &t.resolve_group {
            post_pass(encoder, "hdr resolve", &t.resolved, &self.resolve, group);
        }
        post_pass(
            encoder,
            "composite",
            &t.output,
            &self.composite,
            &t.composite_group,
        );
    }
}

fn post_pass(
    encoder: &mut wgpu::CommandEncoder,
    label: &str,
    target: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    group: &wgpu::BindGroup,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.draw(0..3, 0..1);
}
