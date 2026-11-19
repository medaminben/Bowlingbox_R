//! The wgpu scene renderer: the full path (HDR, MSAA, composite) where the GPU supports it, the
//! basic path (tone mapped straight into the target) otherwise.

mod capabilities;
mod full;
mod meshes;
mod pipelines;
mod renderer;
mod stats;
mod targets;

pub use capabilities::{Capabilities, choose_path};
pub use full::FullPathOptions;
pub use renderer::SceneRenderer;
pub use stats::{PassStats, TimedPass, render_stats_from_env, stats_due, stats_unsupported_line};
pub use targets::FrameSizes;

/// Which render path to use; `BOWLINGBOX_RENDER` picks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderMode {
    /// Tone mapping in the lit shader, straight into the target.
    Basic,
    /// The HDR main pass and post-processing, where the GPU supports them.
    Full,
}

fn is_known(value: &str) -> bool {
    ["basic", "full"]
        .iter()
        .any(|known| value.eq_ignore_ascii_case(known))
}

/// The render path for a `BOWLINGBOX_RENDER` value: `basic` (any ASCII case) gives `Basic`,
/// anything else, or no value, `Full`. See `render_mode_warning` for values that are not
/// understood.
#[must_use]
pub fn render_mode_from_env(value: Option<&str>) -> RenderMode {
    match value {
        Some(v) if v.eq_ignore_ascii_case("basic") => RenderMode::Basic,
        _ => RenderMode::Full,
    }
}

/// The one line to print for a `BOWLINGBOX_RENDER` value that is set but not understood.
#[must_use]
pub fn render_mode_warning(value: Option<&str>) -> Option<String> {
    match value {
        Some(v) if !is_known(v) => Some(format!(
            "render: unknown BOWLINGBOX_RENDER '{v}'; using the full path where supported"
        )),
        _ => None,
    }
}

/// A device on the first adapter for the module tests; `None` (with a note) without one.
#[cfg(test)]
fn test_device() -> Option<wgpu::Device> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .map_err(|err| println!("note: no GPU adapter ({err}); skipping"))
            .ok()?;
    let limits = adapter.limits();
    let (device, _queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: limits,
        ..Default::default()
    }))
    .map_err(|err| println!("note: device request failed ({err}); skipping"))
    .ok()?;
    Some(device)
}
