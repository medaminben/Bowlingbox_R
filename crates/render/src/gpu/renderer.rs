//! Draws the lane, pins, ball, decor, sky and aim guide into the renderer's own colour and depth
//! targets, then copies the result into the caller's render pass.

use super::meshes::{MeshKind, Meshes};
use super::pipelines::{
    DEPTH_FORMAT, DRAW_FLOATS, DRAW_STRIDE, FRAME_FLOATS, Pipelines, SCENE_FORMAT,
};
use crate::camera::Camera;
use crate::decor::{Prop, Shape, build_props};
use crate::lighting::srgb_to_linear_vec3;
use crate::scene::{SceneDescription, SceneGeometry};
use crate::theme::{MAX_POINT_LIGHTS, Material, Pattern, Theme, ThemeId, theme};
use glam::{Mat3, Mat4, Quat, Vec2, Vec3};
use std::num::NonZeroU64;

/// Band range that matches no y (min above max).
const NO_BAND: Vec2 = Vec2::new(1.0, -1.0);
const GUIDE: Material = Material {
    color: Vec3::new(0.0, 0.898, 1.0),
    emissive: true,
    roughness: 1.0,
    specular: 0.0,
    emissive_strength: 1.0,
    pattern: Pattern::None,
};
const GUIDE_ALPHA: f32 = 0.40;
/// The draws besides the props: lane, two gutters, two gutter walls, ten pins, ball, aim guide.
const FIXED_DRAWS: usize = 17;

struct DrawItem {
    mesh: MeshKind,
    model: Mat4,
    material: Material,
    band: Vec2,
    alpha: f32,
    fog: bool,
}

impl DrawItem {
    fn opaque(mesh: MeshKind, model: Mat4, material: Material, band: Vec2) -> Self {
        Self {
            mesh,
            model,
            material,
            band,
            alpha: 1.0,
            fog: true,
        }
    }
}

/// The scene's colour and depth targets at one size.
struct Targets {
    size: [u32; 2],
    color: wgpu::TextureView,
    /// The single-sample copy when the colour target is multisampled.
    resolve: Option<wgpu::TextureView>,
    depth: wgpu::TextureView,
    blit_group: wgpu::BindGroup,
}

/// The 3D view. `prepare` records the scene into the renderer's own targets; `paint` copies them
/// into the caller's pass (an egui-wgpu paint callback's).
pub struct SceneRenderer {
    geometry: SceneGeometry,
    theme: &'static Theme,
    props: Vec<Prop>,
    meshes: Meshes,
    pipelines: Pipelines,
    scene_samples: u32,
    frame_buffer: wgpu::Buffer,
    frame_group: wgpu::BindGroup,
    draw_buffer: wgpu::Buffer,
    draw_group: wgpu::BindGroup,
    draw_capacity: usize,
    draws: Vec<DrawItem>,
    opaque_count: usize,
    targets: Option<Targets>,
    /// The last `prepare` drew a frame; `paint` copies it.
    ready: bool,
}

impl SceneRenderer {
    /// `target_format` and `msaa_samples` describe the render pass `paint` draws into. The queue is
    /// not needed yet; it keeps the signature egui-wgpu callbacks have at hand.
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        msaa_samples: u32,
        geometry: SceneGeometry,
        theme_id: ThemeId,
    ) -> Self {
        let (scene_samples, note) = scene_samples(msaa_samples);
        if let Some(note) = note {
            eprintln!("{note}");
        }
        let pipelines = Pipelines::new(device, scene_samples, target_format, msaa_samples);
        let frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame uniforms"),
            size: (FRAME_FLOATS * 4) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame uniforms"),
            layout: &pipelines.frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buffer.as_entire_binding(),
            }],
        });
        let current = theme(theme_id);
        let props = build_props(current, &geometry);
        let draw_capacity = props.len() + FIXED_DRAWS;
        let (draw_buffer, draw_group) = draw_buffers(device, &pipelines, draw_capacity);
        Self {
            meshes: Meshes::new(device, &geometry),
            geometry,
            theme: current,
            props,
            pipelines,
            scene_samples,
            frame_buffer,
            frame_group,
            draw_buffer,
            draw_group,
            draw_capacity,
            draws: Vec::new(),
            opaque_count: 0,
            targets: None,
            ready: false,
        }
    }

    /// Takes effect at the next `prepare`.
    pub fn set_theme(&mut self, id: ThemeId) {
        self.theme = theme(id);
        self.props = build_props(self.theme, &self.geometry);
    }

    /// Draws the scene at `size_px` (the view's size in physical pixels) into the renderer's
    /// targets and returns the commands to submit before the pass that `paint` draws into. The
    /// camera's aspect is taken from `size_px`. A zero-size view draws nothing and returns `None`.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera: &Camera,
        scene: &SceneDescription,
        size_px: [u32; 2],
    ) -> Option<wgpu::CommandBuffer> {
        let Some(size) = drawable_size(size_px, device.limits().max_texture_dimension_2d) else {
            self.ready = false;
            return None;
        };
        self.ensure_targets(device, size);
        let mut camera = *camera;
        camera.set_viewport(size_px[0], size_px[1]);
        queue.write_buffer(
            &self.frame_buffer,
            0,
            bytemuck::cast_slice(&self.frame_uniforms(&camera)),
        );
        self.collect_draws(scene);
        self.ensure_draw_capacity(device);
        queue.write_buffer(
            &self.draw_buffer,
            0,
            bytemuck::cast_slice(&self.draw_uniforms()),
        );

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("scene"),
        });
        self.record(&mut encoder);
        self.ready = true;
        Some(encoder.finish())
    }

    /// Copies the last prepared frame into `pass`, filling its viewport. Draws nothing after a
    /// zero-size `prepare`.
    pub fn paint(&self, pass: &mut wgpu::RenderPass<'static>) {
        let Some(targets) = self.targets.as_ref().filter(|_| self.ready) else {
            return;
        };
        pass.set_pipeline(&self.pipelines.blit);
        pass.set_bind_group(0, &targets.blit_group, &[]);
        pass.draw(0..3, 0..1);
    }

    fn ensure_targets(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
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
        let shown = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let multisampled = self.scene_samples > 1;
        let color = texture(
            "scene colour",
            SCENE_FORMAT,
            self.scene_samples,
            if multisampled {
                wgpu::TextureUsages::RENDER_ATTACHMENT
            } else {
                shown
            },
        );
        let resolve = multisampled.then(|| texture("scene resolve", SCENE_FORMAT, 1, shown));
        let depth = texture(
            "scene depth",
            DEPTH_FORMAT,
            self.scene_samples,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let blit_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene texture"),
            layout: &self.pipelines.blit_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(resolve.as_ref().unwrap_or(&color)),
            }],
        });
        self.targets = Some(Targets {
            size,
            color,
            resolve,
            depth,
            blit_group,
        });
    }

    fn ensure_draw_capacity(&mut self, device: &wgpu::Device) {
        if self.draws.len() <= self.draw_capacity {
            return;
        }
        self.draw_capacity = self.draws.len().next_power_of_two();
        (self.draw_buffer, self.draw_group) =
            draw_buffers(device, &self.pipelines, self.draw_capacity);
    }

    fn frame_uniforms(&self, camera: &Camera) -> Vec<f32> {
        let t = self.theme;
        let g = &self.geometry;
        let mut out = Vec::with_capacity(FRAME_FLOATS);
        out.extend_from_slice(&camera.view().to_cols_array());
        out.extend_from_slice(&camera.projection().to_cols_array());
        for v in [
            camera.eye,
            t.light_dir.normalize(),
            t.key_color * t.key_intensity,
            t.sky_ambient,
            t.ground_ambient,
            srgb_to_linear_vec3(t.fog_color),
            srgb_to_linear_vec3(t.pin_band.color),
            srgb_to_linear_vec3(t.background_top),
            srgb_to_linear_vec3(t.background_bottom),
        ] {
            out.extend_from_slice(&v.extend(0.0).to_array());
        }
        let count = t.point_light_count.min(MAX_POINT_LIGHTS);
        for l in &t.point_lights {
            out.extend_from_slice(&l.position.extend(l.range).to_array());
        }
        for l in &t.point_lights {
            out.extend_from_slice(&(l.color * l.intensity).extend(0.0).to_array());
        }
        out.extend_from_slice(&[t.fog_start, t.fog_end, t.exposure, count as f32]);
        // A real lane has 39 boards; the pin deck starts 0.3 m before the head pin.
        out.extend_from_slice(&[
            g.lane_half_width,
            2.0 * g.lane_half_width / 39.0,
            g.head_pin_z - 0.3,
            0.0,
        ]);
        debug_assert_eq!(out.len(), FRAME_FLOATS);
        out
    }

    /// The pin neck stripe, in the pin mesh's object space.
    fn neck(&self) -> Vec2 {
        let g = &self.geometry;
        Vec2::new(
            0.55 * g.pin_height - g.pin_com_height,
            0.65 * g.pin_height - g.pin_com_height,
        )
    }

    /// Lane, gutters, gutter walls, standing pins and the ball, then the props (the large near
    /// surfaces first, so the depth test rejects most hidden prop fragments), then the aim guide.
    fn collect_draws(&mut self, scene: &SceneDescription) {
        let t = self.theme;
        let g = self.geometry;
        let unit_box = MeshKind::Shape(Shape::Box);
        let block = |centre: Vec3, size: Vec3| {
            Mat4::from_scale_rotation_translation(size, Quat::IDENTITY, centre)
        };
        let outer_x = g.lane_half_width + g.gutter_width;
        let neck = self.neck();
        self.draws.clear();
        self.draws.push(DrawItem::opaque(
            unit_box,
            block(
                Vec3::new(0.0, -0.05, (g.lane_length - g.approach) / 2.0),
                Vec3::new(2.0 * g.lane_half_width, 0.1, g.lane_length + g.approach),
            ),
            t.lane,
            NO_BAND,
        ));
        for sx in [-1.0, 1.0] {
            self.draws.push(DrawItem::opaque(
                unit_box,
                block(
                    Vec3::new(
                        sx * (g.lane_half_width + g.gutter_width / 2.0),
                        -g.gutter_depth - 0.05,
                        g.lane_length / 2.0,
                    ),
                    Vec3::new(g.gutter_width, 0.1, g.lane_length),
                ),
                t.gutter,
                NO_BAND,
            ));
        }
        for sx in [-1.0, 1.0] {
            self.draws.push(DrawItem::opaque(
                unit_box,
                block(
                    Vec3::new(sx * (outer_x + 0.02), 0.05, g.lane_length / 2.0),
                    Vec3::new(0.04, 0.3, g.lane_length),
                ),
                t.gutter,
                NO_BAND,
            ));
        }
        for (pin, _) in scene
            .pins
            .iter()
            .zip(scene.pin_visible)
            .filter(|(_, visible)| *visible)
        {
            self.draws.push(DrawItem::opaque(
                MeshKind::Shape(Shape::Pin),
                pin.matrix(),
                t.pin_body,
                neck,
            ));
        }
        self.draws.push(DrawItem::opaque(
            MeshKind::Ball,
            scene.ball.matrix(),
            t.ball,
            NO_BAND,
        ));
        for prop in &self.props {
            self.draws.push(DrawItem::opaque(
                MeshKind::Shape(prop.shape),
                prop.transform.matrix(),
                prop.material,
                if prop.pin_band { neck } else { NO_BAND },
            ));
        }
        self.opaque_count = self.draws.len();
        if scene.show_aim_guide {
            // From the ball start (z = -1) to the deck end.
            let length = g.lane_length + 1.0;
            let a = scene.aim_angle_deg.to_radians();
            let dir = Vec3::new(a.sin(), 0.0, a.cos());
            let start = Vec3::new(scene.aim_offset, 0.004, -1.0);
            self.draws.push(DrawItem {
                mesh: unit_box,
                model: Mat4::from_scale_rotation_translation(
                    Vec3::new(0.02, 0.006, length),
                    Quat::from_rotation_y(a),
                    start + dir * (length / 2.0),
                ),
                material: GUIDE,
                band: NO_BAND,
                alpha: GUIDE_ALPHA,
                fog: false,
            });
        }
    }

    fn draw_uniforms(&self) -> Vec<f32> {
        const STRIDE_FLOATS: usize = (DRAW_STRIDE / 4) as usize;
        const { assert!(DRAW_FLOATS <= STRIDE_FLOATS) };
        let mut out = vec![0.0; self.draws.len() * STRIDE_FLOATS];
        for (d, slot) in self
            .draws
            .iter()
            .zip(out.as_chunks_mut::<STRIDE_FLOATS>().0)
        {
            let m = &d.material;
            let flag = |b: bool| if b { 1.0 } else { 0.0 };
            let fields = [
                d.model.to_cols_array(),
                Mat4::from_mat3(normal_matrix(d.model)).to_cols_array(),
            ];
            let tail = [
                srgb_to_linear_vec3(m.color).extend(d.alpha).to_array(),
                [d.band.x, d.band.y, flag(m.emissive), m.emissive_strength],
                [
                    m.roughness,
                    m.specular,
                    m.pattern as u32 as f32,
                    flag(d.fog),
                ],
            ];
            let values = fields.iter().flatten().chain(tail.iter().flatten());
            for (dst, src) in slot.iter_mut().zip(values) {
                *dst = *src;
            }
        }
        out
    }

    fn record(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some(targets) = &self.targets else {
            return;
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &targets.color,
                depth_slice: None,
                resolve_target: targets.resolve.as_ref(),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: if targets.resolve.is_some() {
                        wgpu::StoreOp::Discard
                    } else {
                        wgpu::StoreOp::Store
                    },
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_bind_group(0, &self.frame_group, &[]);
        let offset =
            |i: usize| u32::try_from(i as u64 * DRAW_STRIDE).expect("draw offset fits u32");
        let draw = |pass: &mut wgpu::RenderPass<'_>, i: usize| {
            pass.set_bind_group(1, &self.draw_group, &[offset(i)]);
            self.meshes.get(self.draws[i].mesh).draw(pass);
        };
        pass.set_pipeline(&self.pipelines.lit);
        for i in 0..self.opaque_count {
            draw(&mut pass, i);
        }
        pass.set_pipeline(&self.pipelines.sky);
        pass.set_bind_group(1, &self.draw_group, &[0]);
        pass.draw(0..3, 0..1);
        if self.draws.len() > self.opaque_count {
            pass.set_pipeline(&self.pipelines.guide);
            for i in self.opaque_count..self.draws.len() {
                draw(&mut pass, i);
            }
        }
    }
}

fn draw_buffers(
    device: &wgpu::Device,
    pipelines: &Pipelines,
    capacity: usize,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("draw uniforms"),
        size: capacity as u64 * DRAW_STRIDE,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("draw uniforms"),
        layout: &pipelines.draw_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: NonZeroU64::new((DRAW_FLOATS * 4) as u64),
            }),
        }],
    });
    (buffer, group)
}

/// Inverse transpose of the model's 3x3; identity for a matrix that cannot be inverted (a
/// zero scale), whose normals do not matter.
fn normal_matrix(model: Mat4) -> Mat3 {
    let m = Mat3::from_mat4(model);
    if m.determinant().abs() < 1e-12 {
        Mat3::IDENTITY
    } else {
        m.inverse().transpose()
    }
}

/// The scene targets' size for a view of `size_px`, each side clamped to the device limit (the
/// copy into the view stretches it back); `None` when there is nothing to draw.
fn drawable_size(size_px: [u32; 2], max_dimension: u32) -> Option<[u32; 2]> {
    if size_px.contains(&0) {
        return None;
    }
    Some(size_px.map(|side| side.min(max_dimension.max(1))))
}

/// The scene targets' sample count for the caller's `msaa_samples`, and the reason when it differs.
/// wgpu guarantees 1 and 4 samples for the scene's colour and depth formats, on every backend.
fn scene_samples(msaa_samples: u32) -> (u32, Option<String>) {
    match msaa_samples {
        0 | 1 => (1, None),
        4 => (4, None),
        n => (
            4,
            Some(format!(
                "render: {n}x MSAA is not guaranteed for the scene targets; using 4x"
            )),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zero_side_has_nothing_to_draw() {
        assert_eq!(drawable_size([0, 0], 8192), None);
        assert_eq!(drawable_size([0, 600], 8192), None);
        assert_eq!(drawable_size([800, 0], 8192), None);
    }

    #[test]
    fn sides_are_clamped_to_the_device_limit() {
        assert_eq!(drawable_size([800, 600], 8192), Some([800, 600]));
        assert_eq!(drawable_size([10_000, 600], 8192), Some([8192, 600]));
    }

    #[test]
    fn only_one_and_four_samples_are_used() {
        assert_eq!(scene_samples(1), (1, None));
        assert_eq!(scene_samples(4), (4, None));
        let (samples, note) = scene_samples(8);
        assert_eq!(samples, 4);
        assert!(note.is_some_and(|n| n.contains("8x")));
    }

    #[test]
    fn a_flat_scale_gets_an_identity_normal_matrix() {
        let flat = Mat4::from_scale(Vec3::new(1.0, 0.0, 1.0));
        assert_eq!(normal_matrix(flat), Mat3::IDENTITY);
        let stretched = Mat4::from_scale(Vec3::new(2.0, 1.0, 1.0));
        assert!((normal_matrix(stretched).x_axis.x - 0.5).abs() < 1e-6);
    }
}
