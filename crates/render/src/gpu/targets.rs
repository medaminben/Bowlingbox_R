//! The full path's offscreen targets at one size (C++ `PostChain::ensureSize`).

use super::capabilities::HDR_FORMAT;
use super::pipelines::{DEPTH_FORMAT, SCENE_FORMAT};

/// The bind group layouts the targets are bound through.
pub struct TargetLayouts<'a> {
    pub resolve: &'a wgpu::BindGroupLayout,
    pub composite: &'a wgpu::BindGroupLayout,
    pub blit: &'a wgpu::BindGroupLayout,
}

pub struct HdrTargets {
    pub size: [u32; 2],
    /// The multisampled HDR colour the main pass draws into; `None` with one sample, when the main
    /// pass draws straight into `resolved`.
    pub msaa: Option<wgpu::TextureView>,
    /// The main pass's depth, with its sample count.
    pub depth: wgpu::TextureView,
    /// The single-sample HDR scene the composite reads.
    pub resolved: wgpu::TextureView,
    /// Display-ready sRGB bytes, which `paint` copies into the caller's pass.
    pub output: wgpu::TextureView,
    /// Reads `msaa`; `None` with one sample.
    pub resolve_group: Option<wgpu::BindGroup>,
    /// Reads `resolved` and the post uniforms.
    pub composite_group: wgpu::BindGroup,
    /// Reads `output`.
    pub blit_group: wgpu::BindGroup,
}

impl HdrTargets {
    pub fn new(
        device: &wgpu::Device,
        size: [u32; 2],
        samples: u32,
        layouts: &TargetLayouts<'_>,
        post_uniforms: &wgpu::Buffer,
    ) -> Self {
        let extent = wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        };
        let texture = |label: &str, format, samples, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: extent,
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        };
        let sampled_target =
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let msaa = (samples > 1).then(|| texture("hdr msaa", HDR_FORMAT, samples, sampled_target));
        let depth = texture(
            "hdr depth",
            DEPTH_FORMAT,
            samples,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let resolved = texture("hdr resolved", HDR_FORMAT, 1, sampled_target);
        let output = texture("full output", SCENE_FORMAT, 1, sampled_target);
        let group = |label: &str, layout, entries: &[wgpu::BindGroupEntry<'_>]| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout,
                entries,
            })
        };
        let view_entry = |binding, view| wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::TextureView(view),
        };
        let resolve_group = msaa
            .as_ref()
            .map(|view| group("hdr resolve", layouts.resolve, &[view_entry(0, view)]));
        let composite_group = group(
            "composite",
            layouts.composite,
            &[
                view_entry(0, &resolved),
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: post_uniforms.as_entire_binding(),
                },
            ],
        );
        let blit_group = group("full output", layouts.blit, &[view_entry(0, &output)]);
        Self {
            size,
            msaa,
            depth,
            resolved,
            output,
            resolve_group,
            composite_group,
            blit_group,
        }
    }
}
