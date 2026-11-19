//! Renders real frames offscreen and reads them back. Without a GPU adapter the tests print a note
//! and pass.

// Pixel rows and bytes are small; the casts between them and floats are exact.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

use bowlingbox_render::{
    Camera, SceneDescription, SceneGeometry, SceneRenderer, THEMES, ThemeId, Transform,
    linear_to_srgb, srgb_to_linear_vec3, theme,
};
use glam::Vec3;
use std::sync::{Mutex, MutexGuard, PoisonError};

const G: SceneGeometry = SceneGeometry {
    lane_half_width: 0.525,
    lane_length: 18.3,
    approach: 2.0,
    gutter_width: 0.24,
    gutter_depth: 0.05,
    ball_radius: 0.109,
    pin_height: 0.38,
    pin_max_radius: 0.06,
    pin_com_height: 0.12,
    pin_spacing: 0.3048,
    head_pin_z: 17.4,
};

const W: u32 = 320;
const H: u32 = 180;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
/// sRGB bytes; the scene goes through an 8-bit target before the sRGB one.
const TOLERANCE: i32 = 3;

/// Creating instances and pipelines concurrently crashes some Vulkan loaders, so every GPU test
/// holds this lock from the first instance until its device is dropped.
static GPU_LOCK: Mutex<()> = Mutex::new(());

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    _lock: MutexGuard<'static, ()>,
}

/// A device limited to the WebGL2 downlevel defaults, so the test also checks the basic path stays
/// within what wgpu's GL backend offers.
fn gpu() -> Option<Gpu> {
    let lock = GPU_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        force_fallback_adapter: false,
        ..Default::default()
    }))
    .or_else(|_| {
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            force_fallback_adapter: true,
            ..Default::default()
        }))
    });
    let Ok(adapter) = adapter else {
        println!("note: no GPU adapter; skipping the GPU smoke test");
        return None;
    };
    let info = adapter.get_info();
    println!("adapter: {} ({:?})", info.name, info.backend);
    let limits = wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("gpu smoke"),
        required_limits: limits,
        ..Default::default()
    }))
    .map_err(|err| eprintln!("note: device request failed ({err}); skipping the GPU smoke test"))
    .ok()?;
    Some(Gpu {
        device,
        queue,
        _lock: lock,
    })
}

fn ball_at(position: Vec3) -> SceneDescription {
    SceneDescription {
        ball: Transform {
            position,
            ..Transform::default()
        },
        ..SceneDescription::default()
    }
}

/// The ball out of sight, under the floor.
fn empty_scene() -> SceneDescription {
    ball_at(Vec3::new(0.0, -50.0, 0.0))
}

struct Image {
    pixels: Vec<u8>,
}

impl Image {
    fn at(&self, x: u32, y: u32) -> [i32; 3] {
        let i = ((y * W + x) * 4) as usize;
        [
            i32::from(self.pixels[i]),
            i32::from(self.pixels[i + 1]),
            i32::from(self.pixels[i + 2]),
        ]
    }
}

fn target(gpu: &Gpu, samples: u32, usage: wgpu::TextureUsages) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("smoke target"),
        size: wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: samples,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | usage,
        view_formats: &[],
    })
}

/// One frame through `prepare` and `paint`, read back from an sRGB target. `samples` is the
/// pass's sample count, as given to `SceneRenderer::new`.
fn render(
    gpu: &Gpu,
    renderer: &mut SceneRenderer,
    samples: u32,
    camera: &Camera,
    scene: &SceneDescription,
) -> Image {
    let target = target(gpu, 1, wgpu::TextureUsages::COPY_SRC);
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let multisampled = (samples > 1).then(|| {
        self::target(gpu, samples, wgpu::TextureUsages::empty())
            .create_view(&wgpu::TextureViewDescriptor::default())
    });
    let scene_commands = renderer.prepare(&gpu.device, &gpu.queue, camera, scene, [W, H]);
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("smoke pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: multisampled.as_ref().unwrap_or(&view),
                    depth_slice: None,
                    resolve_target: multisampled.as_ref().map(|_| &view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            })
            .forget_lifetime();
        renderer.paint(&mut pass);
    }
    let bytes_per_row = W * 4;
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("smoke readback"),
        size: u64::from(bytes_per_row * H),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(H),
            },
        },
        wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue
        .submit(scene_commands.into_iter().chain([encoder.finish()]));
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |result| result.expect("map readback"));
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    let pixels = slice.get_mapped_range().expect("mapped range").to_vec();
    readback.unmap();
    Image { pixels }
}

fn aces(x: f32) -> f32 {
    (x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0)
}

/// The sky's sRGB bytes at row `y`: the C++ gradient, tone mapped as the basic path does.
fn sky_bytes(id: ThemeId, y: u32) -> [i32; 3] {
    let t = theme(id);
    let v = 1.0 - (y as f32 + 0.5) / H as f32;
    let linear =
        srgb_to_linear_vec3(t.background_bottom).lerp(srgb_to_linear_vec3(t.background_top), v);
    let byte = |c: f32| (linear_to_srgb(aces(c * t.exposure)) * 255.0).round() as i32;
    [byte(linear.x), byte(linear.y), byte(linear.z)]
}

fn close(a: [i32; 3], b: [i32; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() <= TOLERANCE)
}

/// Above the hall, looking up: nothing but sky.
fn sky_camera() -> Camera {
    let mut camera = Camera::default();
    camera.look_at(Vec3::new(0.0, 30.0, 6.0), Vec3::new(0.0, 40.0, 30.0));
    camera.set_viewport(W, H);
    camera
}

fn lane_camera() -> Camera {
    let mut camera = Camera::default();
    camera.set_viewport(W, H);
    camera
}

#[test]
fn every_theme_draws_the_lane_in_front_of_the_sky() {
    let Some(gpu) = gpu() else { return };
    for id in THEMES {
        let mut renderer = SceneRenderer::new(&gpu.device, &gpu.queue, FORMAT, 1, G, id);
        let lane = render(&gpu, &mut renderer, 1, &lane_camera(), &empty_scene());
        let centre = lane.at(W / 2, H / 2);
        assert!(
            !close(centre, sky_bytes(id, H / 2)),
            "{id:?}: centre {centre:?} is sky"
        );
        // The lane's lit top face: warm wood. The underside (wrong winding) would be near black.
        assert!(
            centre[0] > 100 && centre[0] > centre[1] && centre[1] > centre[2],
            "{id:?}: centre {centre:?} is not lit wood"
        );

        let sky = render(&gpu, &mut renderer, 1, &sky_camera(), &empty_scene());
        for y in [0, H / 2, H - 1] {
            for x in [0, W / 2, W - 1] {
                let got = sky.at(x, y);
                let want = sky_bytes(id, y);
                assert!(
                    close(got, want),
                    "{id:?}: sky at ({x}, {y}) {got:?}, want {want:?}"
                );
            }
        }
    }
}

#[test]
fn outdoor_shows_the_sky_above_the_lane() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = SceneRenderer::new(&gpu.device, &gpu.queue, FORMAT, 1, G, ThemeId::Outdoor);
    let image = render(&gpu, &mut renderer, 1, &lane_camera(), &empty_scene());
    let got = image.at(W / 2, 0);
    let want = sky_bytes(ThemeId::Outdoor, 0);
    assert!(close(got, want), "top row {got:?}, want {want:?}");
}

#[test]
fn a_ball_at_plus_x_lands_right_of_centre() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = SceneRenderer::new(&gpu.device, &gpu.queue, FORMAT, 1, G, ThemeId::NeonHall);
    let camera = lane_camera();
    let without = render(&gpu, &mut renderer, 1, &camera, &empty_scene());
    let with = render(
        &gpu,
        &mut renderer,
        1,
        &camera,
        &ball_at(Vec3::new(0.35, G.ball_radius, 1.0)),
    );
    let (mut count, mut sum_x, mut min_x) = (0_u32, 0_u32, W);
    for y in 0..H {
        for x in 0..W {
            if without.at(x, y) != with.at(x, y) {
                count += 1;
                sum_x += x;
                min_x = min_x.min(x);
            }
        }
    }
    assert!(count > 20, "the ball covers only {count} pixels");
    assert!(
        min_x > W / 2,
        "ball pixels reach x = {min_x}, left of centre"
    );
    assert!(sum_x / count > W / 2);
}

#[test]
fn multisampled_theme_switch_and_aim_guide_render_without_errors() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = SceneRenderer::new(&gpu.device, &gpu.queue, FORMAT, 4, G, ThemeId::NeonHall);
    let mut scene = empty_scene();
    scene.show_aim_guide = true;
    scene.aim_angle_deg = 2.0;
    let camera = lane_camera();
    let plain = {
        let mut no_guide = scene;
        no_guide.show_aim_guide = false;
        render(&gpu, &mut renderer, 4, &camera, &no_guide)
    };
    let guided = render(&gpu, &mut renderer, 4, &camera, &scene);
    assert_ne!(plain.pixels, guided.pixels, "the aim guide changed nothing");
    for id in THEMES {
        renderer.set_theme(id);
        let _ = render(&gpu, &mut renderer, 4, &camera, &scene);
    }
}

#[test]
fn zero_msaa_samples_clamp_to_one_and_render_without_errors() {
    let Some(gpu) = gpu() else { return };
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut renderer = SceneRenderer::new(&gpu.device, &gpu.queue, FORMAT, 0, G, ThemeId::NeonHall);
    // The scene clamps 0 to 1 sample, so the pass is single-sampled.
    let _ = render(&gpu, &mut renderer, 1, &lane_camera(), &empty_scene());
    let error = pollster::block_on(scope.pop());
    assert!(error.is_none(), "validation error: {error:?}");
}

#[test]
fn a_zero_size_rect_draws_nothing() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = SceneRenderer::new(&gpu.device, &gpu.queue, FORMAT, 1, G, ThemeId::NeonHall);
    let camera = lane_camera();
    for size in [[0, 0], [0, 180], [320, 0]] {
        let commands = renderer.prepare(&gpu.device, &gpu.queue, &camera, &empty_scene(), size);
        assert!(commands.is_none(), "{size:?} produced commands");
    }
}
