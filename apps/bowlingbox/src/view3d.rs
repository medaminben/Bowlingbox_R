//! The 3D view inside the window: one `SceneRenderer`, kept in egui-wgpu's callback resources and
//! drawn by a paint callback.

use bowlingbox_render::{Camera, SceneDescription, SceneGeometry, SceneRenderer, ThemeId};
use eframe::egui::{self, Rect};
use eframe::egui_wgpu::{self, CallbackResources, CallbackTrait, ScreenDescriptor};

use crate::scene_adapter::scene_geometry;

/// The renderer and the theme it was last told to use.
struct Resource {
    renderer: SceneRenderer,
    theme: ThemeId,
}

/// Creates the renderer once. eframe draws without multisampling (`multisampling` 0 and no depth
/// buffer, which the renderer's blit needs), so the pass has one sample.
pub fn install(render_state: &egui_wgpu::RenderState, theme: ThemeId) {
    let geometry: SceneGeometry = scene_geometry();
    let renderer = SceneRenderer::new(
        &render_state.device,
        &render_state.queue,
        render_state.target_format,
        1,
        geometry,
        theme,
    );
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(Resource { renderer, theme });
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
        resource
            .renderer
            .prepare(device, queue, &self.camera, &self.scene, size)
            .into_iter()
            .collect()
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
    fn an_empty_or_inverted_rect_has_no_size() {
        assert_eq!(size_in_pixels(Rect::NOTHING, 2.0), [0, 0]);
        assert_eq!(
            size_in_pixels(Rect::from_min_size(pos2(5.0, 5.0), vec2(0.0, 0.0)), 1.0),
            [0, 0]
        );
    }
}
