//! Triangle mesh generators: box, sphere, cylinder, cone, surface of revolution and bowling pin.
//!
//! Winding is the C++ one: counter-clockwise seen from outside in a right-handed reading. The
//! camera mirrors x (see [`crate::Camera`]), so on screen the front faces are clockwise.

use std::f32::consts::PI;

/// A triangle mesh with one normal per position.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

impl MeshData {
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    fn push(&mut self, p: [f32; 3], n: [f32; 3]) {
        self.positions.push(p);
        self.normals.push(n);
    }

    fn tri(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend([a, b, c]);
    }

    fn next_index(&self) -> u32 {
        u32::try_from(self.positions.len()).expect("mesh has fewer than 2^32 vertices")
    }
}

type Point2 = (f32, f32);

/// Catmull-Rom between `b` (t = 0) and `c` (t = 1), with neighbours `a` and `d`.
fn catmull_rom(a: Point2, b: Point2, c: Point2, d: Point2, t: f32) -> Point2 {
    let f = |p0: f32, p1: f32, p2: f32, p3: f32| {
        0.5 * (2.0 * p1
            + (-p0 + p2) * t
            + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
            + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t * t * t)
    };
    (f(a.0, b.0, c.0, d.0), f(a.1, b.1, c.1, d.1))
}

/// An axis-aligned box centred on the origin with half extents `hx`, `hy`, `hz`.
#[must_use]
pub fn box_data(hx: f32, hy: f32, hz: f32) -> MeshData {
    // Normal, then the two in-plane axes of each face.
    const FACES: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ];
    let h = [hx, hy, hz];
    let mut m = MeshData::default();
    for (n, u, v) in FACES {
        let base = m.next_index();
        for corner in 0..4 {
            let su = if corner == 1 || corner == 2 {
                1.0
            } else {
                -1.0
            };
            let sv = if corner >= 2 { 1.0 } else { -1.0 };
            let p = std::array::from_fn(|k| (n[k] + su * u[k] + sv * v[k]) * h[k]);
            m.push(p, n);
        }
        m.tri(base, base + 1, base + 2);
        m.tri(base, base + 2, base + 3);
    }
    m
}

/// A surface of revolution around y. `profile` holds (radius, y) pairs from bottom to top.
#[must_use]
pub fn lathe_data(profile: &[(f32, f32)], segments: u32) -> MeshData {
    let mut m = MeshData::default();
    let rings = profile.len();
    if rings < 2 || segments < 3 {
        return m;
    }
    for (i, &(radius, y)) in profile.iter().enumerate() {
        let prev = profile[i.saturating_sub(1)];
        let next = profile[(i + 1).min(rings - 1)];
        let dr = next.0 - prev.0;
        let dy = next.1 - prev.1;
        let (mut nr, mut ny) = (dy, -dr);
        let len = nr.hypot(ny);
        if len > 1e-6 {
            nr /= len;
            ny /= len;
        } else {
            nr = 0.0;
            ny = 1.0;
        }
        if radius <= 1e-6 {
            nr = 0.0;
            ny = if i == 0 { -1.0 } else { 1.0 };
        }
        for s in 0..=segments {
            let a = 2.0 * PI * s as f32 / segments as f32;
            let (sn, c) = a.sin_cos();
            m.push([radius * c, y, radius * sn], [nr * c, ny, nr * sn]);
        }
    }
    let stride = segments + 1;
    for i in 0..rings as u32 - 1 {
        for s in 0..segments {
            let a = i * stride + s;
            let (b, c) = (a + 1, a + stride);
            let d = c + 1;
            m.tri(a, c, b);
            m.tri(b, c, d);
        }
    }
    m
}

/// A sphere centred on the origin.
#[must_use]
pub fn sphere_data(radius: f32, segments: u32) -> MeshData {
    let stacks = (segments / 2).max(2);
    let profile: Vec<Point2> = (0..=stacks)
        .map(|i| {
            let t = PI * i as f32 / stacks as f32;
            (radius * t.sin(), -radius * t.cos())
        })
        .collect();
    lathe_data(&profile, segments)
}

/// A cylinder along y, centred on the origin.
#[must_use]
pub fn cylinder_data(radius: f32, height: f32, segments: u32) -> MeshData {
    let h = height / 2.0;
    lathe_data(
        &[
            (0.0, -h),
            (radius, -h),
            (radius, -h),
            (radius, h),
            (radius, h),
            (0.0, h),
        ],
        segments,
    )
}

/// A cone along y, centred on the origin: base circle at the bottom, tip at the top.
#[must_use]
pub fn cone_data(radius: f32, height: f32, segments: u32) -> MeshData {
    let h = height / 2.0;
    lathe_data(&[(radius, -h), (0.0, h)], segments)
}

/// A bowling pin whose origin is `com_height` above the base, to match the physics body. It spans
/// y in `[-com_height, height - com_height]`.
#[must_use]
pub fn pin_data(height: f32, max_radius: f32, com_height: f32, segments: u32) -> MeshData {
    // Silhouette control points (radius / max_radius, y / height), base to top.
    const SHAPE: [Point2; 12] = [
        (0.0, 0.00),
        (0.45, 0.00),
        (0.70, 0.05),
        (0.95, 0.20),
        (1.00, 0.30),
        (0.90, 0.45),
        (0.60, 0.60),
        (0.45, 0.70),
        (0.50, 0.80),
        (0.55, 0.90),
        (0.35, 0.98),
        (0.0, 1.00),
    ];
    const SAMPLES_PER_SPAN: u32 = 4;
    let n = SHAPE.len();
    let at = |i: usize| SHAPE[i.min(n - 1)];
    let place = |(r, y): Point2| (r * max_radius, y * height - com_height);
    let mut profile = Vec::new();
    for i in 0..n - 1 {
        for s in 0..SAMPLES_PER_SPAN {
            let t = s as f32 / SAMPLES_PER_SPAN as f32;
            let (r, y) = catmull_rom(at(i.saturating_sub(1)), at(i), at(i + 1), at(i + 2), t);
            // Clamping keeps the base flat and the belly at exactly max_radius.
            profile.push(place((r.clamp(0.0, 1.0), y.clamp(0.0, 1.0))));
        }
    }
    profile.push(place(SHAPE[n - 1]));
    lathe_data(&profile, segments)
}
