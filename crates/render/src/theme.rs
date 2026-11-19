//! The three visual themes: colours, fog, lights, exposure, materials and prop sets.

use glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeId {
    NeonHall,
    ClassicAlley,
    Outdoor,
}

/// Every theme, in the order `next_theme` cycles through them.
pub const THEMES: [ThemeId; 3] = [ThemeId::NeonHall, ThemeId::ClassicAlley, ThemeId::Outdoor];

/// Surface pattern drawn by the lit shader. The values are the shader's pattern id.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum Pattern {
    #[default]
    None = 0,
    Wood = 1,
    BallSwirl = 2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    /// sRGB as authored; the renderer converts to linear.
    pub color: Vec3,
    /// Glows: ignores the lights, fogged at half strength.
    pub emissive: bool,
    /// 0 = sharp highlight, 1 = chalky.
    pub roughness: f32,
    /// Reflectance at normal incidence.
    pub specular: f32,
    /// Above 1 pushes the glow over the bloom threshold.
    pub emissive_strength: f32,
    pub pattern: Pattern,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            color: Vec3::ONE,
            emissive: false,
            roughness: 0.6,
            specular: 0.04,
            emissive_strength: 1.0,
            pattern: Pattern::None,
        }
    }
}

/// A light without shadows; falls off with distance and stops at `range`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointLight {
    pub position: Vec3,
    /// Linear.
    pub color: Vec3,
    pub intensity: f32,
    /// Metres.
    pub range: f32,
}

pub const MAX_POINT_LIGHTS: usize = 4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct PropSet {
    pub pillars: bool,
    pub neon_strips: bool,
    pub ceiling_lights: bool,
    pub wall_screens: bool,
    pub score_monitor: bool,
    pub fences: bool,
    pub trees: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub id: ThemeId,
    /// "neon", "classic", "outdoor": the only name table.
    pub name: &'static str,
    /// "Neon Hall", "Classic Alley", "Outdoor".
    pub label: &'static str,
    pub background_top: Vec3,
    pub background_bottom: Vec3,
    pub fog_color: Vec3,
    /// Metres from the camera.
    pub fog_start: f32,
    pub fog_end: f32,
    /// Direction the light travels.
    pub light_dir: Vec3,
    /// Linear; the key light casts the shadows.
    pub key_color: Vec3,
    pub key_intensity: f32,
    /// Linear; hemisphere ambient from above and below.
    pub sky_ambient: Vec3,
    pub ground_ambient: Vec3,
    pub point_lights: [PointLight; MAX_POINT_LIGHTS],
    /// 0 to `MAX_POINT_LIGHTS`.
    pub point_light_count: usize,
    /// Above 0.
    pub exposure: f32,
    /// 0 to 1.
    pub bloom_strength: f32,
    pub lane: Material,
    pub gutter: Material,
    pub ball: Material,
    pub pin_body: Material,
    pub pin_band: Material,
    /// Walls and ceiling; otherwise open ground.
    pub indoor: bool,
    pub neighbour_lanes_per_side: i32,
    pub floor: Material,
    pub wall: Material,
    pub ceiling: Material,
    pub trim: Material,
    pub accent_a: Material,
    pub accent_b: Material,
    pub screen: Material,
    pub props: PropSet,
}

const fn material(
    (r, g, b): (f32, f32, f32),
    emissive: bool,
    roughness: f32,
    specular: f32,
    emissive_strength: f32,
    pattern: Pattern,
) -> Material {
    Material {
        color: Vec3::new(r, g, b),
        emissive,
        roughness,
        specular,
        emissive_strength,
        pattern,
    }
}

const fn lit(r: f32, g: f32, b: f32) -> Material {
    material((r, g, b), false, 0.8, 0.04, 1.0, Pattern::None)
}

const fn glow(r: f32, g: f32, b: f32, strength: f32) -> Material {
    material((r, g, b), true, 1.0, 0.0, strength, Pattern::None)
}

const fn wood(r: f32, g: f32, b: f32) -> Material {
    material((r, g, b), false, 0.25, 0.04, 1.0, Pattern::Wood)
}

const fn metal(r: f32, g: f32, b: f32) -> Material {
    material((r, g, b), false, 0.40, 0.50, 1.0, Pattern::None)
}

const fn point(position: [f32; 3], color: [f32; 3], intensity: f32, range: f32) -> PointLight {
    PointLight {
        position: Vec3::from_array(position),
        color: Vec3::from_array(color),
        intensity,
        range,
    }
}

const BALL: Material = material(
    (0.098, 0.365, 0.925),
    false,
    0.15,
    0.06,
    1.0,
    Pattern::BallSwirl,
);
const PIN_BODY: Material = material((0.941, 0.941, 0.941), false, 0.30, 0.05, 1.0, Pattern::None);
const PIN_BAND: Material = material((0.902, 0.0, 0.0), false, 0.30, 0.05, 1.0, Pattern::None);
const UNUSED: Material = lit(0.5, 0.5, 0.5);
const NO_LIGHT: PointLight = point([0.0; 3], [0.0; 3], 0.0, 1.0);

static NEON_HALL: Theme = Theme {
    id: ThemeId::NeonHall,
    name: "neon",
    label: "Neon Hall",
    background_top: Vec3::new(0.01, 0.02, 0.08),
    background_bottom: Vec3::new(0.04, 0.05, 0.18),
    fog_color: Vec3::new(0.03, 0.04, 0.14),
    fog_start: 12.0,
    fog_end: 35.0,
    light_dir: Vec3::new(-0.3, -1.0, 0.4),
    key_color: Vec3::new(0.80, 0.85, 1.00),
    key_intensity: 2.5,
    sky_ambient: Vec3::new(0.05, 0.06, 0.16),
    ground_ambient: Vec3::new(0.02, 0.02, 0.05),
    point_lights: [
        // Cyan, along the low strips.
        point([-4.5, 1.2, 3.0], [0.0, 0.79, 1.0], 12.0, 9.0),
        point([4.5, 1.2, 3.0], [0.0, 0.79, 1.0], 12.0, 9.0),
        // Violet, along the high strips.
        point([-4.5, 3.2, 12.0], [0.34, 0.03, 1.0], 12.0, 9.0),
        point([4.5, 3.2, 12.0], [0.34, 0.03, 1.0], 12.0, 9.0),
    ],
    point_light_count: 4,
    exposure: 1.0,
    bloom_strength: 0.6,
    lane: wood(0.831, 0.639, 0.451),
    gutter: metal(0.008, 0.055, 0.22),
    ball: BALL,
    pin_body: PIN_BODY,
    pin_band: PIN_BAND,
    indoor: true,
    neighbour_lanes_per_side: 2,
    floor: lit(0.02, 0.03, 0.10),
    wall: lit(0.03, 0.05, 0.20),
    ceiling: lit(0.02, 0.03, 0.12),
    trim: lit(0.05, 0.08, 0.30),
    accent_a: glow(0.0, 0.9, 1.0, 5.0),
    accent_b: glow(0.62, 0.2, 1.0, 5.0),
    screen: glow(0.1, 0.3, 0.9, 4.0),
    props: PropSet {
        pillars: true,
        neon_strips: true,
        ceiling_lights: true,
        wall_screens: true,
        score_monitor: false,
        fences: false,
        trees: false,
    },
};

static CLASSIC_ALLEY: Theme = Theme {
    id: ThemeId::ClassicAlley,
    name: "classic",
    label: "Classic Alley",
    background_top: Vec3::new(0.93, 0.88, 0.78),
    background_bottom: Vec3::new(0.80, 0.72, 0.60),
    fog_color: Vec3::new(0.85, 0.80, 0.70),
    fog_start: 20.0,
    fog_end: 45.0,
    light_dir: Vec3::new(-0.2, -1.0, 0.3),
    key_color: Vec3::new(1.00, 0.92, 0.80),
    key_intensity: 3.0,
    sky_ambient: Vec3::new(0.35, 0.32, 0.28),
    ground_ambient: Vec3::new(0.12, 0.09, 0.06),
    point_lights: [
        // Warm, under the ceiling lights.
        point([0.0, 4.2, 2.0], [1.0, 0.82, 0.6], 10.0, 8.0),
        point([0.0, 4.2, 7.0], [1.0, 0.82, 0.6], 10.0, 8.0),
        point([0.0, 4.2, 12.0], [1.0, 0.82, 0.6], 10.0, 8.0),
        point([0.0, 4.2, 17.0], [1.0, 0.82, 0.6], 10.0, 8.0),
    ],
    point_light_count: 4,
    exposure: 1.0,
    bloom_strength: 0.15,
    lane: wood(0.86, 0.70, 0.50),
    gutter: metal(0.25, 0.25, 0.28),
    ball: BALL,
    pin_body: PIN_BODY,
    pin_band: PIN_BAND,
    indoor: true,
    neighbour_lanes_per_side: 2,
    floor: lit(0.45, 0.30, 0.18),
    wall: lit(0.88, 0.82, 0.70),
    ceiling: lit(0.95, 0.95, 0.92),
    trim: lit(0.50, 0.32, 0.18),
    accent_a: glow(1.0, 0.96, 0.85, 1.5),
    accent_b: lit(0.3, 0.3, 0.3),
    screen: glow(0.05, 0.1, 0.3, 1.0),
    props: PropSet {
        pillars: false,
        neon_strips: false,
        ceiling_lights: true,
        wall_screens: false,
        score_monitor: true,
        fences: false,
        trees: false,
    },
};

static OUTDOOR: Theme = Theme {
    id: ThemeId::Outdoor,
    name: "outdoor",
    label: "Outdoor",
    background_top: Vec3::new(0.35, 0.60, 0.95),
    background_bottom: Vec3::new(0.80, 0.90, 1.00),
    fog_color: Vec3::new(0.80, 0.88, 0.98),
    fog_start: 25.0,
    fog_end: 60.0,
    light_dir: Vec3::new(-0.4, -1.0, 0.3),
    key_color: Vec3::new(1.00, 0.96, 0.88),
    key_intensity: 3.5,
    sky_ambient: Vec3::new(0.30, 0.42, 0.60),
    ground_ambient: Vec3::new(0.10, 0.14, 0.06),
    point_lights: [NO_LIGHT; MAX_POINT_LIGHTS],
    point_light_count: 0,
    exposure: 1.0,
    bloom_strength: 0.05,
    lane: wood(0.831, 0.639, 0.451),
    gutter: metal(0.30, 0.30, 0.32),
    ball: BALL,
    pin_body: PIN_BODY,
    pin_band: PIN_BAND,
    indoor: false,
    neighbour_lanes_per_side: 1,
    floor: lit(0.25, 0.55, 0.20),
    wall: UNUSED,
    ceiling: UNUSED,
    trim: lit(0.55, 0.40, 0.25),
    accent_a: lit(0.15, 0.45, 0.15),
    accent_b: lit(0.40, 0.26, 0.13),
    screen: lit(0.1, 0.1, 0.1),
    props: PropSet {
        pillars: false,
        neon_strips: false,
        ceiling_lights: false,
        wall_screens: false,
        score_monitor: false,
        fences: true,
        trees: true,
    },
};

#[must_use]
pub fn theme(id: ThemeId) -> &'static Theme {
    match id {
        ThemeId::NeonHall => &NEON_HALL,
        ThemeId::ClassicAlley => &CLASSIC_ALLEY,
        ThemeId::Outdoor => &OUTDOOR,
    }
}

/// ASCII case-insensitive; `None` for an unknown name.
#[must_use]
pub fn theme_from_name(name: &str) -> Option<ThemeId> {
    THEMES
        .into_iter()
        .find(|&id| theme(id).name.eq_ignore_ascii_case(name))
}

/// Neon -> Classic -> Outdoor -> Neon.
#[must_use]
pub fn next_theme(id: ThemeId) -> ThemeId {
    let index = THEMES.iter().position(|&t| t == id).unwrap_or(0);
    THEMES[(index + 1) % THEMES.len()]
}
