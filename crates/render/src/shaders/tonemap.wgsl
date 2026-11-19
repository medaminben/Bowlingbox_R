// The tone map (C++ kToneMapGlsl): the basic path's shaders apply it to each fragment, the full
// path's composite to the resolved HDR image.

fn aces_fit(x: vec3<f32>) -> vec3<f32> {  // Narkowicz 2015
    return clamp(x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14), vec3(0.0), vec3(1.0));
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    return mix(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(vec3(0.0031308), c));
}

fn to_display(hdr: vec3<f32>, exposure: f32) -> vec3<f32> {
    return linear_to_srgb(aces_fit(hdr * exposure));
}
