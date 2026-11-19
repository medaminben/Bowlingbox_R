use bowlingbox_render::{MeshData, box_data, cone_data, cylinder_data, pin_data, sphere_data};

fn expect_well_formed(m: &MeshData) {
    assert_eq!(m.positions.len(), m.normals.len());
    assert_eq!(m.indices.len() % 3, 0);
    let n = u32::try_from(m.vertex_count()).unwrap();
    for &idx in &m.indices {
        assert!(idx < n);
    }
    for v in &m.normals {
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-4, "normal length {len}");
    }
}

fn min_y(m: &MeshData) -> f32 {
    m.positions.iter().fold(1e9, |y, p| y.min(p[1]))
}

fn max_y(m: &MeshData) -> f32 {
    m.positions.iter().fold(-1e9, |y, p| y.max(p[1]))
}

#[test]
fn box_has_24_vertices_and_12_triangles() {
    let m = box_data(1.0, 2.0, 3.0);
    assert_eq!(m.vertex_count(), 24);
    assert_eq!(m.indices.len(), 36);
    expect_well_formed(&m);
    assert_eq!(min_y(&m), -2.0);
    assert_eq!(max_y(&m), 2.0);
}

#[test]
fn sphere_is_well_formed_and_bounded() {
    let m = sphere_data(0.5, 16);
    expect_well_formed(&m);
    assert!((min_y(&m) + 0.5).abs() < 1e-5);
    assert!((max_y(&m) - 0.5).abs() < 1e-5);
}

#[test]
fn cylinder_is_centred_on_origin() {
    let m = cylinder_data(0.3, 1.0, 12);
    expect_well_formed(&m);
    assert!((min_y(&m) + 0.5).abs() < 1e-5);
    assert!((max_y(&m) - 0.5).abs() < 1e-5);
}

#[test]
fn pin_spans_from_base_to_top_relative_to_com() {
    let m = pin_data(0.38, 0.06, 0.12, 40);
    expect_well_formed(&m);
    assert!((min_y(&m) + 0.12).abs() < 1e-5);
    assert!((max_y(&m) - 0.26).abs() < 1e-5);
}

#[test]
fn pin_profile_is_smooth() {
    let segments = 40_usize;
    let (height, com) = (0.38_f32, 0.12_f32);
    let m = pin_data(height, 0.06, com, 40);
    let stride = segments + 1;
    let rings = m.vertex_count() / stride;
    assert!(rings >= 40, "{rings} rings");
    let max_step = 25.0_f32.to_radians(); // the worst step is about 19 degrees
    let mut prev: Option<f32> = None;
    for i in 0..rings {
        // Segment 0: x = radius, z = 0.
        let [r, y, _] = m.positions[i * stride];
        let [_, ny, _] = m.normals[i * stride];
        let nr = m.normals[i * stride][0];
        if r < 1e-6 || y < -com + 0.01 * height {
            // poles and the flat base
            prev = None;
            continue;
        }
        let angle = ny.atan2(nr); // profile normal (n_r, n_y)
        if let Some(p) = prev {
            assert!((angle - p).abs() < max_step, "ring {i} at y = {y}");
        }
        prev = Some(angle);
    }
}

/// The renderer culls back faces, so every triangle must wind counter-clockwise seen from outside
/// (its right-handed face normal points the way its vertex normals do).
fn expect_outward_winding(m: &MeshData) {
    let p = |i: u32| glam::Vec3::from_array(m.positions[i as usize]);
    let n = |i: u32| glam::Vec3::from_array(m.normals[i as usize]);
    for t in m.indices.as_chunks::<3>().0 {
        let face = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
        if face.length() < 1e-9 {
            continue;
        }
        let normal = n(t[0]) + n(t[1]) + n(t[2]);
        assert!(face.dot(normal) > 0.0, "triangle {t:?} winds inward");
    }
}

#[test]
fn every_mesh_winds_counter_clockwise_from_outside() {
    expect_outward_winding(&box_data(0.5, 0.5, 0.5));
    expect_outward_winding(&sphere_data(0.109, 48));
    expect_outward_winding(&cylinder_data(0.5, 1.0, 24));
    expect_outward_winding(&cone_data(0.5, 1.0, 24));
    expect_outward_winding(&pin_data(0.38, 0.06, 0.12, 40));
}
