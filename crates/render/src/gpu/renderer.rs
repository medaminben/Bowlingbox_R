//! Draws the lane, pins, ball, decor, sky and aim guide into the renderer's own colour and depth
//! targets, then copies the result into the caller's render pass.

use super::RenderMode;
use super::capabilities::{Capabilities, choose_path};
use super::full::{FullPath, FullPathOptions};
use super::meshes::{MeshKind, Meshes};
use super::pipelines::{
    DEPTH_FORMAT, DRAW_FLOATS, DRAW_STRIDE, FRAME_FLOATS, Pipelines, SCENE_FORMAT, ScenePipelines,
};
use super::targets::FrameSizes;
use crate::camera::Camera;
use crate::decor::{Prop, Shape, build_props};
use crate::lighting::{
    LightFrustum, fit_light_frustum, props_in_box, reflection_box, shadow_box, srgb_to_linear_vec3,
};
use crate::scene::{SceneDescription, SceneGeometry};
use crate::theme::{MAX_POINT_LIGHTS, Material, Pattern, Theme, ThemeId, theme};
use glam::{Mat3, Mat4, Quat, Vec2, Vec3, Vec4};
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
/// The lane and its two gutters, first in the draw list; the mirrored pass leaves them out.
const LANE_DRAWS: usize = 3;
/// A clip plane that keeps everything.
const KEEP_ALL: Vec4 = Vec4::new(0.0, 0.0, 0.0, 1.0);
/// Keeps y >= 0: what lies above the lane plane (C++ reflectionPass).
const ABOVE_LANE: Vec4 = Vec4::new(0.0, 1.0, 0.0, 0.0);

struct DrawItem {
    mesh: MeshKind,
    model: Mat4,
    material: Material,
    band: Vec2,
    alpha: f32,
    fog: bool,
    /// The main lane, which shows the reflection (C++ uReflect).
    lane: bool,
}

/// What differs between the main and the mirrored pass's frame uniforms (C++ `PassParams`).
struct FramePass {
    view: Mat4,
    eye: Vec3,
    shadows: bool,
    clip_plane: Vec4,
    reflection: bool,
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
            lane: false,
        }
    }
}

/// The basic path's colour and depth targets at one size.
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
    /// The key light's camera, fitted to the shadow box.
    light: LightFrustum,
    /// Indices into `props` that cast into the shadow map.
    shadow_casters: Vec<usize>,
    /// Indices into `props` that the lane mirrors.
    reflection_casters: Vec<usize>,
    meshes: Meshes,
    pipelines: Pipelines,
    scene_samples: u32,
    frame_buffer: wgpu::Buffer,
    frame_group: wgpu::BindGroup,
    /// The frame uniforms of the pass mirrored in the lane plane.
    mirror_frame_buffer: wgpu::Buffer,
    mirror_frame_group: wgpu::BindGroup,
    draw_buffer: wgpu::Buffer,
    draw_group: wgpu::BindGroup,
    draw_capacity: usize,
    draws: Vec<DrawItem>,
    /// Index of the first prop in `draws`; everything before it casts a shadow.
    prop_start: usize,
    opaque_count: usize,
    targets: Option<Targets>,
    /// The full path while it runs; `None` on the basic path.
    full: Option<FullPath>,
    /// The longest side the full path's targets get (`Capabilities::max_texture_2d`).
    full_max_side: u32,
    full_options: FullPathOptions,
    /// Why the basic path runs, without the `render: ` prefix.
    fallback: Option<String>,
    /// The last `prepare` drew a frame; `paint` copies it.
    ready: bool,
}

impl SceneRenderer {
    /// `target_format` and `msaa_samples` describe the render pass `paint` draws into. The queue is
    /// not needed yet; it keeps the signature egui-wgpu callbacks have at hand. The full path runs
    /// when `mode` asks for it, `caps` allow it (see `choose_path`) and its pipelines build;
    /// otherwise the basic path runs and `fallback_reason` says why.
    #[must_use]
    // One argument per thing the caller decides; a builder would only rename them.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        msaa_samples: u32,
        geometry: SceneGeometry,
        theme_id: ThemeId,
        mode: RenderMode,
        caps: &Capabilities,
    ) -> Self {
        let msaa_samples = msaa_samples.max(1);
        let (scene_samples, note) = scene_samples(msaa_samples);
        if let Some(note) = note {
            eprintln!("{note}");
        }
        let pipelines = Pipelines::new(device, scene_samples, target_format, msaa_samples);
        let (full, fallback) = match choose_path(mode, caps).and_then(|()| {
            FullPath::new(device, &pipelines, caps.hdr_msaa_samples).map_err(|error| {
                format!("full-path pipelines failed ({error}); using the basic path")
            })
        }) {
            Ok(full) => (Some(full), None),
            Err(reason) => (None, Some(reason)),
        };
        let (frame_buffer, frame_group) = frame_buffers(device, &pipelines, "frame uniforms");
        let (mirror_frame_buffer, mirror_frame_group) =
            frame_buffers(device, &pipelines, "mirrored frame uniforms");
        let current = theme(theme_id);
        let props = build_props(current, &geometry);
        let draw_capacity = props.len() + FIXED_DRAWS;
        let (draw_buffer, draw_group) = draw_buffers(device, &pipelines, draw_capacity);
        let region = shadow_box(&geometry);
        Self {
            meshes: Meshes::new(device, &geometry),
            light: fit_light_frustum(current.light_dir, &region),
            shadow_casters: props_in_box(&props, &geometry, &region),
            reflection_casters: props_in_box(&props, &geometry, &reflection_box(&geometry)),
            geometry,
            theme: current,
            props,
            pipelines,
            scene_samples,
            frame_buffer,
            frame_group,
            mirror_frame_buffer,
            mirror_frame_group,
            draw_buffer,
            draw_group,
            draw_capacity,
            draws: Vec::new(),
            prop_start: 0,
            opaque_count: 0,
            targets: None,
            full,
            full_max_side: caps.max_texture_2d,
            full_options: FullPathOptions::default(),
            fallback,
            ready: false,
        }
    }

    /// The path that draws now. `Full` can turn into `Basic` at a `prepare` (see `fallback_reason`).
    #[must_use]
    pub fn path(&self) -> RenderMode {
        if self.full.is_some() {
            RenderMode::Full
        } else {
            RenderMode::Basic
        }
    }

    /// Why the basic path runs, without the `render: ` prefix; `None` on the full path. A target
    /// that fails to build at a new size switches to the basic path for good, so this can change
    /// after `prepare`.
    #[must_use]
    pub fn fallback_reason(&self) -> Option<&str> {
        self.fallback.as_deref()
    }

    /// Which full-path passes run; takes effect at the next `prepare`.
    pub fn set_full_path_options(&mut self, options: FullPathOptions) {
        self.full_options = options;
    }

    #[must_use]
    pub fn full_path_options(&self) -> FullPathOptions {
        self.full_options
    }

    /// Takes effect at the next `prepare`.
    pub fn set_theme(&mut self, id: ThemeId) {
        self.theme = theme(id);
        self.props = build_props(self.theme, &self.geometry);
        let region = shadow_box(&self.geometry);
        self.shadow_casters = props_in_box(&self.props, &self.geometry, &region);
        self.light = fit_light_frustum(self.theme.light_dir, &region);
        self.reflection_casters =
            props_in_box(&self.props, &self.geometry, &reflection_box(&self.geometry));
    }

    /// The full path runs and draws the shadow map this frame.
    fn shadows(&self) -> bool {
        self.full.is_some() && self.full_options.shadows
    }

    /// The full path runs and mirrors the scene in the lane this frame.
    fn reflection(&self) -> bool {
        self.full.is_some() && self.full_options.reflection
    }

    /// The full path runs the bloom chain this frame (C++: only at a strength above 0).
    fn bloom(&self) -> bool {
        self.full.is_some() && self.full_options.bloom && self.theme.bloom_strength > 0.0
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
        if size_px.contains(&0) {
            self.ready = false;
            return None;
        }
        // A target that fails at a new size (C++ `ensureSize`) ends the full path for good.
        if let Some(full) = &mut self.full {
            let sizes = FrameSizes::new(drawable_size(size_px, self.full_max_side));
            if let Err(error) = full.ensure_targets(device, sizes, &self.pipelines.blit_layout) {
                let [w, h] = sizes.scene;
                self.fallback = Some(format!(
                    "offscreen targets incomplete at {w}x{h} ({error}); using the basic path"
                ));
                self.full = None;
            }
        }
        let size = if let Some(full) = &self.full {
            let strength = if self.bloom() {
                self.theme.bloom_strength
            } else {
                0.0
            };
            full.write_post(queue, self.theme.exposure, strength);
            full.targets().map_or(size_px, |t| t.sizes.scene)
        } else {
            let size = drawable_size(size_px, device.limits().max_texture_dimension_2d);
            self.ensure_targets(device, size);
            size
        };
        let mut camera = *camera;
        camera.set_viewport(size_px[0], size_px[1]);
        let main = FramePass {
            view: camera.view(),
            eye: camera.eye,
            shadows: self.shadows(),
            clip_plane: KEEP_ALL,
            reflection: self.reflection(),
        };
        queue.write_buffer(
            &self.frame_buffer,
            0,
            bytemuck::cast_slice(&self.frame_uniforms(&camera, &main, size)),
        );
        if self.reflection() {
            // C++ reflectionPass: the view mirrored in y = 0, no shadows, no reflection.
            let mirrored = FramePass {
                view: camera.view() * Mat4::from_scale(Vec3::new(1.0, -1.0, 1.0)),
                eye: camera.eye * Vec3::new(1.0, -1.0, 1.0),
                shadows: false,
                clip_plane: ABOVE_LANE,
                reflection: false,
            };
            queue.write_buffer(
                &self.mirror_frame_buffer,
                0,
                bytemuck::cast_slice(&self.frame_uniforms(&camera, &mirrored, size)),
            );
        }
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
        if !self.ready {
            return;
        }
        let group = match &self.full {
            Some(full) => full.targets().map(|t| &t.blit_group),
            None => self.targets.as_ref().map(|t| &t.blit_group),
        };
        let Some(group) = group else {
            return;
        };
        pass.set_pipeline(&self.pipelines.blit);
        pass.set_bind_group(0, group, &[]);
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

    /// `size` is the scene target's, which the lane's reflection lookup divides by.
    fn frame_uniforms(&self, camera: &Camera, pass: &FramePass, size: [u32; 2]) -> Vec<f32> {
        let t = self.theme;
        let g = &self.geometry;
        let flag = |b: bool| f32::from(u8::from(b));
        let mut out = Vec::with_capacity(FRAME_FLOATS);
        out.extend_from_slice(&pass.view.to_cols_array());
        out.extend_from_slice(&camera.projection().to_cols_array());
        for v in [
            pass.eye,
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
        out.extend_from_slice(&self.light.view_projection().to_cols_array());
        out.extend_from_slice(&[flag(pass.shadows), 0.0, 0.0, 0.0]);
        out.extend_from_slice(&pass.clip_plane.to_array());
        out.extend_from_slice(&[size[0] as f32, size[1] as f32, flag(pass.reflection), 0.0]);
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
        self.draws.push(DrawItem {
            lane: true,
            ..DrawItem::opaque(
                unit_box,
                block(
                    Vec3::new(0.0, -0.05, (g.lane_length - g.approach) / 2.0),
                    Vec3::new(2.0 * g.lane_half_width, 0.1, g.lane_length + g.approach),
                ),
                t.lane,
                NO_BAND,
            )
        });
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
        self.prop_start = self.draws.len();
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
                lane: false,
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
                [flag(d.lane), 0.0, 0.0, 0.0],
            ];
            let values = fields.iter().flatten().chain(tail.iter().flatten());
            for (dst, src) in slot.iter_mut().zip(values) {
                *dst = *src;
            }
        }
        out
    }

    fn record(&self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(full) = &self.full {
            let options = FullPathOptions {
                shadows: self.shadows(),
                reflection: self.reflection(),
                bloom: self.bloom(),
            };
            full.record(
                encoder,
                options,
                |pass, pipeline| self.draw_casters(pass, pipeline),
                |pass, pipeline| self.draw_mirrored(pass, pipeline),
                |pass| self.draw_scene(pass, full.scene()),
            );
            return;
        }
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
        self.draw_scene(&mut pass, &self.pipelines.scene);
    }

    /// What C++ `shadowPass` draws: lane, gutters, walls, pins and ball, then the props in the
    /// shadow box.
    fn draw_casters(&self, pass: &mut wgpu::RenderPass<'_>, pipeline: &wgpu::RenderPipeline) {
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.frame_group, &[]);
        let props = self.shadow_casters.iter().map(|i| self.prop_start + i);
        for i in (0..self.prop_start).chain(props) {
            self.draw(pass, i);
        }
    }

    /// What C++ `reflectionPass` draws: walls, pins and ball, then the props in the reflection
    /// box; not the lane and gutters, which the mirror lies in.
    fn draw_mirrored(&self, pass: &mut wgpu::RenderPass<'_>, pipeline: &wgpu::RenderPipeline) {
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.mirror_frame_group, &[]);
        let props = self.reflection_casters.iter().map(|i| self.prop_start + i);
        for i in (LANE_DRAWS..self.prop_start).chain(props) {
            self.draw(pass, i);
        }
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, i: usize) {
        let offset = u32::try_from(i as u64 * DRAW_STRIDE).expect("draw offset fits u32");
        pass.set_bind_group(1, &self.draw_group, &[offset]);
        self.meshes.get(self.draws[i].mesh).draw(pass);
    }

    /// The opaque draws, the sky behind them, then the translucent aim guide.
    fn draw_scene(&self, pass: &mut wgpu::RenderPass<'_>, pipelines: &ScenePipelines) {
        pass.set_bind_group(0, &self.frame_group, &[]);
        pass.set_pipeline(&pipelines.lit);
        for i in 0..self.opaque_count {
            self.draw(pass, i);
        }
        pass.set_pipeline(&pipelines.sky);
        pass.set_bind_group(1, &self.draw_group, &[0]);
        pass.draw(0..3, 0..1);
        if self.draws.len() > self.opaque_count {
            pass.set_pipeline(&pipelines.guide);
            for i in self.opaque_count..self.draws.len() {
                self.draw(pass, i);
            }
        }
    }
}

fn frame_buffers(
    device: &wgpu::Device,
    pipelines: &Pipelines,
    label: &str,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (FRAME_FLOATS * 4) as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout: &pipelines.frame_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });
    (buffer, group)
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

/// The scene targets' size for a view of `size_px` with no zero side, each side clamped to
/// `max_dimension` (the copy into the view stretches it back).
fn drawable_size(size_px: [u32; 2], max_dimension: u32) -> [u32; 2] {
    size_px.map(|side| side.min(max_dimension.max(1)))
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
    fn sides_are_clamped_to_the_device_limit() {
        assert_eq!(drawable_size([800, 600], 8192), [800, 600]);
        assert_eq!(drawable_size([10_000, 600], 8192), [8192, 600]);
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
