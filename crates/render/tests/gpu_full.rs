//! Renders real frames on the full path (HDR main pass, resolve, composite) and reads them back.
//! Without a GPU adapter, or on one that cannot run the full path, the tests print a note and pass.

// Pixel rows and bytes are small; the casts between them and floats are exact.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use bowlingbox_render::{
    Camera, Capabilities, FullPathOptions, RenderMode, SceneDescription, SceneGeometry,
    SceneRenderer, THEMES, ThemeId, Transform, choose_path, linear_to_srgb, srgb_to_linear_vec3,
    theme,
};
use glam::{Vec3, Vec4Swizzles};

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

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    caps: Capabilities,
}

fn adapter() -> Option<wgpu::Adapter> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let request = |force_fallback_adapter| {
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            force_fallback_adapter,
            ..Default::default()
        }))
    };
    let Ok(adapter) = request(false).or_else(|_| request(true)) else {
        println!("note: no GPU adapter; skipping the full-path test");
        return None;
    };
    let info = adapter.get_info();
    println!("adapter: {} ({:?})", info.name, info.backend);
    Some(adapter)
}

fn device(adapter: &wgpu::Adapter, limits: wgpu::Limits) -> Option<Gpu> {
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("gpu full"),
        required_limits: limits,
        ..Default::default()
    }))
    .map_err(|err| eprintln!("note: device request failed ({err}); skipping the full-path test"))
    .ok()?;
    let caps = Capabilities::probe(adapter, &device);
    Some(Gpu {
        device,
        queue,
        caps,
    })
}

/// A device with all the adapter's limits, as the app requests; `None` (with a note) when the
/// adapter cannot run the full path.
fn full_gpu() -> Option<Gpu> {
    let adapter = adapter()?;
    let gpu = device(&adapter, adapter.limits())?;
    println!("capabilities: {:?}", gpu.caps);
    if let Err(reason) = choose_path(RenderMode::Full, &gpu.caps) {
        println!("note: {reason}; skipping the full-path test");
        return None;
    }
    Some(gpu)
}

fn full_renderer(gpu: &Gpu, samples: u32, id: ThemeId) -> SceneRenderer {
    let renderer = SceneRenderer::new(
        &gpu.device,
        &gpu.queue,
        FORMAT,
        samples,
        G,
        id,
        RenderMode::Full,
        &gpu.caps,
    );
    assert_eq!(
        renderer.path(),
        RenderMode::Full,
        "fell back: {:?}",
        renderer.fallback_reason()
    );
    assert_eq!(renderer.fallback_reason(), None);
    renderer
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
        label: Some("full target"),
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
                label: Some("full pass"),
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
        label: Some("full readback"),
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

/// The sky's sRGB bytes at row `y`: the C++ gradient through the composite's tone map.
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
    let Some(gpu) = full_gpu() else { return };
    for id in THEMES {
        let mut renderer = full_renderer(&gpu, 1, id);
        let lane = render(&gpu, &mut renderer, 1, &lane_camera(), &empty_scene());
        let centre = lane.at(W / 2, H / 2);
        assert!(
            !close(centre, sky_bytes(id, H / 2)),
            "{id:?}: centre {centre:?} is sky"
        );
        assert!(
            centre.iter().sum::<i32>() > 60,
            "{id:?}: centre {centre:?} is black"
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
fn a_ball_at_plus_x_lands_right_of_centre() {
    let Some(gpu) = full_gpu() else { return };
    let mut renderer = full_renderer(&gpu, 1, ThemeId::NeonHall);
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

/// The pixel that shows the world point `p`.
fn pixel(camera: &Camera, p: Vec3) -> (u32, u32) {
    let clip = camera.projection() * camera.view() * p.extend(1.0);
    let ndc = clip.xy() / clip.w;
    let x = (ndc.x * 0.5 + 0.5) * W as f32;
    let y = (0.5 - ndc.y * 0.5) * H as f32;
    assert!(
        (0.0..W as f32).contains(&x) && (0.0..H as f32).contains(&y),
        "{p} is off screen"
    );
    (x as u32, y as u32)
}

fn brightness(rgb: [i32; 3]) -> i32 {
    rgb.iter().sum()
}

#[test]
fn the_ball_shadows_the_lane_on_the_side_away_from_the_sun() {
    let Some(gpu) = full_gpu() else { return };
    let mut renderer = full_renderer(&gpu, 1, ThemeId::Outdoor);
    // From the left, in front of the ball: Outdoor's sun shines towards -x and +z, so the shadow
    // falls to the ball's left and lies in view.
    let mut camera = Camera::default();
    camera.look_at(Vec3::new(-1.0, 0.8, 0.0), Vec3::new(0.0, 0.0, 1.0));
    camera.set_viewport(W, H);
    let scene = ball_at(Vec3::new(0.0, G.ball_radius, 1.0));
    renderer.set_full_path_options(FullPathOptions {
        shadows: false,
        ..FullPathOptions::default()
    });
    let lit = render(&gpu, &mut renderer, 1, &camera, &scene);
    renderer.set_full_path_options(FullPathOptions::default());
    let shadowed = render(&gpu, &mut renderer, 1, &camera, &scene);

    // On the lane, inside the ball's shadow ellipse and not behind the ball.
    let (x, y) = pixel(&camera, Vec3::new(-0.14, 0.0, 1.0));
    let (with, without) = (shadowed.at(x, y), lit.at(x, y));
    assert!(
        brightness(with) + 60 < brightness(without),
        "shadow side ({x}, {y}): {with:?} with shadows, {without:?} without"
    );
    // On the lane, away from every caster.
    let (x, y) = pixel(&camera, Vec3::new(0.35, 0.0, 0.5));
    let (with, without) = (shadowed.at(x, y), lit.at(x, y));
    assert!(
        close(with, without),
        "open lane ({x}, {y}): {with:?} with shadows, {without:?} without"
    );
}

/// The ten pins standing in the rack, as the physics world places them.
fn racked_pins() -> SceneDescription {
    let s = G.pin_spacing;
    let row = s * 0.866_025_4;
    let spots = [
        (0.0, 0.0),
        (-s / 2.0, row),
        (s / 2.0, row),
        (-s, 2.0 * row),
        (0.0, 2.0 * row),
        (s, 2.0 * row),
        (-1.5 * s, 3.0 * row),
        (-s / 2.0, 3.0 * row),
        (s / 2.0, 3.0 * row),
        (1.5 * s, 3.0 * row),
    ];
    let mut scene = empty_scene();
    for (pin, (x, dz)) in scene.pins.iter_mut().zip(spots) {
        pin.position = Vec3::new(x, G.pin_com_height, G.head_pin_z + dz);
    }
    scene.pin_visible = [true; 10];
    scene
}

#[test]
fn standing_pins_are_mirrored_in_the_lane_in_front_of_the_deck() {
    let Some(gpu) = full_gpu() else { return };
    // Low over the lane, looking at the head pin.
    let mut camera = Camera::default();
    camera.look_at(
        Vec3::new(0.0, 0.3, G.head_pin_z - 1.9),
        Vec3::new(0.0, 0.1, G.head_pin_z),
    );
    camera.set_viewport(W, H);
    // The head pin's belly mirrored below the lane: the lane pixel in front of the deck that
    // shows it.
    let (x, y) = pixel(&camera, Vec3::new(0.0, -0.2, G.head_pin_z));
    let scene = racked_pins();
    for id in THEMES {
        let mut renderer = full_renderer(&gpu, 1, id);
        renderer.set_full_path_options(FullPathOptions {
            reflection: false,
            ..FullPathOptions::default()
        });
        let plain = render(&gpu, &mut renderer, 1, &camera, &scene);
        renderer.set_full_path_options(FullPathOptions::default());
        let mirrored = render(&gpu, &mut renderer, 1, &camera, &scene);
        let (with, without) = (mirrored.at(x, y), plain.at(x, y));
        // The lit wood is about as bright as the pin, so the reflection shows as the white pin
        // pulling the orange wood towards blue.
        assert!(
            with[2] >= without[2] + 8,
            "{id:?} ({x}, {y}): {with:?} with reflection, {without:?} without"
        );
    }
}

#[test]
fn multisampled_pass_theme_switch_resize_and_aim_guide_render_without_errors() {
    let Some(gpu) = full_gpu() else { return };
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut renderer = full_renderer(&gpu, 4, ThemeId::NeonHall);
    renderer.set_full_path_options(FullPathOptions::default());
    assert_eq!(renderer.full_path_options(), FullPathOptions::default());
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
    // A new size rebuilds the targets; the full path keeps running.
    let commands = renderer.prepare(&gpu.device, &gpu.queue, &camera, &scene, [200, 100]);
    gpu.queue.submit(commands);
    assert_eq!(renderer.path(), RenderMode::Full);
    let error = pollster::block_on(scope.pop());
    assert!(error.is_none(), "validation error: {error:?}");
}

#[test]
fn a_zero_size_rect_draws_nothing() {
    let Some(gpu) = full_gpu() else { return };
    let mut renderer = full_renderer(&gpu, 1, ThemeId::NeonHall);
    let camera = lane_camera();
    for size in [[0, 0], [0, 180], [320, 0]] {
        let commands = renderer.prepare(&gpu.device, &gpu.queue, &camera, &empty_scene(), size);
        assert!(commands.is_none(), "{size:?} produced commands");
    }
}

#[test]
fn a_webgl2_limited_device_falls_back_to_the_basic_path_with_a_reason() {
    let Some(adapter) = adapter() else { return };
    let Some(gpu) = device(&adapter, wgpu::Limits::downlevel_webgl2_defaults()) else {
        return;
    };
    let renderer = SceneRenderer::new(
        &gpu.device,
        &gpu.queue,
        FORMAT,
        1,
        G,
        ThemeId::NeonHall,
        RenderMode::Full,
        &gpu.caps,
    );
    assert_eq!(renderer.path(), RenderMode::Basic);
    let reason = renderer.fallback_reason().expect("a fallback reason");
    assert!(reason.contains("4096"), "{reason}");
    assert!(!reason.starts_with("render: "), "{reason}");
}

#[test]
fn the_basic_mode_reports_the_environment_variable() {
    let Some(adapter) = adapter() else { return };
    let Some(gpu) = device(&adapter, adapter.limits()) else {
        return;
    };
    let renderer = SceneRenderer::new(
        &gpu.device,
        &gpu.queue,
        FORMAT,
        1,
        G,
        ThemeId::NeonHall,
        RenderMode::Basic,
        &gpu.caps,
    );
    assert_eq!(renderer.path(), RenderMode::Basic);
    assert_eq!(
        renderer.fallback_reason(),
        Some("basic path (BOWLINGBOX_RENDER=basic)")
    );
}
