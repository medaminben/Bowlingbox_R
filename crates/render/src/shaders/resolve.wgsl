// The full path's MSAA resolve where the HDR format has no hardware resolve: the plain average of
// the samples, which the main pass stored weighted (weight.wgsl), as the hardware resolve takes it.

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
        sum += textureLoad(src, p, i32(i)).rgb;
    }
    return vec4(sum / f32(samples), 1.0);
}
