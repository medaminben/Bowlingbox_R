//! What the full render path needs from the GPU, and the choice between the two paths.

use super::RenderMode;
use super::pipelines::DEPTH_FORMAT;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

/// The full path's HDR colour format.
pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// The shadow map's short side (1024x4096, as in C++).
pub const SHADOW_MAP_WIDTH: u32 = 1024;
/// The shadow map's long side (1024x4096, as in C++).
pub const SHADOW_MAP_HEIGHT: u32 = 4096;

/// What the adapter and device offer the full path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    /// The HDR main pass's sample count: 4, 2 or 1, the highest that both `Rgba16Float` and the
    /// depth format support; 0 when `Rgba16Float` cannot be rendered to and sampled.
    pub hdr_msaa_samples: u32,
    /// The device's `max_texture_dimension_2d`.
    pub max_texture_2d: u32,
    /// The device has `TIMESTAMP_QUERY`.
    pub timestamp_query: bool,
}

impl Capabilities {
    /// Reads what `device`, created from `adapter`, can do.
    #[must_use]
    pub fn probe(adapter: &wgpu::Adapter, device: &wgpu::Device) -> Self {
        let features = device.features();
        // As wgpu validates: the adapter's own format features count only with
        // TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES or on a downlevel adapter.
        let adapter_specific = features
            .contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES)
            || !adapter
                .get_downlevel_capabilities()
                .flags
                .contains(wgpu::DownlevelFlags::WEBGPU_TEXTURE_FORMAT_SUPPORT);
        let format_features = |format: wgpu::TextureFormat| {
            if adapter_specific {
                adapter.get_texture_format_features(format)
            } else {
                format.guaranteed_format_features(features)
            }
        };
        Self {
            hdr_msaa_samples: on_backend(
                adapter.get_info().backend,
                hdr_samples(format_features(HDR_FORMAT), format_features(DEPTH_FORMAT)),
            ),
            max_texture_2d: device.limits().max_texture_dimension_2d,
            timestamp_query: features.contains(wgpu::Features::TIMESTAMP_QUERY),
        }
    }
}

/// `samples` limited to what `backend` can draw: wgpu-hal 30's GL backend creates a multisampled
/// texture that is also sampled with the `GL_TEXTURE_2D` target, which fails, and the HDR pass then
/// draws nothing. On GL the pass therefore has one sample.
fn on_backend(backend: wgpu::Backend, samples: u32) -> u32 {
    if backend == wgpu::Backend::Gl {
        samples.min(1)
    } else {
        samples
    }
}

/// The highest of 4, 2 and 1 samples both formats support; 0 when the colour format cannot be a
/// render target that is sampled afterwards.
fn hdr_samples(color: wgpu::TextureFormatFeatures, depth: wgpu::TextureFormatFeatures) -> u32 {
    let needed = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
    if !color.allowed_usages.contains(needed)
        || !depth
            .allowed_usages
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
    {
        return 0;
    }
    let both = |flag| color.flags.contains(flag) && depth.flags.contains(flag);
    if both(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4) {
        4
    } else if both(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X2) {
        2
    } else {
        1
    }
}

/// `Ok` when `mode` and `caps` allow the full path; otherwise the reason the basic path runs,
/// without the `render: ` prefix.
///
/// # Errors
///
/// The reason line when the basic path runs.
pub fn choose_path(mode: RenderMode, caps: &Capabilities) -> Result<(), String> {
    if mode == RenderMode::Basic {
        return Err("basic path (BOWLINGBOX_RENDER=basic)".to_owned());
    }
    if caps.hdr_msaa_samples == 0 {
        return Err(format!(
            "{HDR_FORMAT:?} target not supported; using the basic path"
        ));
    }
    if caps.max_texture_2d < SHADOW_MAP_HEIGHT {
        return Err(format!(
            "max texture size {} is below the {SHADOW_MAP_HEIGHT} px shadow map; using the basic path",
            caps.max_texture_2d
        ));
    }
    Ok(())
}

/// Runs `create` inside validation and out-of-memory error scopes; `Err` holds the first error.
pub fn catch_errors<T>(device: &wgpu::Device, create: impl FnOnce() -> T) -> Result<T, String> {
    let memory = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let value = create();
    let error = ready(validation.pop()).or_else(|| ready(memory.pop()));
    match error {
        None => Ok(value),
        Some(error) => Err(one_line(&error.to_string())),
    }
}

/// The scope's result. Native wgpu reports errors as they happen, so the future is ready at once;
/// one that is not (the web) counts as no error.
fn ready(future: impl Future<Output = Option<wgpu::Error>>) -> Option<wgpu::Error> {
    match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(error) => error,
        Poll::Pending => None,
    }
}

/// wgpu's multi-line error text on one line.
fn one_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::{TextureFormatFeatureFlags as Flags, TextureFormatFeatures, TextureUsages};

    fn features(usages: TextureUsages, flags: Flags) -> TextureFormatFeatures {
        TextureFormatFeatures {
            allowed_usages: usages,
            flags,
        }
    }

    const SAMPLED_TARGET: TextureUsages =
        TextureUsages::RENDER_ATTACHMENT.union(TextureUsages::TEXTURE_BINDING);

    #[test]
    fn samples_are_the_highest_both_formats_support() {
        let x4 = Flags::MULTISAMPLE_X2.union(Flags::MULTISAMPLE_X4);
        let colour = features(SAMPLED_TARGET, x4);
        assert_eq!(hdr_samples(colour, features(SAMPLED_TARGET, x4)), 4);
        let depth_x2 = features(SAMPLED_TARGET, Flags::MULTISAMPLE_X2);
        assert_eq!(hdr_samples(colour, depth_x2), 2);
        assert_eq!(
            hdr_samples(colour, features(SAMPLED_TARGET, Flags::empty())),
            1
        );
    }

    #[test]
    fn the_gl_backend_draws_the_hdr_pass_with_one_sample() {
        assert_eq!(on_backend(wgpu::Backend::Gl, 4), 1);
        assert_eq!(on_backend(wgpu::Backend::Gl, 0), 0);
        assert_eq!(on_backend(wgpu::Backend::Vulkan, 4), 4);
    }

    #[test]
    fn a_colour_format_that_cannot_be_drawn_and_sampled_gives_zero() {
        let depth = features(SAMPLED_TARGET, Flags::MULTISAMPLE_X4);
        let unsampled = features(TextureUsages::RENDER_ATTACHMENT, Flags::MULTISAMPLE_X4);
        assert_eq!(hdr_samples(unsampled, depth), 0);
    }

    #[test]
    fn an_invalid_creation_inside_the_scope_is_an_error() {
        let Some(device) = crate::gpu::test_device() else {
            return;
        };
        let empty = || {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("empty"),
                size: wgpu::Extent3d {
                    width: 0,
                    height: 0,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HDR_FORMAT,
                usage: TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        };
        let error = catch_errors(&device, empty).expect_err("a zero-size texture is invalid");
        assert!(!error.contains('\n'), "{error}");
        assert_eq!(catch_errors(&device, || 7), Ok(7));
    }

    #[test]
    fn error_text_is_joined_onto_one_line() {
        assert_eq!(
            one_line("Validation Error\n\nCaused by:\n  bad format\n"),
            "Validation Error Caused by: bad format"
        );
    }
}
