use bowlingbox_render::SceneDescription;
use glam::Vec3;

#[test]
fn a_default_scene_hides_every_pin_and_the_aim_guide() {
    let s = SceneDescription::default();
    assert_eq!(s.pins.len(), 10);
    assert!(s.pin_visible.iter().all(|v| !v));
    assert!(!s.show_aim_guide);
    assert_eq!(s.aim_offset, 0.0);
    assert_eq!(s.aim_angle_deg, 0.0);
    assert_eq!(s.ball.position, Vec3::ZERO);
    assert_eq!(s.ball.scale, Vec3::ONE);
}
