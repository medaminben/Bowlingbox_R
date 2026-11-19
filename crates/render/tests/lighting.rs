use bowlingbox_render::{
    Aabb, HALL_HEIGHT, LightFrustum, PIT_LENGTH, PIT_TOP_Y, Pattern, RETURN_HEIGHT, SHADOW_TOP_Y,
    SceneGeometry, THEMES, ThemeId, bounds, build_props, fit_light_frustum, lane_pitch,
    linear_to_srgb, linear_to_srgb_vec3, props_in_box, reflection_box, shadow_box, srgb_to_linear,
    srgb_to_linear_vec3, theme,
};
use glam::Vec3;

/// The C++ `Alley::kLane`, copied: the render crate does not depend on the lane crate.
const G: SceneGeometry = SceneGeometry {
    lane_half_width: 0.525,
    lane_length: 18.3,
    approach: 2.0,
    gutter_width: 0.24,
    gutter_depth: 0.05,
    ball_radius: 0.109,
    pin_height: 0.38,
    pin_max_radius: 0.06,
    pin_com_height: 0.12,
    pin_spacing: 0.3048,
    head_pin_z: 17.4,
};

fn to_clip(f: &LightFrustum, p: Vec3) -> Vec3 {
    let c = f.view_projection() * p.extend(1.0);
    c.truncate() / c.w
}

fn corners(b: &Aabb) -> [Vec3; 8] {
    std::array::from_fn(|i| {
        Vec3::new(
            if i & 1 != 0 { b.max.x } else { b.min.x },
            if i & 2 != 0 { b.max.y } else { b.min.y },
            if i & 4 != 0 { b.max.z } else { b.min.z },
        )
    })
}

#[test]
fn srgb_round_trips() {
    for i in 0..=100_i16 {
        let c = f32::from(i) / 100.0;
        assert!((linear_to_srgb(srgb_to_linear(c)) - c).abs() < 1e-5, "{c}");
    }
}

#[test]
fn srgb_matches_known_values() {
    assert_eq!(srgb_to_linear(0.0), 0.0);
    assert!((srgb_to_linear(1.0) - 1.0).abs() < 1e-6);
    assert!((srgb_to_linear(0.5) - 0.21404).abs() < 1e-4);
    let v = srgb_to_linear_vec3(Vec3::new(0.0, 0.5, 1.0));
    assert!((v.y - 0.21404).abs() < 1e-4);
    assert!((linear_to_srgb_vec3(Vec3::splat(0.21404)).z - 0.5).abs() < 1e-4);
}

#[test]
fn shadow_box_covers_the_lane_deck_and_pit() {
    let b = shadow_box(&G);
    assert!(b.min.z <= -G.approach);
    assert!(b.max.z >= G.lane_length + PIT_LENGTH);
    assert!(b.max.x >= G.lane_half_width + G.gutter_width);
    assert!(b.min.x <= -(G.lane_half_width + G.gutter_width));
    assert!(b.min.y <= PIT_TOP_Y);
    assert_eq!(b.max.y, SHADOW_TOP_Y);
}

#[test]
fn light_frustum_holds_the_shadow_box_for_every_theme() {
    let b = shadow_box(&G);
    for id in THEMES {
        let f = fit_light_frustum(theme(id).light_dir, &b);
        let (mut max_x, mut max_y) = (0.0_f32, 0.0_f32);
        for c in corners(&b) {
            let p = to_clip(&f, c);
            // wgpu depth runs 0..1; x and y run -1..1.
            assert!(
                p.x >= -1.0 - 1e-4 && p.x <= 1.0 + 1e-4,
                "{}",
                theme(id).name
            );
            assert!(
                p.y >= -1.0 - 1e-4 && p.y <= 1.0 + 1e-4,
                "{}",
                theme(id).name
            );
            assert!(p.z >= -1e-4 && p.z <= 1.0 + 1e-4, "{}", theme(id).name);
            max_x = max_x.max(p.x.abs());
            max_y = max_y.max(p.y.abs());
        }
        assert!(
            (max_x - 1.0).abs() < 1e-4,
            "fit is tight in x: {}",
            theme(id).name
        );
        assert!(
            (max_y - 1.0).abs() < 1e-4,
            "fit is tight in y: {}",
            theme(id).name
        );
    }
}

#[test]
fn light_frustum_depth_grows_along_the_light() {
    let b = shadow_box(&G);
    let dir = theme(ThemeId::NeonHall).light_dir;
    let f = fit_light_frustum(dir, &b);
    let c = (b.min + b.max) * 0.5;
    assert!(to_clip(&f, c).z < to_clip(&f, c + dir.normalize() * 0.1).z);
}

#[test]
fn the_lane_runs_along_the_light_maps_long_axis() {
    let b = shadow_box(&G);
    for id in THEMES {
        let f = fit_light_frustum(theme(id).light_dir, &b);
        let foul = to_clip(&f, Vec3::ZERO);
        let down = to_clip(&f, Vec3::new(0.0, 0.0, 10.0));
        assert!(
            (foul.x - down.x).abs() < 1e-4,
            "one map column: {}",
            theme(id).name
        );
        assert!((down.y - foul.y).abs() > 0.1, "{}", theme(id).name);
    }
}

#[test]
fn straight_down_light_gives_a_finite_frustum() {
    let m = fit_light_frustum(Vec3::new(0.0, -1.0, 0.0), &shadow_box(&G)).view_projection();
    for v in m.to_cols_array() {
        assert!(v.is_finite());
    }
}

#[test]
fn shadow_casters_skip_the_ceiling_and_keep_the_ball_returns() {
    for id in THEMES {
        let t = theme(id);
        let props = build_props(t, &G);
        let casters = props_in_box(&props, &G, &shadow_box(&G));
        assert!(!casters.is_empty(), "{}", t.name);
        let mut above = 0;
        for (i, p) in props.iter().enumerate() {
            if bounds(p, &G).min.y < SHADOW_TOP_Y {
                continue;
            }
            above += 1;
            assert!(
                !casters.contains(&i),
                "{} prop {i} is above the box",
                t.name
            );
        }
        if t.indoor {
            assert!(
                above > 0,
                "{}: the ceiling lies above the shadow box",
                t.name
            );
        }
        let has_return = casters.iter().any(|&i| {
            let b = bounds(&props[i], &G);
            (b.max.y - RETURN_HEIGHT).abs() < 1e-4
                && (f32::midpoint(b.min.x, b.max.x) - lane_pitch(&G) / 2.0).abs() < 1e-3
        });
        assert!(
            has_return,
            "{}: the ball return beside the lane casts",
            t.name
        );
        if t.indoor {
            let ceiling_casts = casters
                .iter()
                .any(|&i| bounds(&props[i], &G).min.y >= HALL_HEIGHT - 1e-3);
            assert!(!ceiling_casts, "{}", t.name);
        }
    }
}

#[test]
fn reflection_casters_skip_the_floor_and_the_neighbour_lanes() {
    for id in THEMES {
        let t = theme(id);
        let props = build_props(t, &G);
        let casters = props_in_box(&props, &G, &reflection_box(&G));
        let (mut below, mut neighbours) = (0, 0);
        for (i, p) in props.iter().enumerate() {
            if bounds(p, &G).max.y <= 0.0 {
                below += 1;
                assert!(
                    !casters.contains(&i),
                    "{} prop {i} is below the lane",
                    t.name
                );
            }
            if p.material.pattern == Pattern::Wood {
                neighbours += 1;
                assert!(
                    !casters.contains(&i),
                    "{} prop {i} is a neighbour lane",
                    t.name
                );
            }
        }
        assert!(below > 0, "{}: nothing to exclude", t.name);
        assert!(neighbours > 0, "{}: nothing to exclude", t.name);
        let has_return = casters
            .iter()
            .any(|&i| (bounds(&props[i], &G).max.y - RETURN_HEIGHT).abs() < 1e-4);
        assert!(has_return, "{}", t.name);
    }
}
