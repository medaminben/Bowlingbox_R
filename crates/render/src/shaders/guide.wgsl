// The translucent aim guide: the lit shader's emissive branch without fog (C++ drawGuide), drawn
// alpha-blended after the opaque geometry and the sky.

@fragment
fn fs_guide(in: MeshOut) -> @location(0) vec4<f32> {
    return vec4(to_display(draw.color.rgb * draw.band.w, frame.params.z), draw.color.a);
}
