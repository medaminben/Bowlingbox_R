// Shared by lit.wgsl, guide.wgsl and sky.wgsl: the per-frame and per-draw uniforms, the tone map
// and the mesh vertex stage. The renderer fills the uniforms in the same field order.

struct Frame {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    light_dir: vec4<f32>,      // normalised, the direction the light travels
    key_color: vec4<f32>,      // key colour * key intensity, linear
    sky_ambient: vec4<f32>,
    ground_ambient: vec4<f32>,
    fog_color: vec4<f32>,      // linear
    band_color: vec4<f32>,     // pin neck stripe, linear
    sky_top: vec4<f32>,        // linear
    sky_bottom: vec4<f32>,     // linear
    point_pos: array<vec4<f32>, 4>,   // xyz position, w range
    point_color: array<vec4<f32>, 4>, // colour * intensity
    params: vec4<f32>,         // fog start, fog end, exposure, point light count
    lane: vec4<f32>,           // lane half width, board width, pin deck z, unused
}

struct Draw {
    model: mat4x4<f32>,
    normal_mat: mat4x4<f32>,   // inverse transpose of the model's 3x3
    color: vec4<f32>,          // linear rgb, alpha
    band: vec4<f32>,           // neck stripe min y, max y (object space); emissive 0/1, emissive strength
    surface: vec4<f32>,        // roughness, specular, pattern (0 none, 1 wood, 2 ball swirl), fog 0/1
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<uniform> draw: Draw;

fn aces_fit(x: vec3<f32>) -> vec3<f32> {  // Narkowicz 2015
    return clamp(x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14), vec3(0.0), vec3(1.0));
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    return mix(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(vec3(0.0031308), c));
}

fn to_display(hdr: vec3<f32>, exposure: f32) -> vec3<f32> {
    return linear_to_srgb(aces_fit(hdr * exposure));
}

struct MeshIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct MeshOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) local_pos: vec3<f32>,
    @location(3) view_pos: vec3<f32>,
}

@vertex
fn vs_mesh(v: MeshIn) -> MeshOut {
    let world = draw.model * vec4(v.pos, 1.0);
    let view_pos = frame.view * world;
    var out: MeshOut;
    out.world_pos = world.xyz;
    out.local_pos = v.pos;
    out.normal = normalize((draw.normal_mat * vec4(v.normal, 0.0)).xyz);
    out.view_pos = view_pos.xyz;
    out.clip = frame.proj * view_pos;
    return out;
}
