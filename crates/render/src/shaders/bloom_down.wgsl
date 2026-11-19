// The bloom chain's way down (C++ kBrightFs, kBloomDownFs): `fs_bright` keeps what is brighter
// than 1 from the resolved scene in the first level; `fs_bloom_down` filters each level into the
// next, half the size. Follows weight.wgsl.

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

// C++ kPostVs: one triangle over the target; uv runs down the screen, as texture rows do.
@vertex
fn vs_post(@builtin(vertex_index) index: u32) -> Varyings {
    let p = vec2(select(-1.0, 3.0, index == 1u), select(-1.0, 3.0, index == 2u));
    return Varyings(vec4(p, 0.0, 1.0), vec2(p.x + 1.0, 1.0 - p.y) * 0.5);
}

// The ball's highlight reaches ~40 on a few pixels; unclamped, those pixels make the glow
// flicker as the ball rolls. Neon (5) and anything a viewer reads as a light stay below this.
const MAX_BRIGHT: f32 = 16.0;
const THRESHOLD: f32 = 1.0;

// A resolved scene texel with its weight undone (weight.wgsl), clamped to the edge.
fn scene_at(p: vec2<i32>, size: vec2<i32>) -> vec3<f32> {
    return resolve_unweight(textureLoad(src, clamp(p, vec2(0), size - 1), 0).rgb);
}

@fragment
fn fs_bright(in: Varyings) -> @location(0) vec4<f32> {
    // Bilinear by hand: the sampler would filter the weighted texels, not the scene.
    let size = vec2<i32>(textureDimensions(src));
    let x = in.uv * vec2<f32>(size) - 0.5;
    let i = vec2<i32>(floor(x));
    let f = fract(x);
    let top = mix(scene_at(i, size), scene_at(i + vec2(1, 0), size), f.x);
    let bottom = mix(scene_at(i + vec2(0, 1), size), scene_at(i + vec2(1, 1), size), f.x);
    let c = min(mix(top, bottom, f.y), vec3(MAX_BRIGHT));
    let peak = max(max(c.r, c.g), c.b);
    return vec4(c * max(peak - THRESHOLD, 0.0) / max(peak, 1e-4), 1.0);
}

fn at(uv: vec2<f32>, texel: vec2<f32>, x: f32, y: f32) -> vec3<f32> {
    return textureSample(src, src_sampler, uv + texel * vec2(x, y)).rgb;
}

@fragment
fn fs_bloom_down(in: Varyings) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(src));
    let uv = in.uv;
    let r = at(uv, texel, 0.0, 0.0) * 0.125
          + (at(uv, texel, -2.0, 2.0) + at(uv, texel, 2.0, 2.0) + at(uv, texel, -2.0, -2.0)
             + at(uv, texel, 2.0, -2.0)) * 0.03125
          + (at(uv, texel, 0.0, 2.0) + at(uv, texel, -2.0, 0.0) + at(uv, texel, 2.0, 0.0)
             + at(uv, texel, 0.0, -2.0)) * 0.0625
          + (at(uv, texel, -1.0, 1.0) + at(uv, texel, 1.0, 1.0) + at(uv, texel, -1.0, -1.0)
             + at(uv, texel, 1.0, -1.0)) * 0.125;
    return vec4(r, 1.0);
}
