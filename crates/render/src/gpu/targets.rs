//! The full path's offscreen targets at one size (C++ `PostChain::ensureSize`).

use super::capabilities::{HDR_FORMAT, SHADOW_MAP_HEIGHT, SHADOW_MAP_WIDTH};
use super::pipelines::{DEPTH_FORMAT, SCENE_FORMAT, SHADOW_FORMAT};

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

/// The key light's depth map (C++ `ShadowMap`), 1024x4096: the lane runs along its long side.
/// Its size does not depend on the view, so it is built once with the full path.
pub struct ShadowMap {
    pub depth: wgpu::TextureView,
    /// The map and its less-equal comparison sampler, for the lit shader's group 2.
    pub group: wgpu::BindGroup,
}

impl ShadowMap {
    /// The layout of [`ShadowMap::group`].
    pub fn layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow map"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        })
    }

    pub fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let depth = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("shadow map"),
                size: wgpu::Extent3d {
                    width: SHADOW_MAP_WIDTH,
                    height: SHADOW_MAP_HEIGHT,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: SHADOW_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());
        // As C++: clamped, linear (each tap a 2x2 hardware PCF), compare ref <= stored.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow map"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..wgpu::SamplerDescriptor::default()
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow map"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&depth),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        Self { depth, group }
    }
}
