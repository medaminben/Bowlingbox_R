use bowlingbox_render::{MeshData, box_data, cylinder_data, pin_data, sphere_data};

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
