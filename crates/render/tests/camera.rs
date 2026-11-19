use bowlingbox_render::{Camera, box_data};
use glam::{Vec3, Vec4};

fn ndc(camera: &Camera, p: Vec3) -> Vec3 {
    let c = camera.projection() * camera.view() * p.extend(1.0);
    c.truncate() / c.w
}

#[test]
fn a_point_at_plus_x_projects_to_the_right_half() {
    let camera = Camera::default();
    let p = ndc(&camera, Vec3::new(0.5, 1.0, 6.0));
    assert!(p.x > 0.0, "ndc x {}", p.x);
    let q = ndc(&camera, Vec3::new(-0.5, 1.0, 6.0));
    assert!(q.x < 0.0, "ndc x {}", q.x);
    assert!((p.x + q.x).abs() < 1e-5);
}

#[test]
fn a_point_above_the_target_projects_to_the_upper_half() {
    let camera = Camera::default();
    assert!(ndc(&camera, Vec3::new(0.0, 2.5, 6.0)).y > 0.0);
    assert!(ndc(&camera, Vec3::new(0.0, -0.5, 6.0)).y < 0.0);
}

#[test]
fn points_in_front_land_inside_the_zero_to_one_depth_range() {
    let camera = Camera::default();
    for z in [0.0, 6.0, 20.0, 100.0] {
        let p = ndc(&camera, Vec3::new(0.0, 1.0, z));
        assert!((0.0..=1.0).contains(&p.z), "z {z} -> depth {}", p.z);
    }
    let dir = (camera.target - camera.eye).normalize();
    let near = ndc(&camera, camera.eye + dir * 0.1);
    let far = ndc(&camera, camera.eye + dir * 150.0);
    assert!(near.z.abs() < 1e-3);
    assert!((far.z - 1.0).abs() < 1e-3);
}

#[test]
fn the_default_view_looks_from_the_c_plus_plus_eye_to_the_target() {
    let camera = Camera::default();
    assert_eq!(camera.eye, Vec3::new(0.0, 1.8, -3.5));
    assert_eq!(camera.target, Vec3::new(0.0, 0.0, 6.0));
    // The eye is the view-space origin.
    let e = camera.view() * Vec4::new(0.0, 1.8, -3.5, 1.0);
    assert!(e.truncate().length() < 1e-5);
}

#[test]
fn set_viewport_changes_the_aspect_and_guards_zero() {
    let mut camera = Camera::default();
    camera.set_viewport(800, 400);
    assert!((camera.aspect - 2.0).abs() < 1e-6);
    camera.set_viewport(0, 0);
    assert!((camera.aspect - 1.0).abs() < 1e-6);
}

#[test]
fn look_at_moves_the_eye_and_target() {
    let mut camera = Camera::default();
    camera.look_at(Vec3::new(1.0, 2.0, 3.0), Vec3::new(1.0, 0.0, 9.0));
    assert_eq!(camera.eye, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(camera.target, Vec3::new(1.0, 0.0, 9.0));
}

/// The meshes keep the C++ winding (counter-clockwise outside in a right-handed reading). The
/// camera mirrors the picture, so a face turned towards the camera appears clockwise on screen.
#[test]
fn faces_turned_to_the_camera_wind_clockwise_on_screen() {
    let camera = Camera::default();
    let mesh = box_data(0.5, 0.5, 0.5);
    let mut checked = 0;
    for tri in mesh.indices.chunks(3) {
        let n = Vec3::from(mesh.normals[tri[0] as usize]);
        if n != Vec3::new(0.0, 0.0, -1.0) {
            continue;
        }
        let at = |i: u32| {
            let p = Vec3::from(mesh.positions[i as usize]) + Vec3::new(0.0, 1.0, 5.0);
            ndc(&camera, p)
        };
        let (a, b, c) = (at(tri[0]), at(tri[1]), at(tri[2]));
        let area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
        assert!(area < 0.0, "signed area {area}");
        checked += 1;
    }
    assert_eq!(checked, 2);
}
