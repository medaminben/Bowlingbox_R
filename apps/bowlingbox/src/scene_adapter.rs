//! The only place that turns lane types into render types.

use bowlingbox_lane::{LANE, PIN_MESH_OFFSET_Y, Phase, Pose, Snapshot};
use bowlingbox_render::{SceneDescription, SceneGeometry, Transform};
use glam::{Quat, Vec3};

/// The lane's sizes, copied for the renderer.
pub fn scene_geometry() -> SceneGeometry {
    SceneGeometry {
        lane_half_width: LANE.half_width,
        lane_length: LANE.length,
        approach: LANE.approach,
        gutter_width: LANE.gutter_width,
        gutter_depth: LANE.gutter_depth,
        ball_radius: LANE.ball_radius,
        pin_height: LANE.pin_height,
        pin_max_radius: LANE.pin_max_radius,
        pin_com_height: LANE.pin_origin_height,
        pin_spacing: LANE.pin_spacing,
        head_pin_z: LANE.head_pin_z,
    }
}

fn to_transform(pose: &Pose) -> Transform {
    let [x, y, z, w] = pose.rotation;
    Transform {
        position: Vec3::from_array(pose.position),
        rotation: Quat::from_xyzw(x, y, z, w),
        ..Transform::default()
    }
}

/// A pin's pose places the collider; the mesh sits `PIN_MESH_OFFSET_Y` along the pin's own up axis.
fn to_pin_transform(pose: &Pose) -> Transform {
    let mut transform = to_transform(pose);
    transform.position += transform.rotation * (Vec3::Y * PIN_MESH_OFFSET_Y);
    transform
}

/// What to draw for a snapshot. The theme is not part of it: it changes the look, not the poses.
pub fn to_scene(snapshot: &Snapshot) -> SceneDescription {
    SceneDescription {
        ball: to_transform(&snapshot.ball),
        pins: snapshot.pins.each_ref().map(to_pin_transform),
        pin_visible: snapshot.pin_in_play,
        show_aim_guide: matches!(snapshot.phase, Phase::Aiming | Phase::Charging),
        aim_offset: snapshot.aim_offset,
        aim_angle_deg: snapshot.aim_angle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bowlingbox_render::{ThemeId, bounds, build_props, next_theme, theme};

    fn snapshot(phase: Phase) -> Snapshot {
        Snapshot {
            phase,
            ball: Pose::default(),
            pins: [Pose::default(); 10],
            pin_standing: [true; 10],
            pin_in_play: [true; 10],
            aim_offset: 0.0,
            aim_angle: 0.0,
            power: 0.0,
            last_pins_down: None,
        }
    }

    #[test]
    fn geometry_copies_every_lane_dimension() {
        let g = scene_geometry();
        assert_eq!(g.lane_half_width, LANE.half_width);
        assert_eq!(g.lane_length, LANE.length);
        assert_eq!(g.approach, LANE.approach);
        assert_eq!(g.gutter_width, LANE.gutter_width);
        assert_eq!(g.gutter_depth, LANE.gutter_depth);
        assert_eq!(g.ball_radius, LANE.ball_radius);
        assert_eq!(g.pin_height, LANE.pin_height);
        assert_eq!(g.pin_max_radius, LANE.pin_max_radius);
        assert_eq!(g.pin_com_height, LANE.pin_origin_height);
        assert_eq!(g.pin_spacing, LANE.pin_spacing);
        assert_eq!(g.head_pin_z, LANE.head_pin_z);
    }

    #[test]
    fn the_aim_guide_shows_only_before_the_throw() {
        for (phase, shown) in [
            (Phase::Aiming, true),
            (Phase::Charging, true),
            (Phase::Rolling, false),
            (Phase::Settling, false),
            (Phase::FrameDone, false),
            (Phase::GameOver, false),
        ] {
            assert_eq!(to_scene(&snapshot(phase)).show_aim_guide, shown, "{phase}");
        }
    }

    #[test]
    fn pins_out_of_play_are_not_visible() {
        let mut s = snapshot(Phase::Settling);
        s.pin_in_play = [
            true, false, true, true, false, true, true, true, true, false,
        ];
        assert_eq!(to_scene(&s).pin_visible, s.pin_in_play);
    }

    #[test]
    fn aim_and_ball_pass_through() {
        let mut s = snapshot(Phase::Aiming);
        s.aim_offset = 0.3;
        s.aim_angle = -2.0;
        s.ball.position = [0.3, 0.1, -1.0];
        s.ball.rotation = [0.0, 0.5, 0.0, 0.866_025_4];
        let scene = to_scene(&s);
        assert_eq!(scene.aim_offset, 0.3);
        assert_eq!(scene.aim_angle_deg, -2.0);
        assert_eq!(scene.ball.position, Vec3::new(0.3, 0.1, -1.0));
        assert_eq!(
            scene.ball.rotation,
            Quat::from_xyzw(0.0, 0.5, 0.0, 0.866_025_4)
        );
    }

    #[test]
    fn a_standing_pin_is_lifted_along_up_and_a_tilted_one_along_its_own_up() {
        let mut s = snapshot(Phase::Settling);
        s.pins[0].position = [0.0, 0.2, 5.0];
        // A quarter turn about z: the pin's up axis points to -x.
        let q = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        s.pins[1].position = [1.0, 0.1, 5.0];
        s.pins[1].rotation = q.to_array();
        let scene = to_scene(&s);
        let up = scene.pins[0].position;
        assert!((up - Vec3::new(0.0, 0.2 + PIN_MESH_OFFSET_Y, 5.0)).length() < 1e-6);
        let tilted = scene.pins[1].position;
        assert!((tilted - Vec3::new(1.0 - PIN_MESH_OFFSET_Y, 0.1, 5.0)).length() < 1e-6);
    }

    #[test]
    fn a_theme_change_leaves_ball_and_pin_transforms_unchanged() {
        let mut s = snapshot(Phase::Rolling);
        s.ball.position = [0.1, 0.11, 4.0];
        s.pins[3].position = [0.2, 0.2, 18.0];
        let before = to_scene(&s);
        let id = next_theme(ThemeId::NeonHall);
        assert_ne!(id, ThemeId::NeonHall);
        assert_eq!(to_scene(&s), before);
    }

    #[test]
    fn decor_clears_the_play_area_of_the_real_lane() {
        let g = scene_geometry();
        let zones = [
            (Vec3::new(-0.82, -0.14, -4.0), Vec3::new(0.82, 0.8, 19.0)),
            (
                Vec3::new(-0.82, -0.14, -4.0),
                Vec3::new(0.82, 2.2, LANE.length),
            ),
            (Vec3::new(-0.6, 1.4, -4.0), Vec3::new(0.6, 2.4, 13.5)),
        ];
        for id in bowlingbox_render::THEMES {
            let th = theme(id);
            for (i, prop) in build_props(th, &g).iter().enumerate() {
                let b = bounds(prop, &g);
                for (min, max) in zones {
                    let overlap = b.min.cmplt(max).all() && b.max.cmpgt(min).all();
                    assert!(!overlap, "theme {} prop {i}", th.name);
                }
            }
        }
    }
}
