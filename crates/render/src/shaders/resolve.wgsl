// The full path's MSAA resolve (C++ kResolveFs). A plain average lets one sample of a neon strip
// (5.0) or the ball's highlight (~40) dominate an edge pixel, so edges look jagged again after the
// tone map. Averaging c / (1 + max(c)) and inverting that curve afterwards keeps the edge gradient.

@group(0) @binding(0) var src: texture_multisampled_2d<f32>;

@vertex
fn vs_post(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let p = vec2(select(-1.0, 3.0, index == 1u), select(-1.0, 3.0, index == 2u));
    return vec4(p, 0.0, 1.0);
}

@fragment
fn fs_resolve(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(frag.xy);
    let samples = textureNumSamples(src);
    var sum = vec3(0.0);
    for (var i = 0u; i < samples; i++) {
        let c = textureLoad(src, p, i32(i)).rgb;
        sum += c / (1.0 + max(max(c.r, c.g), c.b));
    }
    let t = sum / f32(samples);
    return vec4(t / max(1.0 - max(max(t.r, t.g), t.b), 1e-4), 1.0);
}
