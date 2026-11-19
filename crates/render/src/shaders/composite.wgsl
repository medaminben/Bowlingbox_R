// The full path's last pass (C++ kCompositeBody): the resolved HDR scene plus the bloom, exposed,
// ACES tone mapped and sRGB encoded into the renderer's 8-bit output target. Follows
// tonemap.wgsl.

struct Post {
    params: vec4<f32>,  // exposure, bloom strength (0: no bloom this frame), unused x2
}

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var<uniform> post: Post;
@group(0) @binding(2) var bloom: texture_2d<f32>;
@group(0) @binding(3) var bloom_sampler: sampler;

@vertex
fn vs_post(@builtin(vertex_index) index: u32) -> Varyings {
    let p = vec2(select(-1.0, 3.0, index == 1u), select(-1.0, 3.0, index == 2u));
    return Varyings(vec4(p, 0.0, 1.0), vec2(p.x + 1.0, 1.0 - p.y) * 0.5);
}

@fragment
fn fs_composite(in: Varyings) -> @location(0) vec4<f32> {
    var c = textureLoad(scene, vec2<i32>(in.position.xy), 0).rgb;
    let strength = post.params.y;
    if strength > 0.0 {
        c += textureSampleLevel(bloom, bloom_sampler, in.uv, 0.0).rgb * strength;
    }
    return vec4(to_display(c, post.params.x), 1.0);
}
