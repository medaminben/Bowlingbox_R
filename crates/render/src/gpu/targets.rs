//! The full path's offscreen targets at one size (C++ `PostChain::ensureSize`).

use super::capabilities::{HDR_FORMAT, SHADOW_MAP_HEIGHT, SHADOW_MAP_WIDTH};
use super::pipelines::{DEPTH_FORMAT, SCENE_FORMAT, SHADOW_FORMAT};

/// The full path's offscreen target sizes for a view (C++ `FrameSizes`); every side is at least
/// 1 px.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameSizes {
    /// The HDR main pass.
    pub scene: [u32; 2],
    /// The lane mirror: half the scene.
    pub reflection: [u32; 2],
    /// The bloom chain: half, quarter, eighth and sixteenth of the scene.
    pub bloom: [[u32; 2]; BLOOM_LEVELS],
}

/// Levels in the bloom chain.
pub const BLOOM_LEVELS: usize = 4;

impl FrameSizes {
    /// The sizes for a view of `view` pixels (C++ `frameSizes`).
    #[must_use]
    pub fn new(view: [u32; 2]) -> Self {
        let half = |size: [u32; 2]| size.map(|side| (side / 2).max(1));
        let scene = view.map(|side| side.max(1));
        let mut bloom = [half(scene); BLOOM_LEVELS];
        for i in 1..BLOOM_LEVELS {
            bloom[i] = half(bloom[i - 1]);
        }
        Self {
            scene,
            reflection: half(scene),
            bloom,
        }
    }
}

/// The bind group layouts the targets are bound through.
pub struct TargetLayouts<'a> {
    pub resolve: &'a wgpu::BindGroupLayout,
    pub composite: &'a wgpu::BindGroupLayout,
    pub blit: &'a wgpu::BindGroupLayout,
    /// A filtered texture and its sampler: the reflection and the bloom passes' sources.
    pub sampled: &'a wgpu::BindGroupLayout,
}

pub struct HdrTargets {
    pub sizes: FrameSizes,
    pub samples: u32,
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
    pub reflection: Reflection,
    /// The bloom chain's levels, half to sixteenth of the scene (C++ `_bloom`).
    pub bloom: [wgpu::TextureView; BLOOM_LEVELS],
    /// Reads `resolved` filtered, for the bright pass into the first level.
    pub bright_group: wgpu::BindGroup,
    /// Reads each bloom level filtered.
    pub bloom_groups: [wgpu::BindGroup; BLOOM_LEVELS],
}

impl HdrTargets {
    /// `linear_sampler` filters the reflection and the bloom levels.
    pub fn new(
        device: &wgpu::Device,
        sizes: FrameSizes,
        samples: u32,
        layouts: &TargetLayouts<'_>,
        post_uniforms: &wgpu::Buffer,
        linear_sampler: &wgpu::Sampler,
    ) -> Self {
        let texture = |label: &str, size: [u32; 2], format, samples, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size[0],
                        height: size[1],
                        depth_or_array_layers: 1,
                    },
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
        let size = sizes.scene;
        let msaa =
            (samples > 1).then(|| texture("hdr msaa", size, HDR_FORMAT, samples, sampled_target));
        let depth = texture(
            "hdr depth",
            size,
            DEPTH_FORMAT,
            samples,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let resolved = texture("hdr resolved", size, HDR_FORMAT, 1, sampled_target);
        let output = texture("full output", size, SCENE_FORMAT, 1, sampled_target);
        let bloom = sizes
            .bloom
            .map(|level| texture("bloom", level, HDR_FORMAT, 1, sampled_target));
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
        let filtered = |label: &str, view| {
            group(
                label,
                layouts.sampled,
                &[
                    view_entry(0, view),
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(linear_sampler),
                    },
                ],
            )
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
                view_entry(2, &bloom[0]),
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(linear_sampler),
                },
            ],
        );
        let blit_group = group("full output", layouts.blit, &[view_entry(0, &output)]);
        let bright_group = filtered("bloom bright", &resolved);
        let bloom_groups = std::array::from_fn(|i| filtered("bloom level", &bloom[i]));
        let reflection = Reflection::new(device, sizes.reflection, layouts.sampled, linear_sampler);
        Self {
            sizes,
            samples,
            msaa,
            depth,
            resolved,
            output,
            resolve_group,
            composite_group,
            blit_group,
            reflection,
            bloom,
            bright_group,
            bloom_groups,
        }
    }
}

/// The scene mirrored in the lane plane (C++ `reflection`, `GL_RGBA16F` with a depth buffer, one
/// sample), which the main lane samples.
pub struct Reflection {
    pub color: wgpu::TextureView,
    pub depth: wgpu::TextureView,
    /// `color` and its sampler, for the lit shader's group 3.
    pub group: wgpu::BindGroup,
}

impl Reflection {
    /// The layout of [`Reflection::group`] and of the bloom passes' sources: a filtered texture
    /// and its sampler.
    pub fn layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sampled texture"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    /// As C++ `GlFramebuffer`'s colour textures: linear, clamped to the edge. The bloom levels
    /// share it.
    pub fn sampler(device: &wgpu::Device) -> wgpu::Sampler {
        device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("reflection"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        })
    }

    fn new(
        device: &wgpu::Device,
        size: [u32; 2],
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
    ) -> Self {
        let texture = |label: &str, format, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size[0],
                        height: size[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        };
        let color = texture(
            "reflection",
            HDR_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let depth = texture(
            "reflection depth",
            DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("reflection"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&color),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        Self {
            color,
            depth,
            group,
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
