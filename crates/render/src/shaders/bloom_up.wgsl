// The bloom chain's way up (C++ kBloomUpFs): a 3x3 tent over each level, added onto the level
// above it (the pipeline blends one + one).

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

fn at(uv: vec2<f32>, texel: vec2<f32>, x: f32, y: f32) -> vec3<f32> {
    return textureSample(src, src_sampler, uv + texel * vec2(x, y)).rgb;
}

@fragment
fn fs_bloom_up(in: Varyings) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(src));
    let uv = in.uv;
    let r = 4.0 * at(uv, texel, 0.0, 0.0)
          + 2.0 * (at(uv, texel, 0.0, 1.0) + at(uv, texel, -1.0, 0.0) + at(uv, texel, 1.0, 0.0)
                   + at(uv, texel, 0.0, -1.0))
          + (at(uv, texel, -1.0, 1.0) + at(uv, texel, 1.0, 1.0) + at(uv, texel, -1.0, -1.0)
             + at(uv, texel, 1.0, -1.0));
    return vec4(r / 16.0, 1.0);
}
