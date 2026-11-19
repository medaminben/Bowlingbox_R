use bowlingbox_render::{
    Aabb, HALL_BACK_Z, HALL_HEIGHT, MAX_POINT_LIGHTS, Material, MeshData, Pattern, Prop,
    RETURN_WIDTH, SceneGeometry, Shape, THEMES, Theme, ThemeId, Transform, bounds, build_props,
    hall_half_width, lane_pitch, local_bounds, next_theme, shape_mesh, theme, theme_from_name,
};
use glam::{Mat4, Quat, Vec3};

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

// Volumes that must stay free of decor (spec "Coordinates").
const PLAY_CORRIDOR: Aabb = Aabb {
    min: Vec3::new(-0.82, -0.14, -4.0),
    max: Vec3::new(0.82, 0.8, 19.0),
};
const SIGHT_CORRIDOR: Aabb = Aabb {
    min: Vec3::new(-0.82, -0.14, -4.0),
    max: Vec3::new(0.82, 2.2, 18.3),
};
const CAMERA_BOX: Aabb = Aabb {
    min: Vec3::new(-0.6, 1.4, -4.0),
    max: Vec3::new(0.6, 2.4, 13.5),
};

fn overlaps(a: &Aabb, b: &Aabb) -> bool {
    a.min.x < b.max.x
        && a.max.x > b.min.x
        && a.min.y < b.max.y
        && a.max.y > b.min.y
        && a.min.z < b.max.z
        && a.max.z > b.min.z
}

fn mesh_extent(m: &MeshData) -> Aabb {
    let mut b = Aabb {
        min: Vec3::splat(f32::MAX),
        max: Vec3::splat(f32::MIN),
    };
    for p in &m.positions {
        b.min = b.min.min(Vec3::from(*p));
        b.max = b.max.max(Vec3::from(*p));
    }
    b
}

fn expect_near(actual: Vec3, expected: Vec3, what: &str) {
    assert!(
        (actual - expected).abs().max_element() < 1e-4,
        "{what}: {actual} vs {expected}"
    );
}

fn lanes_per_side(t: &Theme) -> u8 {
    u8::try_from(t.neighbour_lanes_per_side).unwrap()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn count(props: &[Prop], pred: impl Fn(&Prop) -> bool) -> usize {
    props.iter().filter(|p| pred(p)).count()
}

#[test]
fn every_theme_round_trips_its_id_and_name() {
    for id in THEMES {
        assert_eq!(theme(id).id, id);
        assert_eq!(theme_from_name(theme(id).name), Some(id));
    }
    assert_eq!(theme_from_name("neon"), Some(ThemeId::NeonHall));
    assert_eq!(theme_from_name("classic"), Some(ThemeId::ClassicAlley));
    assert_eq!(theme_from_name("outdoor"), Some(ThemeId::Outdoor));
}

#[test]
fn theme_names_ignore_case_and_reject_unknown() {
    assert_eq!(theme_from_name("Classic"), Some(ThemeId::ClassicAlley));
    assert_eq!(theme_from_name("NEON"), Some(ThemeId::NeonHall));
    assert_eq!(theme_from_name("x"), None);
    assert_eq!(theme_from_name(""), None);
    assert_eq!(theme_from_name("neonx"), None);
}

#[test]
fn theme_labels_are_the_menu_names() {
    assert_eq!(theme(ThemeId::NeonHall).label, "Neon Hall");
    assert_eq!(theme(ThemeId::ClassicAlley).label, "Classic Alley");
    assert_eq!(theme(ThemeId::Outdoor).label, "Outdoor");
}

#[test]
fn next_theme_visits_all_three_and_wraps() {
    assert_eq!(next_theme(ThemeId::NeonHall), ThemeId::ClassicAlley);
    assert_eq!(next_theme(ThemeId::ClassicAlley), ThemeId::Outdoor);
    assert_eq!(next_theme(ThemeId::Outdoor), ThemeId::NeonHall);
}

#[test]
fn fog_and_lighting_are_in_range() {
    for id in THEMES {
        let t = theme(id);
        assert!(t.fog_start < t.fog_end, "{}", t.name);
        assert!((0..=3).contains(&t.neighbour_lanes_per_side), "{}", t.name);
        assert!(t.key_intensity > 0.0, "{}", t.name);
        for k in 0..3 {
            assert!((0.0..=1.0).contains(&t.sky_ambient[k]), "{}", t.name);
            assert!((0.0..=1.0).contains(&t.ground_ambient[k]), "{}", t.name);
        }
        assert!(t.point_light_count <= MAX_POINT_LIGHTS, "{}", t.name);
        for (i, l) in t.point_lights[..t.point_light_count].iter().enumerate() {
            assert!(l.intensity > 0.0, "{} light {i}", t.name);
            assert!(l.range > 0.0, "{} light {i}", t.name);
        }
        assert!(t.exposure > 0.0, "{}", t.name);
        assert!((0.0..=1.0).contains(&t.bloom_strength), "{}", t.name);
    }
}

#[test]
fn local_bounds_match_the_drawn_meshes() {
    for shape in [Shape::Box, Shape::Cylinder, Shape::Cone, Shape::Pin] {
        let mesh = mesh_extent(&shape_mesh(shape, &G));
        let local = local_bounds(shape, &G);
        expect_near(local.min, mesh.min, "min");
        expect_near(local.max, mesh.max, "max");
    }
}

#[test]
fn bounds_of_a_turned_box_are_its_corner_extent() {
    let p = Prop {
        shape: Shape::Box,
        transform: Transform {
            position: Vec3::new(1.0, 2.0, 3.0),
            rotation: Quat::from_axis_angle(Vec3::Y, 30.0_f32.to_radians()),
            scale: Vec3::new(2.0, 1.0, 0.5),
        },
        ..Prop::default()
    };
    let m: Mat4 = p.transform.matrix();
    let mut corners = Aabb {
        min: Vec3::splat(f32::MAX),
        max: Vec3::splat(f32::MIN),
    };
    for i in 0..8 {
        let c = Vec3::new(
            if i & 1 != 0 { 0.5 } else { -0.5 },
            if i & 2 != 0 { 0.5 } else { -0.5 },
            if i & 4 != 0 { 0.5 } else { -0.5 },
        );
        let w = m.transform_point3(c);
        corners.min = corners.min.min(w);
        corners.max = corners.max.max(w);
    }
    let b = bounds(&p, &G);
    expect_near(b.min, corners.min, "min");
    expect_near(b.max, corners.max, "max");
}

#[test]
fn bounds_of_a_pin_at_the_origin() {
    let p = Prop {
        shape: Shape::Pin,
        ..Prop::default()
    };
    let b = bounds(&p, &G);
    expect_near(
        b.min,
        Vec3::new(-G.pin_max_radius, -G.pin_com_height, -G.pin_max_radius),
        "min",
    );
    expect_near(
        b.max,
        Vec3::new(
            G.pin_max_radius,
            G.pin_height - G.pin_com_height,
            G.pin_max_radius,
        ),
        "max",
    );
}

#[test]
fn transform_matrix_is_translate_rotate_scale() {
    let t = Transform {
        position: Vec3::new(1.0, 2.0, 3.0),
        rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        scale: Vec3::new(2.0, 1.0, 1.0),
    };
    // Scale first: (1,0,0) -> (2,0,0); then a quarter turn about y sends +x to -z; then translate.
    expect_near(
        t.matrix().transform_point3(Vec3::X),
        Vec3::new(1.0, 2.0, 1.0),
        "point",
    );
    assert_eq!(Transform::default().matrix(), Mat4::IDENTITY);
}

#[test]
fn lane_pitch_and_hall_width_follow_the_geometry() {
    assert!(near(
        lane_pitch(&G),
        2.0 * (G.lane_half_width + G.gutter_width) + 0.30
    ));
    assert!(near(lane_pitch(&G), 1.83));
    assert!(near(
        hall_half_width(theme(ThemeId::NeonHall), &G),
        1.83 * 2.5 + 0.6
    ));
}

#[test]
fn no_prop_enters_the_corridors_or_the_camera_box() {
    for id in THEMES {
        let props = build_props(theme(id), &G);
        assert!(!props.is_empty(), "{}", theme(id).name);
        for (i, p) in props.iter().enumerate() {
            let b = bounds(p, &G);
            let name = theme(id).name;
            assert!(!overlaps(&b, &PLAY_CORRIDOR), "{name} prop {i}");
            assert!(!overlaps(&b, &SIGHT_CORRIDOR), "{name} prop {i}");
            assert!(!overlaps(&b, &CAMERA_BOX), "{name} prop {i}");
        }
    }
}

#[test]
fn ball_return_beside_the_main_lane_clears_the_corridor() {
    let mut nearest = f32::MAX;
    for p in build_props(theme(ThemeId::NeonHall), &G) {
        if p.shape == Shape::Box
            && (p.transform.scale.x - RETURN_WIDTH).abs() < 1e-5
            && p.transform.position.x > 0.0
        {
            nearest = nearest.min(bounds(&p, &G).min.x);
        }
    }
    assert!((nearest - 0.845).abs() < 1e-4, "{nearest}");
    assert!(nearest >= 0.82);
}

#[test]
fn indoor_halls_are_closed_by_walls_ceiling_and_back_wall() {
    for id in [ThemeId::NeonHall, ThemeId::ClassicAlley] {
        let t = theme(id);
        let w = hall_half_width(t, &G);
        let (mut left, mut right, mut ceiling, mut back) = (false, false, false, false);
        for p in build_props(t, &G) {
            let b = bounds(&p, &G);
            left = left || b.min.x <= -w;
            right = right || b.max.x >= w;
            ceiling = ceiling || b.max.y >= HALL_HEIGHT;
            back = back || b.max.z >= HALL_BACK_Z;
        }
        assert!(left && right && ceiling && back, "{}", t.name);
    }
}

#[test]
fn neighbour_lanes_carry_full_racks_standing_on_the_lane() {
    let pitch = lane_pitch(&G);
    for id in THEMES {
        let t = theme(id);
        let mut rack = 0;
        for p in build_props(t, &G) {
            if p.shape != Shape::Pin || (p.transform.position.y - G.pin_com_height).abs() > 1e-5 {
                continue;
            }
            rack += 1;
            assert!(p.pin_band, "{}", t.name);
            let lane = (p.transform.position.x / pitch).round();
            assert_ne!(lane, 0.0, "decor pins never stand on the main lane");
            assert!(lane.abs() <= f32::from(lanes_per_side(t)), "{}", t.name);
            assert!(
                (p.transform.position.x - lane * pitch).abs() <= G.lane_half_width,
                "{}",
                t.name
            );
            assert!(p.transform.position.z >= G.head_pin_z - 0.1, "{}", t.name);
            assert!(p.transform.position.z <= G.lane_length, "{}", t.name);
        }
        assert_eq!(rack, 10 * 2 * usize::from(lanes_per_side(t)), "{}", t.name);
    }
}

#[test]
fn neighbour_lanes_are_wood() {
    for id in THEMES {
        let t = theme(id);
        let props = build_props(t, &G);
        let wooden = count(&props, |p| p.material.pattern == Pattern::Wood);
        assert_eq!(wooden, 2 * usize::from(lanes_per_side(t)), "{}", t.name);
    }
}

#[test]
fn only_neon_hall_hangs_glowing_pin_icons() {
    for id in THEMES {
        let props = build_props(theme(id), &G);
        let icons = count(&props, |p| {
            p.shape == Shape::Pin && p.transform.position.y > 1.0
        });
        let expected = if theme(id).props.wall_screens { 4 } else { 0 };
        assert_eq!(icons, expected, "{}", theme(id).name);
        for p in &props {
            if p.shape == Shape::Pin && p.transform.position.y > 1.0 {
                assert!(p.pin_band);
                assert!(p.material.emissive);
            }
        }
    }
}

#[test]
fn neon_hall_has_pillars_strips_lights_and_screens() {
    let props = build_props(theme(ThemeId::NeonHall), &G);
    // pillars: 6 per side wall
    assert_eq!(
        count(&props, |p| p.shape == Shape::Box
            && near(p.transform.scale.x, 0.4)
            && near(p.transform.scale.z, 0.4)),
        12
    );
    // 4 wall strips + 1 masking strip + 14 ceiling panels + 4 screen panels
    assert_eq!(
        count(&props, |p| p.shape == Shape::Box && p.material.emissive),
        23
    );
    assert_eq!(count(&props, |p| p.shape == Shape::Cone), 0);
}

#[test]
fn classic_alley_has_ceiling_lights_and_a_score_monitor() {
    let props = build_props(theme(ThemeId::ClassicAlley), &G);
    // 14 ceiling panels + 1 monitor screen
    assert_eq!(
        count(&props, |p| p.shape == Shape::Box && p.material.emissive),
        15
    );
    assert_eq!(
        count(&props, |p| p.shape == Shape::Box
            && near(p.transform.position.x, 0.0)
            && near(p.transform.position.y, 2.9)
            && near(p.transform.position.z, G.lane_length + 0.2)),
        1
    );
    // no pillars
    assert_eq!(
        count(&props, |p| p.shape == Shape::Box
            && near(p.transform.scale.x, 0.4)
            && near(p.transform.scale.z, 0.4)),
        0
    );
}

#[test]
fn outdoor_terrace_has_fences_and_sixteen_trees_below_six_and_a_half_metres() {
    let props = build_props(theme(ThemeId::Outdoor), &G);
    assert_eq!(count(&props, |p| p.shape == Shape::Cylinder), 16);
    assert_eq!(count(&props, |p| p.shape == Shape::Cone), 16);
    // 19 posts per side
    assert_eq!(
        count(&props, |p| p.shape == Shape::Box
            && near(p.transform.scale.x, 0.1)
            && near(p.transform.scale.y, 1.0)),
        38
    );
    // two rails per side
    assert_eq!(
        count(&props, |p| p.shape == Shape::Box
            && near(p.transform.scale.z, 36.0)),
        4
    );
    assert_eq!(count(&props, |p| p.material.emissive), 0);
    let top = props
        .iter()
        .map(|p| bounds(p, &G).max.y)
        .fold(f32::MIN, f32::max);
    assert!(top <= 6.5);
    assert!((top - 6.25).abs() < 1e-3, "the tallest tree: {top}");
}

fn all_materials(t: &Theme) -> Vec<Material> {
    let mut out = vec![
        t.lane, t.gutter, t.ball, t.pin_body, t.pin_band, t.floor, t.wall, t.ceiling, t.trim,
        t.accent_a, t.accent_b, t.screen,
    ];
    out.extend(build_props(t, &G).iter().map(|p| p.material));
    out
}

#[test]
fn every_material_has_roughness_and_specular_in_range() {
    for id in THEMES {
        for m in all_materials(theme(id)) {
            let name = theme(id).name;
            assert!((0.0..=1.0).contains(&m.roughness), "{name}");
            assert!((0.0..=1.0).contains(&m.specular), "{name}");
            assert!(m.emissive_strength >= 1.0, "{name}");
        }
    }
}

/// The strongest glow in a theme (1 if nothing glows harder than its plain colour).
fn hottest_glow(t: &Theme) -> f32 {
    all_materials(t)
        .iter()
        .filter(|m| m.emissive)
        .fold(1.0, |h, m| h.max(m.emissive_strength))
}

#[test]
fn neon_hall_glows_hottest_and_outdoor_not_at_all() {
    let neon = hottest_glow(theme(ThemeId::NeonHall));
    let classic = hottest_glow(theme(ThemeId::ClassicAlley));
    assert!(neon > classic);
    assert!(
        classic > 1.0,
        "the bright pass keeps only what exceeds 1: Classic's ceiling panels glow faintly"
    );
    assert_eq!(hottest_glow(theme(ThemeId::Outdoor)), 1.0);
}

#[test]
fn the_lane_is_wood_and_the_ball_swirls() {
    for id in THEMES {
        let t = theme(id);
        assert_eq!(t.lane.pattern, Pattern::Wood, "{}", t.name);
        assert_eq!(t.ball.pattern, Pattern::BallSwirl, "{}", t.name);
        assert!(t.ball.roughness < t.floor.roughness, "{}", t.name);
    }
}
