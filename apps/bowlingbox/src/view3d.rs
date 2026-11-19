//! The 3D view inside the window: one `SceneRenderer`, kept in egui-wgpu's callback resources and
//! drawn by a paint callback.

use bowlingbox_render::{
    Camera, Capabilities, RenderMode, SceneDescription, SceneGeometry, SceneRenderer, ThemeId,
};
use eframe::egui::{self, Rect};
use eframe::egui_wgpu::{self, CallbackResources, CallbackTrait, ScreenDescriptor};
use eframe::wgpu;

use crate::scene_adapter::scene_geometry;

/// The renderer, the theme it was last told to use and whether its fallback reason is printed.
struct Resource {
    renderer: SceneRenderer,
    theme: ThemeId,
    reported_fallback: bool,
}

impl Resource {
    /// Prints why the basic path runs, once: at startup, or when the full path gives up later.
    fn report_fallback(&mut self) {
        if !self.reported_fallback
            && let Some(reason) = self.renderer.fallback_reason()
        {
            eprintln!("render: {reason}");
            self.reported_fallback = true;
        }
    }
}

/// The device's limits for `adapter`: wgpu's defaults for the backend (as eframe asks), lowered to
/// what the adapter has, with the adapter's whole 2D texture size, which the full path's shadow
/// map and the view-sized targets need.
pub fn device_limits(backend: wgpu::Backend, adapter: &wgpu::Limits) -> wgpu::Limits {
    let base = if backend == wgpu::Backend::Gl {
        wgpu::Limits::downlevel_webgl2_defaults()
    } else {
        wgpu::Limits::default()
    };
    wgpu::Limits {
        max_texture_dimension_2d: adapter.max_texture_dimension_2d,
        ..base.or_worse_values_from(adapter)
    }
}

/// eframe's device request with `device_limits`.
pub fn device_descriptor(adapter: &wgpu::Adapter) -> wgpu::DeviceDescriptor<'static> {
    wgpu::DeviceDescriptor {
        label: Some("bowlingbox device"),
        required_limits: device_limits(adapter.get_info().backend, &adapter.limits()),
        ..Default::default()
    }
}

/// Creates the renderer once. eframe draws without multisampling (`multisampling` 0 and no depth
/// buffer, which the renderer's blit needs), so the pass has one sample.
pub fn install(render_state: &egui_wgpu::RenderState, theme: ThemeId, mode: RenderMode) {
    let geometry: SceneGeometry = scene_geometry();
    let caps = Capabilities::probe(&render_state.adapter, &render_state.device);
    let renderer = SceneRenderer::new(
        &render_state.device,
        &render_state.queue,
        render_state.target_format,
        1,
        geometry,
        theme,
        mode,
        &caps,
    );
    let mut resource = Resource {
        renderer,
        theme,
        reported_fallback: false,
    };
    resource.report_fallback();
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(resource);
}

/// One frame of the 3D view.
struct Frame {
    camera: Camera,
    scene: SceneDescription,
    theme: ThemeId,
    /// The view in points, to find its size in pixels.
    rect: Rect,
}

/// The view's size in whole pixels, rounded as egui rounds the callback's viewport.
fn size_in_pixels(rect: Rect, pixels_per_point: f32) -> [u32; 2] {
    let side = |min: f32, max: f32| {
        let (min, max) = (
            (min * pixels_per_point).round(),
            (max * pixels_per_point).round(),
        );
        // Both are non-negative, whole numbers of pixels.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let pixels = (max - min).max(0.0) as u32;
        pixels
    };
    [side(rect.min.x, rect.max.x), side(rect.min.y, rect.max.y)]
}

impl CallbackTrait for Frame {
    fn prepare(
        &self,
        device: &eframe::wgpu::Device,
        queue: &eframe::wgpu::Queue,
        screen: &ScreenDescriptor,
        _encoder: &mut eframe::wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<eframe::wgpu::CommandBuffer> {
        let Some(resource) = resources.get_mut::<Resource>() else {
            return Vec::new();
        };
        if resource.theme != self.theme {
            resource.renderer.set_theme(self.theme);
            resource.theme = self.theme;
        }
        let size = size_in_pixels(self.rect, screen.pixels_per_point);
        let commands = resource
            .renderer
            .prepare(device, queue, &self.camera, &self.scene, size);
        resource.report_fallback();
        commands.into_iter().collect()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut eframe::wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        if let Some(resource) = resources.get::<Resource>() {
            resource.renderer.paint(pass);
        }
    }
}

/// Fills the rest of the panel with the scene and returns its rectangle.
pub fn show(ui: &mut egui::Ui, camera: Camera, scene: &SceneDescription, theme: ThemeId) -> Rect {
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        rect,
        Frame {
            camera,
            scene: *scene,
            theme,
            rect,
        },
    ));
    rect
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    #[test]
    fn size_scales_with_the_pixel_ratio() {
        let rect = Rect::from_min_size(pos2(10.0, 20.0), vec2(400.0, 300.0));
        assert_eq!(size_in_pixels(rect, 1.0), [400, 300]);
        assert_eq!(size_in_pixels(rect, 1.5), [600, 450]);
    }

    #[test]
    fn the_device_gets_the_adapters_texture_size_and_never_more_than_it_has() {
        let big = wgpu::Limits {
            max_texture_dimension_2d: 16384,
            ..wgpu::Limits::default()
        };
        assert_eq!(
            device_limits(wgpu::Backend::Vulkan, &big).max_texture_dimension_2d,
            16384
        );
        let gl = wgpu::Limits {
            max_texture_dimension_2d: 4096,
            ..wgpu::Limits::downlevel_webgl2_defaults()
        };
        let limits = device_limits(wgpu::Backend::Gl, &gl);
        assert_eq!(limits.max_texture_dimension_2d, 4096);
        assert_eq!(limits.max_bind_groups, gl.max_bind_groups);
        let small = wgpu::Limits {
            max_texture_dimension_2d: 2048,
            max_bind_groups: 2,
            ..wgpu::Limits::default()
        };
        let limits = device_limits(wgpu::Backend::Vulkan, &small);
        assert_eq!(limits.max_texture_dimension_2d, 2048);
        assert_eq!(limits.max_bind_groups, 2);
    }

    #[test]
    fn an_empty_or_inverted_rect_has_no_size() {
        assert_eq!(size_in_pixels(Rect::NOTHING, 2.0), [0, 0]);
        assert_eq!(
            size_in_pixels(Rect::from_min_size(pos2(5.0, 5.0), vec2(0.0, 0.0)), 1.0),
            [0, 0]
        );
    }
}
