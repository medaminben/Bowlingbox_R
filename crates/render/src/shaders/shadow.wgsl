// The shadow pass's vertex stage (C++ kLitVs with the light's matrices and the empty kDepthFs):
// depth only, from the key light.

@vertex
fn vs_shadow(v: MeshIn) -> @builtin(position) vec4<f32> {
    let clip = frame.light_vp * draw.model * vec4(v.pos, 1.0);
    // C++ turns on GL_DEPTH_CLAMP so that casters between the light and the box still cast.
    // Clamping z here does the same without wgpu's DEPTH_CLIP_CONTROL feature.
    return vec4(clip.xy, clamp(clip.z, 0.0, clip.w), clip.w);
}
