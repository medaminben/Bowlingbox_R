// The full path's main pass stores each HDR sample weighted, c / (1 + max(c)), so a plain average
// of the samples, which the hardware MSAA resolve takes, is the C++ weighted resolve (kResolveFs):
// one very bright sample (a neon strip at 5, the ball's highlight at ~40) does not dominate an
// edge pixel, and edges keep their gradient after the tone map. Whatever reads the resolved scene
// undoes the weight per texel.

fn resolve_weight(c: vec3<f32>) -> vec3<f32> {
    return c / (1.0 + max(max(c.r, c.g), c.b));
}

fn resolve_unweight(t: vec3<f32>) -> vec3<f32> {
    return t / max(1.0 - max(max(t.r, t.g), t.b), 1e-4);
}
