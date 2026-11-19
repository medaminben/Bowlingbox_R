// The translucent aim guide: the lit shader's emissive branch without fog (C++ drawGuide), drawn
// alpha-blended after the opaque geometry and the sky; `fs_guide_hdr` leaves the tone map to the
// composite.

@fragment
fn fs_guide(in: MeshOut) -> @location(0) vec4<f32> {
    return vec4(to_display(draw.color.rgb * draw.band.w, frame.params.z), draw.color.a);
}

@fragment
fn fs_guide_hdr(in: MeshOut) -> @location(0) vec4<f32> {
    return vec4(draw.color.rgb * draw.band.w, draw.color.a);
}
