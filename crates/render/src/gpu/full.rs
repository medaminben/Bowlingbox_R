//! The full render path: the scene in linear HDR with MSAA, resolved, bloomed and then tone mapped
//! by a composite pass into the same 8-bit output the basic path fills, with the key light's
//! shadow map and the lane reflection.

use super::capabilities::{HDR_FORMAT, catch_errors};
use super::pipelines::{
    Pipelines, SCENE_FORMAT, SceneOutput, ScenePipelines, TONEMAP, mirror_pipeline, shadow_pipeline,
};
use super::stats::{PassWrites, TimedPass};
use super::targets::{BLOOM_LEVELS, FrameSizes, HdrTargets, Reflection, ShadowMap, TargetLayouts};
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
    /// The glow around what is brighter than 1, at the theme's strength.
    pub bloom: bool,
}

impl Default for FullPathOptions {
    fn default() -> Self {
        Self {
            shadows: true,
            reflection: true,
            bloom: true,
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
    /// A filtered texture and its sampler: the reflection and the bloom passes' sources.
    sampled_layout: wgpu::BindGroupLayout,
    /// Linear and clamped to the edge, for the reflection and the bloom levels.
    linear_sampler: wgpu::Sampler,
    bloom: BloomPipelines,
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
    blend: Option<wgpu::BlendState>,
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
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// The bloom chain's passes (C++ `bright`, `bloomDown`, `bloomUp`), each reading one filtered
/// texture through `layout`.
struct BloomPipelines {
    /// The resolved scene's bright part into the first level.
    bright: wgpu::RenderPipeline,
    /// Each level filtered into the next.
    down: wgpu::RenderPipeline,
    /// Each level added onto the one above.
    up: wgpu::RenderPipeline,
}

impl BloomPipelines {
    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let pipeline = |source: &str, fragment, blend| {
            post_pipeline(
                device,
                fragment,
                layout,
                source.to_owned(),
                fragment,
                HDR_FORMAT,
                blend,
            )
        };
        let down = include_str!("../shaders/bloom_down.wgsl");
        let add = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        Self {
            bright: pipeline(down, "fs_bright", None),
            down: pipeline(down, "fs_bloom_down", None),
            up: pipeline(
                include_str!("../shaders/bloom_up.wgsl"),
                "fs_bloom_up",
                Some(wgpu::BlendState {
                    color: add,
                    alpha: add,
                }),
            ),
        }
    }
}

/// The composite's inputs: the resolved scene, the post uniforms, the first bloom level and its
/// sampler.
fn composite_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
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
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
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
            let sampled_layout = Reflection::layout(device);
            let scene_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("hdr scene"),
                bind_group_layouts: &[
                    Some(&pipelines.frame_layout),
                    Some(&pipelines.draw_layout),
                    Some(&shadow_layout),
                    Some(&sampled_layout),
                ],
                immediate_size: 0,
            });
            let scene =
                ScenePipelines::new(device, &scene_layout, SceneOutput::Hdr(HDR_FORMAT), samples);
            let shadow = shadow_pipeline(device, &pipelines.scene_layout);
            let shadow_map = ShadowMap::new(device, &shadow_layout);
            let mirror = mirror_pipeline(device, &pipelines.scene_layout, HDR_FORMAT);
            let linear_sampler = Reflection::sampler(device);
            let bloom = BloomPipelines::new(device, &sampled_layout);
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
                None,
            );
            let composite_layout = composite_layout(device);
            let composite = post_pipeline(
                device,
                "composite",
                &composite_layout,
                format!("{TONEMAP}\n{}", include_str!("../shaders/composite.wgsl")),
                "fs_composite",
                SCENE_FORMAT,
                None,
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
                sampled_layout,
                linear_sampler,
                bloom,
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

    /// Makes the targets `sizes`, keeping them when they already have those sizes and sample
    /// count (C++ `PostChain::ensureSize`).
    ///
    /// # Errors
    ///
    /// wgpu's error, on one line, when a target fails to build; the old targets are gone then.
    pub fn ensure_targets(
        &mut self,
        device: &wgpu::Device,
        sizes: FrameSizes,
        blit_layout: &wgpu::BindGroupLayout,
    ) -> Result<(), String> {
        if self
            .targets
            .as_ref()
            .is_some_and(|t| t.sizes == sizes && t.samples == self.samples)
        {
            return Ok(());
        }
        self.targets = None;
        let layouts = TargetLayouts {
            resolve: &self.resolve_layout,
            composite: &self.composite_layout,
            blit: blit_layout,
            sampled: &self.sampled_layout,
        };
        let targets = catch_errors(device, || {
            HdrTargets::new(
                device,
                sizes,
                self.samples,
                &layouts,
                &self.post_buffer,
                &self.linear_sampler,
            )
        })?;
        self.targets = Some(targets);
        Ok(())
    }

    /// `bloom_strength` 0 leaves the bloom out of the composite.
    pub fn write_post(&self, queue: &wgpu::Queue, exposure: f32, bloom_strength: f32) {
        let post: [f32; POST_FLOATS] = [exposure, bloom_strength, 0.0, 0.0];
        queue.write_buffer(&self.post_buffer, 0, bytemuck::cast_slice(&post));
    }

    /// The shadow pass (`casters` draws them with the pipeline it is given) and the reflection
    /// pass (`mirrored` likewise) when `options` turn them on, the main pass (`draw` fills it), the
    /// resolve, the bloom chain when `options` turn it on, and the composite into the output
    /// target.
    pub fn record(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        options: FullPathOptions,
        casters: impl FnOnce(&mut wgpu::RenderPass<'_>, &wgpu::RenderPipeline),
        mirrored: impl FnOnce(&mut wgpu::RenderPass<'_>, &wgpu::RenderPipeline),
        draw: impl FnOnce(&mut wgpu::RenderPass<'_>),
        timer: Option<PassWrites<'_>>,
    ) {
        let Some(t) = &self.targets else {
            return;
        };
        let at = |pass, begin, end| timer.map(|w| w.span(pass, begin, end));
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
                timestamp_writes: at(TimedPass::Shadow, true, true),
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
                timestamp_writes: at(TimedPass::Reflection, true, true),
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
                timestamp_writes: at(TimedPass::Main, true, true),
                ..Default::default()
            });
            // Bound even when the passes are off: the lit pipeline's layout has the groups.
            pass.set_bind_group(2, &self.shadow_map.group, &[]);
            pass.set_bind_group(3, &t.reflection.group, &[]);
            draw(&mut pass);
        }
        // The post time runs from the first of these passes to the end of the composite.
        let mut post_start = at(TimedPass::Post, true, false);
        if let Some(group) = &t.resolve_group {
            let writes = post_start.take();
            post_pass(
                encoder,
                "hdr resolve",
                &t.resolved,
                &self.resolve,
                group,
                writes,
            );
        }
        if options.bloom {
            self.record_bloom(encoder, t, post_start.take());
        }
        post_pass(
            encoder,
            "composite",
            &t.output,
            &self.composite,
            &t.composite_group,
            at(TimedPass::Post, post_start.is_some(), true),
        );
    }

    /// C++ `PostChain::present`'s bloom: the bright part into the first level, down the chain,
    /// then each level added onto the one above, back to the first.
    /// `writes` go on the bright pass, the chain's first.
    fn record_bloom(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        t: &HdrTargets,
        writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
    ) {
        let p = &self.bloom;
        post_pass(
            encoder,
            "bloom bright",
            &t.bloom[0],
            &p.bright,
            &t.bright_group,
            writes,
        );
        for i in 1..BLOOM_LEVELS {
            let group = &t.bloom_groups[i - 1];
            post_pass(encoder, "bloom down", &t.bloom[i], &p.down, group, None);
        }
        for i in (1..BLOOM_LEVELS).rev() {
            let group = &t.bloom_groups[i];
            let load = wgpu::LoadOp::Load;
            let mut pass = target_pass(encoder, "bloom up", &t.bloom[i - 1], load, None);
            pass.set_pipeline(&p.up);
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

/// A pass that draws into `target` alone.
fn target_pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    label: &str,
    target: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
    timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        timestamp_writes,
        ..Default::default()
    })
}

/// One generated triangle over `target`, which `pipeline` fills from `group`.
fn post_pass(
    encoder: &mut wgpu::CommandEncoder,
    label: &str,
    target: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    group: &wgpu::BindGroup,
    timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
) {
    let load = wgpu::LoadOp::Clear(wgpu::Color::BLACK);
    let mut pass = target_pass(encoder, label, target, load, timestamp_writes);
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.draw(0..3, 0..1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_are_rebuilt_only_for_a_new_size() {
        let Some(device) = crate::gpu::test_device() else {
            return;
        };
        let pipelines = Pipelines::new(&device, 1, wgpu::TextureFormat::Rgba8UnormSrgb, 1);
        let mut full = FullPath::new(&device, &pipelines, 1).expect("full-path pipelines");
        let mut ensure = |view| {
            full.ensure_targets(&device, FrameSizes::new(view), &pipelines.blit_layout)
                .expect("targets");
            let t = full.targets().expect("targets");
            (t.resolved.clone(), t.bloom[3].clone())
        };
        let first = ensure([320, 180]);
        assert_eq!(
            ensure([320, 180]),
            first,
            "the same size rebuilt the targets"
        );
        let smaller = ensure([64, 36]);
        assert_ne!(smaller.0, first.0, "a new size kept the old targets");
        let tiny = ensure([1, 1]);
        assert_eq!(tiny.1.texture().size().width, 1);
        assert_eq!(tiny.1.texture().size().height, 1);
    }
}
