// The full path's last pass (C++ kCompositeBody without bloom so far): the resolved HDR scene,
// exposed, ACES tone mapped and sRGB encoded into the renderer's 8-bit output target. Follows
// tonemap.wgsl.

struct Post {
    params: vec4<f32>,  // exposure, unused x3
}

@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var<uniform> post: Post;

@vertex
fn vs_post(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let p = vec2(select(-1.0, 3.0, index == 1u), select(-1.0, 3.0, index == 2u));
    return vec4(p, 0.0, 1.0);
}

@fragment
fn fs_composite(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let c = textureLoad(scene, vec2<i32>(frag.xy), 0).rgb;
    return vec4(to_display(c, post.params.x), 1.0);
}
