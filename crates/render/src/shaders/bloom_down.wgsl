// The bloom chain's way down (C++ kBrightFs, kBloomDownFs): `fs_bright` keeps what is brighter
// than 1 from the resolved scene in the first level; `fs_bloom_down` filters each level into the
// next, half the size.

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

@fragment
fn fs_bright(in: Varyings) -> @location(0) vec4<f32> {
    let c = min(textureSample(src, src_sampler, in.uv).rgb, vec3(MAX_BRIGHT));
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
