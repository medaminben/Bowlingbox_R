//! The wgpu scene renderer: basic path only (no shadow map, reflection, HDR target or bloom).

mod meshes;
mod pipelines;
mod renderer;

pub use renderer::SceneRenderer;

/// Which render path to use; `BOWLINGBOX_RENDER` picks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderMode {
    /// Tone mapping in the lit shader, straight into the target: the only path so far.
    Basic,
}

fn is_basic(value: &str) -> bool {
    value.eq_ignore_ascii_case("basic")
}

/// The render path for a `BOWLINGBOX_RENDER` value. Every value gives `Basic` for now; see
/// `render_mode_warning` for values that are not understood.
#[must_use]
pub fn render_mode_from_env(_value: Option<&str>) -> RenderMode {
    RenderMode::Basic
}

/// The one line to print for a `BOWLINGBOX_RENDER` value that is set but not understood.
#[must_use]
pub fn render_mode_warning(value: Option<&str>) -> Option<String> {
    match value {
        Some(v) if !is_basic(v) => Some(format!(
            "render: unknown BOWLINGBOX_RENDER '{v}'; using the basic path"
        )),
        _ => None,
    }
}
