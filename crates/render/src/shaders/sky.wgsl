// One triangle that covers the viewport on the far plane, drawn after the opaque geometry with a
// less-or-equal depth test, so it fills only the uncovered pixels (C++ kSkyVs and kSkyBody).

struct SkyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) t: f32,  // 0 at the bottom, 1 at the top
}

@vertex
fn vs_sky(@builtin(vertex_index) index: u32) -> SkyOut {
    let p = vec2(select(-1.0, 3.0, index == 1u), select(-1.0, 3.0, index == 2u));
    var out: SkyOut;
    out.t = (p.y + 1.0) * 0.5;
    out.clip = vec4(p, 1.0, 1.0);
    return out;
}

fn sky(in: SkyOut) -> vec3<f32> {
    return mix(frame.sky_bottom.rgb, frame.sky_top.rgb, clamp(in.t, 0.0, 1.0));
}

@fragment
fn fs_sky(in: SkyOut) -> @location(0) vec4<f32> {
    return vec4(to_display(sky(in), frame.params.z), 1.0);
}

@fragment
fn fs_sky_hdr(in: SkyOut) -> @location(0) vec4<f32> {
    return vec4(resolve_weight(sky(in)), 1.0);
}
