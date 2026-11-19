//! The hall, ground, neighbour lanes, ball returns and theme decor as a list of props.

use crate::geometry::{MeshData, box_data, cone_data, cylinder_data, pin_data};
use crate::scene::{SceneGeometry, Transform};
use crate::theme::{Material, Pattern, Theme};
use glam::{Mat4, Vec2, Vec3};

// Layout constants, metres.
/// Between the gutters of adjacent lanes.
pub const RETURN_GAP: f32 = 0.30;
/// The ball-return box in the middle of that gap.
pub const RETURN_WIDTH: f32 = 0.14;
pub const RETURN_HEIGHT: f32 = 0.3;
/// The ceiling's inner face.
pub const HALL_HEIGHT: f32 = 4.5;
pub const HALL_FRONT_Z: f32 = -8.0;
pub const HALL_BACK_Z: f32 = 22.0;
pub const WALL_THICKNESS: f32 = 0.2;
pub const FLOOR_TOP_Y: f32 = -0.15;
/// The pit behind the deck (the physics world's pit).
pub const PIT_LENGTH: f32 = 2.4;
pub const PIT_TOP_Y: f32 = -0.25;

const SLAB: f32 = 0.1; // floor and pit thickness
const PIT_MARGIN: f32 = 0.12; // the pit is this much wider than the gutters on each side
const MASK_BOTTOM_Y: f32 = 0.9;
const MASK_TOP_Y: f32 = 2.4;
const OUTDOOR_HALF_WIDTH: f32 = 40.0;
const OUTDOOR_FRONT_Z: f32 = -20.0;
const OUTDOOR_BACK_Z: f32 = 80.0;

const STRIP: f32 = 0.06; // neon strip thickness
const SCREEN_Y: f32 = 2.3;
const ICON_WHITE: Material = Material {
    color: Vec3::ONE,
    emissive: true,
    roughness: 1.0,
    specular: 0.0,
    emissive_strength: 1.0,
    pattern: Pattern::None,
};

/// Box, Cylinder and Cone are unit shapes (1 m across, centred on the origin), so
/// `transform.scale` is their size in metres. Pin is the real pin mesh, base at `y = -pin_com_height`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    #[default]
    Box,
    Cylinder,
    Cone,
    Pin,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Prop {
    pub shape: Shape,
    pub transform: Transform,
    pub material: Material,
    /// Draw the theme's pin band on the neck (Pin only).
    pub pin_band: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    /// The axis-aligned box around this box's eight corners through `m`.
    #[must_use]
    pub fn transformed(&self, m: Mat4) -> Self {
        let mut out = Self {
            min: Vec3::splat(f32::MAX),
            max: Vec3::splat(f32::MIN),
        };
        for i in 0..8 {
            let corner = Vec3::new(
                if i & 1 != 0 { self.max.x } else { self.min.x },
                if i & 2 != 0 { self.max.y } else { self.min.y },
                if i & 4 != 0 { self.max.z } else { self.min.z },
            );
            let w = m.transform_point3(corner);
            out.min = out.min.min(w);
            out.max = out.max.max(w);
        }
        out
    }

    /// Distance from `p` to the nearest point of the box; 0 inside it.
    #[must_use]
    pub fn distance_to(&self, p: Vec3) -> f32 {
        (self.min - p).max(p - self.max).max(Vec3::ZERO).length()
    }
}

/// A shape centred at `centre`, `size` metres across (unit shapes are 1 m).
fn shaped(shape: Shape, centre: Vec3, size: Vec3, material: Material) -> Prop {
    Prop {
        shape,
        transform: Transform {
            position: centre,
            scale: size,
            ..Transform::default()
        },
        material,
        pin_band: false,
    }
}

fn boxed(centre: Vec3, size: Vec3, material: Material) -> Prop {
    shaped(Shape::Box, centre, size, material)
}

/// An axis-aligned box from its low corner to its high corner.
fn slab(lo: [f32; 3], hi: [f32; 3], material: Material) -> Prop {
    let (lo, hi) = (Vec3::from_array(lo), Vec3::from_array(hi));
    boxed((lo + hi) * 0.5, hi - lo, material)
}

/// A pin whose origin (`pin_com_height` above its base) is at `origin`.
fn pin(origin: Vec3, scale: Vec3, material: Material) -> Prop {
    shaped(Shape::Pin, origin, scale, material)
}

/// Distance between lane centres: `2 * (lane_half_width + gutter_width) + RETURN_GAP`.
#[must_use]
pub fn lane_pitch(g: &SceneGeometry) -> f32 {
    2.0 * (g.lane_half_width + g.gutter_width) + RETURN_GAP
}

/// Inner half-width of an indoor hall: `lane_pitch * (neighbour_lanes_per_side + 0.5) + 0.6`.
#[must_use]
pub fn hall_half_width(theme: &Theme, g: &SceneGeometry) -> f32 {
    lane_pitch(g) * (theme.neighbour_lanes_per_side as f32 + 0.5) + 0.6
}

/// The physics world's pin positions: head pin first, then rows of 2, 3 and 4.
fn rack(g: &SceneGeometry) -> [Vec2; 10] {
    let s = g.pin_spacing;
    let row = s * 0.866_025_4;
    let z0 = g.head_pin_z;
    [
        Vec2::new(0.0, z0),
        Vec2::new(-s / 2.0, z0 + row),
        Vec2::new(s / 2.0, z0 + row),
        Vec2::new(-s, z0 + 2.0 * row),
        Vec2::new(0.0, z0 + 2.0 * row),
        Vec2::new(s, z0 + 2.0 * row),
        Vec2::new(-1.5 * s, z0 + 3.0 * row),
        Vec2::new(-s / 2.0, z0 + 3.0 * row),
        Vec2::new(s / 2.0, z0 + 3.0 * row),
        Vec2::new(1.5 * s, z0 + 3.0 * row),
    ]
}

/// Ground slabs (top at `FLOOR_TOP_Y`) over `[-half_width, half_width] x [front, back]`, with a
/// hole over the main pit, plus the dark pit box itself.
fn add_ground_and_pit(
    out: &mut Vec<Prop>,
    t: &Theme,
    g: &SceneGeometry,
    half_width: f32,
    front: f32,
    back: f32,
) {
    let px = g.lane_half_width + g.gutter_width + PIT_MARGIN;
    let (y0, y1) = (FLOOR_TOP_Y - SLAB, FLOOR_TOP_Y);
    let pit_end = g.lane_length + PIT_LENGTH;
    out.push(slab([-half_width, y0, front], [-px, y1, back], t.floor));
    out.push(slab([px, y0, front], [half_width, y1, back], t.floor));
    out.push(slab([-px, y0, front], [px, y1, g.lane_length], t.floor));
    out.push(slab([-px, y0, pit_end], [px, y1, back], t.floor));
    out.push(slab(
        [-px, PIT_TOP_Y - SLAB, g.lane_length],
        [px, PIT_TOP_Y, pit_end],
        t.gutter,
    ));
}

/// Floor, walls, ceiling and the masking unit over the pinsetters.
fn add_hall(out: &mut Vec<Prop>, t: &Theme, g: &SceneGeometry) {
    let (w, h, th) = (hall_half_width(t, g), HALL_HEIGHT, WALL_THICKNESS);
    add_ground_and_pit(out, t, g, w, HALL_FRONT_Z, HALL_BACK_Z);
    out.push(slab(
        [-w - th, FLOOR_TOP_Y, HALL_FRONT_Z],
        [-w, h, HALL_BACK_Z],
        t.wall,
    ));
    out.push(slab(
        [w, FLOOR_TOP_Y, HALL_FRONT_Z],
        [w + th, h, HALL_BACK_Z],
        t.wall,
    ));
    out.push(slab(
        [-w, FLOOR_TOP_Y, HALL_BACK_Z],
        [w, h, HALL_BACK_Z + th],
        t.wall,
    ));
    out.push(slab(
        [-w, FLOOR_TOP_Y, HALL_FRONT_Z - th],
        [w, h, HALL_FRONT_Z],
        t.wall,
    ));
    out.push(slab(
        [-w - th, h, HALL_FRONT_Z - th],
        [w + th, h + th, HALL_BACK_Z + th],
        t.ceiling,
    ));
    out.push(slab(
        [-w, MASK_BOTTOM_Y, g.lane_length + 0.05],
        [w, MASK_TOP_Y, g.lane_length + 0.35],
        t.trim,
    ));
}

/// A copy of the main lane centred at `x0`: lane with approach, two gutters, a full rack.
fn add_neighbour_lane(out: &mut Vec<Prop>, t: &Theme, g: &SceneGeometry, x0: f32) {
    let hw = g.lane_half_width;
    let outer = hw + g.gutter_width;
    out.push(slab(
        [x0 - hw, -0.1, -g.approach],
        [x0 + hw, 0.0, g.lane_length],
        t.lane,
    ));
    for side in [-1.0_f32, 1.0] {
        let (a, b) = (x0 + side * hw, x0 + side * outer);
        out.push(slab(
            [a.min(b), -g.gutter_depth - 0.1, 0.0],
            [a.max(b), -g.gutter_depth, g.lane_length],
            t.gutter,
        ));
    }
    for p in rack(g) {
        let mut standing = pin(
            Vec3::new(x0 + p.x, g.pin_com_height, p.y),
            Vec3::ONE,
            t.pin_body,
        );
        standing.pin_band = true;
        out.push(standing);
    }
}

/// One return in the middle of every gap between adjacent lanes.
fn add_ball_returns(out: &mut Vec<Prop>, t: &Theme, g: &SceneGeometry) {
    let n = t.neighbour_lanes_per_side;
    let pitch = lane_pitch(g);
    for k in -n..n {
        let x = (k as f32 + 0.5) * pitch;
        out.push(slab(
            [x - RETURN_WIDTH / 2.0, 0.0, -g.approach],
            [x + RETURN_WIDTH / 2.0, RETURN_HEIGHT, g.lane_length],
            t.gutter,
        ));
    }
}

/// The x range of a layer `from..to` metres in from the side wall at `x = side * w`.
fn layer(side: f32, w: f32, from: f32, to: f32) -> (f32, f32) {
    let (a, b) = (side * (w - from), side * (w - to));
    (a.min(b), a.max(b))
}

fn add_pillars(out: &mut Vec<Prop>, t: &Theme, w: f32) {
    for z in [-6.0, -1.0, 4.0, 9.0, 14.0, 19.0] {
        for side in [-1.0, 1.0] {
            let (x0, x1) = layer(side, w, 0.0, 0.4);
            out.push(slab(
                [x0, FLOOR_TOP_Y, z - 0.2],
                [x1, HALL_HEIGHT, z + 0.2],
                t.trim,
            ));
        }
    }
}

fn add_neon_strips(out: &mut Vec<Prop>, t: &Theme, g: &SceneGeometry, w: f32) {
    for side in [-1.0, 1.0] {
        let (x0, x1) = layer(side, w, 0.0, STRIP);
        out.push(slab(
            [x0, 1.0 - STRIP / 2.0, HALL_FRONT_Z],
            [x1, 1.0 + STRIP / 2.0, HALL_BACK_Z],
            t.accent_a,
        ));
        out.push(slab(
            [x0, 3.4 - STRIP / 2.0, HALL_FRONT_Z],
            [x1, 3.4 + STRIP / 2.0, HALL_BACK_Z],
            t.accent_b,
        ));
    }
    // Along the masking unit's lower front edge, centred on its face (z = lane_length + 0.05).
    let z = g.lane_length + 0.05;
    out.push(slab(
        [-w, 0.9 - STRIP / 2.0, z - STRIP / 2.0],
        [w, 0.9 + STRIP / 2.0, z + STRIP / 2.0],
        t.accent_a,
    ));
}

fn add_ceiling_lights(out: &mut Vec<Prop>, t: &Theme, g: &SceneGeometry) {
    let x = lane_pitch(g) / 2.0;
    for i in 0..7 {
        let z = -6.0 + 4.0 * i as f32;
        for side in [-1.0, 1.0] {
            out.push(boxed(
                Vec3::new(side * x, HALL_HEIGHT - 0.2, z),
                Vec3::new(1.2, 0.05, 0.3),
                t.accent_a,
            ));
        }
    }
}

fn add_wall_screens(out: &mut Vec<Prop>, t: &Theme, w: f32) {
    for side in [-1.0, 1.0] {
        for z in [6.0, 12.0] {
            let (fx0, fx1) = layer(side, w, 0.0, 0.08);
            out.push(slab(
                [fx0, SCREEN_Y - 0.8, z - 1.3],
                [fx1, SCREEN_Y + 0.8, z + 1.3],
                t.trim,
            ));
            let (px0, px1) = layer(side, w, 0.08, 0.10);
            out.push(slab(
                [px0, SCREEN_Y - 0.7, z - 1.2],
                [px1, SCREEN_Y + 0.7, z + 1.2],
                t.screen,
            ));
            let mut icon = pin(
                Vec3::new(side * (w - 0.12), SCREEN_Y - 0.15, z),
                Vec3::new(0.2, 3.0, 3.0),
                ICON_WHITE,
            );
            icon.pin_band = true;
            out.push(icon);
        }
    }
}

fn add_score_monitor(out: &mut Vec<Prop>, t: &Theme, g: &SceneGeometry) {
    let z = g.lane_length + 0.2;
    out.push(boxed(
        Vec3::new(0.0, 2.9, z),
        Vec3::new(3.0, 0.9, 0.1),
        t.trim,
    ));
    out.push(slab(
        [-1.4, 2.55, z - 0.07],
        [1.4, 3.25, z - 0.05],
        t.screen,
    ));
}

fn add_fences(out: &mut Vec<Prop>, t: &Theme, g: &SceneGeometry) {
    let f = lane_pitch(g) * t.neighbour_lanes_per_side as f32
        + g.lane_half_width
        + g.gutter_width
        + 1.0;
    for side in [-1.0, 1.0] {
        let x = side * f;
        for i in 0..19 {
            let z = -6.0 + 2.0 * i as f32;
            out.push(slab(
                [x - 0.05, FLOOR_TOP_Y, z - 0.05],
                [x + 0.05, FLOOR_TOP_Y + 1.0, z + 0.05],
                t.trim,
            ));
        }
        for y in [0.4, 0.8] {
            out.push(boxed(
                Vec3::new(x, y, 12.0),
                Vec3::new(0.05, 0.08, 36.0),
                t.trim,
            ));
        }
    }
}

/// (x, z, scale), mirrored to both sides: 16 trees.
const TREES: [(f32, f32, f32); 8] = [
    (6.5, -4.0, 1.0),
    (8.0, 4.0, 1.3),
    (6.0, 12.0, 0.9),
    (9.5, 18.0, 1.4),
    (7.0, 26.0, 1.1),
    (11.0, 34.0, 1.5),
    (8.5, 44.0, 1.2),
    (12.0, 55.0, 1.6),
];

fn add_trees(out: &mut Vec<Prop>, t: &Theme) {
    for (tx, z, s) in TREES {
        for side in [-1.0, 1.0] {
            let x = side * tx;
            out.push(shaped(
                Shape::Cylinder,
                Vec3::new(x, FLOOR_TOP_Y + 0.75 * s, z),
                Vec3::new(0.3 * s, 1.5 * s, 0.3 * s),
                t.accent_b,
            ));
            out.push(shaped(
                Shape::Cone,
                Vec3::new(x, FLOOR_TOP_Y + 2.75 * s, z),
                Vec3::new(2.4 * s, 2.5 * s, 2.4 * s),
                t.accent_a,
            ));
        }
    }
}

/// Appends the theme's optional props as selected by `theme.props`.
fn add_decor(out: &mut Vec<Prop>, t: &Theme, g: &SceneGeometry) {
    let p = &t.props;
    if t.indoor {
        let w = hall_half_width(t, g);
        if p.pillars {
            add_pillars(out, t, w);
        }
        if p.neon_strips {
            add_neon_strips(out, t, g, w);
        }
        if p.ceiling_lights {
            add_ceiling_lights(out, t, g);
        }
        if p.wall_screens {
            add_wall_screens(out, t, w);
        }
        if p.score_monitor {
            add_score_monitor(out, t, g);
        }
    } else {
        if p.fences {
            add_fences(out, t, g);
        }
        if p.trees {
            add_trees(out, t);
        }
    }
}

/// Every prop of a theme around the main lane: ground, hall, neighbour lanes, returns and decor.
#[must_use]
pub fn build_props(t: &Theme, g: &SceneGeometry) -> Vec<Prop> {
    let mut props = Vec::new();
    if t.indoor {
        add_hall(&mut props, t, g);
    } else {
        add_ground_and_pit(
            &mut props,
            t,
            g,
            OUTDOOR_HALF_WIDTH,
            OUTDOOR_FRONT_Z,
            OUTDOOR_BACK_Z,
        );
    }
    let pitch = lane_pitch(g);
    for k in 1..=t.neighbour_lanes_per_side {
        for side in [-1.0, 1.0] {
            add_neighbour_lane(&mut props, t, g, side * pitch * k as f32);
        }
    }
    add_ball_returns(&mut props, t, g);
    add_decor(&mut props, t, g);
    props
}

/// The mesh drawn for a shape.
#[must_use]
pub fn shape_mesh(shape: Shape, g: &SceneGeometry) -> MeshData {
    match shape {
        Shape::Box => box_data(0.5, 0.5, 0.5),
        Shape::Cylinder => cylinder_data(0.5, 1.0, 24),
        Shape::Cone => cone_data(0.5, 1.0, 24),
        Shape::Pin => pin_data(g.pin_height, g.pin_max_radius, g.pin_com_height, 40),
    }
}

/// The local box of a shape's mesh (min and max over its vertices).
#[must_use]
pub fn local_bounds(shape: Shape, g: &SceneGeometry) -> Aabb {
    if shape == Shape::Pin {
        return Aabb {
            min: Vec3::new(-g.pin_max_radius, -g.pin_com_height, -g.pin_max_radius),
            max: Vec3::new(
                g.pin_max_radius,
                g.pin_height - g.pin_com_height,
                g.pin_max_radius,
            ),
        };
    }
    Aabb {
        min: Vec3::splat(-0.5),
        max: Vec3::splat(0.5),
    }
}

/// The world box: `local_bounds`' eight corners through `transform.matrix()`.
#[must_use]
pub fn bounds(prop: &Prop, g: &SceneGeometry) -> Aabb {
    local_bounds(prop.shape, g).transformed(prop.transform.matrix())
}
