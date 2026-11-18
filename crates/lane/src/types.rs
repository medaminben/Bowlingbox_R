use std::fmt;

/// Throw state machine phases.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Aiming,
    Charging,
    Rolling,
    Settling,
    FrameDone,
    GameOver,
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Aiming => "aiming",
            Self::Charging => "charging",
            Self::Rolling => "rolling",
            Self::Settling => "settling",
            Self::FrameDone => "frame done",
            Self::GameOver => "game over",
        };
        f.write_str(name)
    }
}

/// Physical dimensions shared with the renderer (metres).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaneDimensions {
    pub half_width: f32,
    /// Foul line (z = 0) to the end of the pin deck.
    pub length: f32,
    /// Walkable area before the foul line (negative z).
    pub approach: f32,
    pub gutter_width: f32,
    pub gutter_depth: f32,
    pub ball_radius: f32,
    pub pin_height: f32,
    pub pin_max_radius: f32,
    /// Rigid-body origin above the pin base.
    pub pin_com_height: f32,
    pub pin_spacing: f32,
    pub head_pin_z: f32,
}

/// The lane of the C++ game.
pub const LANE: LaneDimensions = LaneDimensions {
    half_width: 0.525,
    length: 18.3,
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

/// Ball fully over the gutter (0.7 - r > half width).
pub const MAX_AIM_OFFSET: f32 = 0.7;
pub const MAX_AIM_ANGLE_DEG: f32 = 6.0;
pub const BALL_START_Z: f32 = -1.0;

/// Position in metres and rotation as a quaternion (x, y, z, w).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
}

impl Default for Pose {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

/// What a rigid body is, for impact reporting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Body {
    Ball,
    Pin,
    Lane,
    Gutter,
    Pit,
    Other,
}

/// A new contact between two bodies in one physics sub-step, with `a <= b`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Impact {
    pub a: Body,
    pub b: Body,
    /// Applied impulse in N·s.
    pub impulse: f32,
    /// World position in metres.
    pub position: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub phase: Phase,
    pub ball: Pose,
    pub pins: [Pose; 10],
    /// Upright on the deck right now.
    pub pin_standing: [bool; 10],
    /// Still part of the world, standing or lying.
    pub pin_in_play: [bool; 10],
    /// Lateral ball position in metres, within `[-MAX_AIM_OFFSET, MAX_AIM_OFFSET]`.
    pub aim_offset: f32,
    /// Degrees, within `[-MAX_AIM_ANGLE_DEG, MAX_AIM_ANGLE_DEG]`.
    pub aim_angle: f32,
    /// Throw power in `[0, 1]`.
    pub power: f32,
    /// Pins credited for the last throw, `None` if there was none yet.
    pub last_pins_down: Option<u8>,
}
