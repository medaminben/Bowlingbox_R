// Shared by lit.wgsl, guide.wgsl and sky.wgsl: the per-frame and per-draw uniforms and the mesh
// vertex stage. The renderer fills the uniforms in the same field order.

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
    light_vp: mat4x4<f32>,     // the key light's view projection (shadow map)
    shadow: vec4<f32>,         // shadows 0/1 (full path only), unused
    clip_plane: vec4<f32>,     // world space, mirrored pass only; (0, 0, 0, 1) keeps everything
    reflect: vec4<f32>,        // scene target width, height; lane reflection 0/1 (full path), unused
}

struct Draw {
    model: mat4x4<f32>,
    normal_mat: mat4x4<f32>,   // inverse transpose of the model's 3x3
    color: vec4<f32>,          // linear rgb, alpha
    band: vec4<f32>,           // neck stripe min y, max y (object space); emissive 0/1, emissive strength
    surface: vec4<f32>,        // roughness, specular, pattern (0 none, 1 wood, 2 ball swirl), fog 0/1
    flags: vec4<f32>,          // the main lane 0/1 (shows the reflection), unused
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<uniform> draw: Draw;

struct MeshIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct MeshOut {
    // Invariant: the depth pass and the lit pass must compute the same depth (Equal test).
    @builtin(position) @invariant clip: vec4<f32>,
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
