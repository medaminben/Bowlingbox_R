// The C++ lit shader (Shaders.cpp kLitBody) without reflection sampling: GGX key light,
// hemisphere ambient, up to four point lights, distance fog, emissive materials, procedural wood
// and ball patterns. `fs_lit` (basic path) tone maps to sRGB here; `fs_lit_hdr` (full path) adds
// the key light's shadow and writes linear HDR for the composite to tone map.

const PI: f32 = 3.14159265;

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

fn hash3(p: vec3<f32>) -> f32 {
    return fract(sin(dot(p, vec3(127.1, 311.7, 74.7))) * 43758.5453);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
               mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

fn value_noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(hash3(i), hash3(i + vec3(1.0, 0.0, 0.0)), u.x);
    let b = mix(hash3(i + vec3(0.0, 1.0, 0.0)), hash3(i + vec3(1.0, 1.0, 0.0)), u.x);
    let c = mix(hash3(i + vec3(0.0, 0.0, 1.0)), hash3(i + vec3(1.0, 0.0, 1.0)), u.x);
    let d = mix(hash3(i + vec3(0.0, 1.0, 1.0)), hash3(i + vec3(1.0, 1.0, 1.0)), u.x);
    return mix(mix(a, b, u.y), mix(c, d, u.y), u.z);
}

fn fbm3(start: vec3<f32>) -> f32 {
    var p = start;
    var sum = 0.0;
    var amp = 0.5;
    for (var i = 0; i < 4; i++) {
        sum += amp * value_noise3(p);
        p *= 2.03;
        amp *= 0.5;
    }
    return sum;
}

// Dark-red inlay on the main lane: 7 arrows about 4.6 m down and 7 dots on the approach,
// on boards 5, 10, ... 35 of 39. Board 20 is the middle board, so the pattern is symmetric.
fn marking(p: vec3<f32>) -> f32 {
    let half_width = frame.lane.x;
    let board_width = frame.lane.y;
    if abs(p.x) > half_width {
        return 0.0;
    }
    let from_edge = (p.x + half_width) / board_width;
    let board = floor(from_edge) + 1.0;                        // 1 .. 39
    if board < 5.0 || board > 35.0 || board % 5.0 != 0.0 {
        return 0.0;
    }
    let u = fract(from_edge) - 0.5;                            // -0.5 .. 0.5 across the board
    let tip = 4.3 + 0.6 * (1.0 - abs(board - 20.0) / 15.0);    // the middle arrow is furthest
    let along = (tip - p.z) / 0.15;                            // 0 at the tip, 1 at the base
    let arrow = select(0.0, 1.0, along >= 0.0 && along <= 1.0 && abs(u) <= 0.5 * along);
    let spot = select(0.0, 1.0, length(vec2(u * board_width, p.z + 0.15)) < 0.008);
    return max(arrow, spot);
}

// Boards are counted from the main lane's edge, so its 39 boards line up with both edges.
// `dpx` is fwidth(p.x), taken in uniform control flow.
fn wood(base: vec3<f32>, p: vec3<f32>, dpx: f32) -> vec3<f32> {
    let half_width = frame.lane.x;
    let board_width = frame.lane.y;
    let from_edge = (p.x + half_width) / board_width;
    let board = floor(from_edge);
    let tint = 1.0 + 0.16 * (hash(vec2(board, 7.0)) - 0.5);
    let phase = hash(vec2(board, 13.0)) * 50.0;
    let fine = clamp(1.0 - dpx * 750.0, 0.0, 1.0);  // fade grain finer than a pixel
    let grain = 0.6 * noise(vec2(p.x * 400.0, p.z * 3.0 + phase))
              + 0.4 * fine * noise(vec2(p.x * 1500.0, p.z * 9.0 + phase));
    let streak = smoothstep(0.75, 0.95, noise(vec2(board * 3.1, p.z * 0.4 + phase)));
    var c = base * tint * (0.92 + 0.12 * grain) * (1.0 - 0.18 * streak);
    let seam = smoothstep(0.92, 1.0, abs(fract(from_edge) - 0.5) * 2.0);
    c *= 1.0 - 0.35 * seam * clamp(1.0 - dpx / board_width, 0.0, 1.0);
    if p.z < 0.0 {
        c *= vec3(1.04, 1.02, 0.97);           // approach
    } else if p.z > frame.lane.z {
        c *= vec3(0.96, 0.94, 0.92);           // pin deck
    }
    return mix(c, vec3(0.30, 0.02, 0.02), marking(p));
}

fn finger_hole(c: vec3<f32>, d: vec3<f32>, centre: vec3<f32>, r: f32) -> vec3<f32> {
    let a = acos(clamp(dot(d, normalize(centre)), -1.0, 1.0));  // radians from the hole centre
    return c * mix(0.08, 1.0, smoothstep(r - 0.02, r + 0.01, a));
}

// Two-tone marbling in object space, so it rolls with the ball, plus three finger holes.
fn swirl(base: vec3<f32>, p: vec3<f32>) -> vec3<f32> {
    let d = normalize(p);
    let warp = fbm3(d * 2.5 + vec3(3.1));
    let m = fbm3(d * 3.0 + 2.0 * vec3(warp));
    var c = mix(base, base * 0.35, smoothstep(0.45, 0.55, m));
    c = finger_hole(c, d, vec3(0.0, -0.15, 1.0), 0.11);
    c = finger_hole(c, d, vec3(-0.06, 0.12, 1.0), 0.09);
    c = finger_hole(c, d, vec3(0.06, 0.12, 1.0), 0.09);
    return c;
}

fn distribution_ggx(n_dot_h: f32, a: f32) -> f32 {
    let a2 = a * a;
    let d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

fn visibility_smith(n_dot_v: f32, n_dot_l: f32, a: f32) -> f32 {  // includes 1 / (4 nDotV nDotL)
    let k = a * 0.5;
    return 0.25 / ((n_dot_v * (1.0 - k) + k) * (n_dot_l * (1.0 - k) + k));
}

fn brdf(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, albedo: vec3<f32>, radiance: vec3<f32>) -> vec3<f32> {
    let n_dot_l = max(dot(n, l), 0.0);
    if n_dot_l <= 0.0 {
        return vec3(0.0);
    }
    let roughness = draw.surface.x;
    let specular = vec3(draw.surface.y);
    let h = normalize(v + l);
    let n_dot_v = max(dot(n, v), 1e-4);
    let n_dot_h = max(dot(n, h), 0.0);
    let v_dot_h = max(dot(v, h), 0.0);
    let a = max(roughness * roughness, 0.002);
    let f = specular + (1.0 - specular) * pow(1.0 - v_dot_h, 5.0);
    let spec = distribution_ggx(n_dot_h, a) * visibility_smith(n_dot_v, n_dot_l, a) * f;
    let diff = (1.0 - f) * albedo / PI;
    return (diff + spec) * radiance * n_dot_l;
}

// Full path only: the basic path's layout has no group 2.
@group(2) @binding(0) var shadow_map: texture_depth_2d;
@group(2) @binding(1) var shadow_sampler: sampler_comparison;  // less-equal, linear

// How much of the key light reaches `world_pos`, 0..1 (C++ keyLightVisible).
fn key_light_visible(n: vec3<f32>, world_pos: vec3<f32>) -> f32 {
    if frame.shadow.x < 0.5 {
        return 1.0;
    }
    let clip = frame.light_vp * vec4(world_pos, 1.0);
    let ndc = clip.xyz / clip.w;
    // Texture v runs down while NDC y runs up; depth is already 0..1.
    let p = vec3(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5, ndc.z);
    if any(p < vec3(0.0)) || any(p > vec3(1.0)) {
        return 1.0;  // outside: lit
    }
    let bias = max(0.0025 * (1.0 - max(dot(n, -frame.light_dir.xyz), 0.0)), 0.0005);
    // Each tap is a bilinear 2x2 compare (linear filtering), so four taps at +-0.5 texel cover
    // the same 3x3-texel tent footprint as nine point taps. The Level variant needs no
    // derivatives, so it may run outside uniform control flow.
    let h = 0.5 / vec2<f32>(textureDimensions(shadow_map));
    let z = p.z - bias;
    return 0.25 * (textureSampleCompareLevel(shadow_map, shadow_sampler, p.xy + vec2(-h.x, -h.y), z)
                 + textureSampleCompareLevel(shadow_map, shadow_sampler, p.xy + vec2(h.x, -h.y), z)
                 + textureSampleCompareLevel(shadow_map, shadow_sampler, p.xy + vec2(-h.x, h.y), z)
                 + textureSampleCompareLevel(shadow_map, shadow_sampler, p.xy + vec2(h.x, h.y), z));
}

// Linear HDR colour, before the tone map. `key_visible` scales the key light (1 without shadows).
fn shade(in: MeshOut, key_visible: f32) -> vec3<f32> {
    let dpx = fwidth(in.world_pos.x);
    let in_band = in.local_pos.y >= draw.band.x && in.local_pos.y <= draw.band.y;
    var base = select(draw.color.rgb, frame.band_color.rgb, in_band);
    let pattern = i32(draw.surface.z + 0.5);
    if pattern == 1 {
        base = wood(base, in.world_pos, dpx);
    } else if pattern == 2 {
        base = swirl(base, in.local_pos);
    }

    let emissive = draw.band.z > 0.5;
    var rgb: vec3<f32>;
    if emissive {
        rgb = base * draw.band.w;
    } else {
        let roughness = draw.surface.x;
        let specular = draw.surface.y;
        let n = normalize(in.normal);
        let v = normalize(frame.camera_pos.xyz - in.world_pos);
        rgb = mix(frame.ground_ambient.rgb, frame.sky_ambient.rgb, n.y * 0.5 + 0.5) * base;
        rgb += brdf(n, v, -frame.light_dir.xyz, base, frame.key_color.rgb) * key_visible;
        let count = i32(frame.params.w + 0.5);
        for (var i = 0; i < count; i++) {
            let to_light = frame.point_pos[i].xyz - in.world_pos;
            let d = length(to_light);
            let range = frame.point_pos[i].w;
            if d >= range {
                continue;  // the window below is zero: skip the BRDF
            }
            let window = clamp(1.0 - pow(d / range, 4.0), 0.0, 1.0);
            let radiance = frame.point_color[i].rgb * window * window / (d * d + 1.0);
            let l = to_light / d;
            // Rough surfaces show no visible point-light highlight: Lambert is enough there.
            if roughness >= 0.6 {
                rgb += (1.0 - specular) * base / PI * radiance * max(dot(n, l), 0.0);
            } else {
                rgb += brdf(n, v, l, base, radiance);
            }
        }
    }
    if draw.surface.w > 0.5 {
        let f = clamp((length(in.view_pos) - frame.params.x) / (frame.params.y - frame.params.x), 0.0, 1.0);
        rgb = mix(rgb, frame.fog_color.rgb, select(f, 0.5 * f, emissive));
    }
    return rgb;
}

@fragment
fn fs_lit(in: MeshOut) -> @location(0) vec4<f32> {
    return vec4(to_display(shade(in, 1.0), frame.params.z), draw.color.a);
}

@fragment
fn fs_lit_hdr(in: MeshOut) -> @location(0) vec4<f32> {
    var key_visible = 1.0;
    if draw.band.z <= 0.5 {  // emissive surfaces ignore the key light
        key_visible = key_light_visible(normalize(in.normal), in.world_pos);
    }
    return vec4(shade(in, key_visible), draw.color.a);
}
