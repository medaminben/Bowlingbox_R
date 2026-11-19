// Copies the renderer's finished scene (sRGB-encoded bytes in an Rgba8Unorm texture) into the
// caller's render pass, inside the viewport the caller set.

@group(0) @binding(0) var scene: texture_2d<f32>;

struct BlitOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,  // 0,0 at the top left
}

@vertex
fn vs_blit(@builtin(vertex_index) index: u32) -> BlitOut {
    let p = vec2(select(-1.0, 3.0, index == 1u), select(-1.0, 3.0, index == 2u));
    var out: BlitOut;
    out.uv = vec2((p.x + 1.0) * 0.5, (1.0 - p.y) * 0.5);
    out.clip = vec4(p, 0.0, 1.0);
    return out;
}

fn scene_at(uv: vec2<f32>) -> vec3<f32> {
    let size = vec2<f32>(textureDimensions(scene));
    let texel = vec2<i32>(clamp(uv * size, vec2(0.0), size - 1.0));
    return textureLoad(scene, texel, 0).rgb;
}

// For a target that stores the bytes as they are.
@fragment
fn fs_blit_encoded(in: BlitOut) -> @location(0) vec4<f32> {
    return vec4(scene_at(in.uv), 1.0);
}

// For an sRGB target, which encodes on write: decode first so the bytes come out unchanged.
@fragment
fn fs_blit_linear(in: BlitOut) -> @location(0) vec4<f32> {
    let c = scene_at(in.uv);
    let decoded = mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(vec3(0.04045), c));
    return vec4(decoded, 1.0);
}
