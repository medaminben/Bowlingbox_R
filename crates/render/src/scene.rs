//! What the renderer is told to draw each frame, and the sizes it needs to know.

use glam::{Mat4, Quat, Vec3};

/// Position, rotation and scale of a mesh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    /// Scale first, then rotate, then translate.
    #[must_use]
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }
}

/// Physical sizes in metres; the app copies them from the lane crate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneGeometry {
    pub lane_half_width: f32,
    pub lane_length: f32,
    pub approach: f32,
    pub gutter_width: f32,
    pub gutter_depth: f32,
    pub ball_radius: f32,
    pub pin_height: f32,
    pub pin_max_radius: f32,
    /// The pin mesh's origin above its base (the C++ `pinComHeight`, the lane crate's
    /// `pin_origin_height`).
    pub pin_com_height: f32,
    pub pin_spacing: f32,
    pub head_pin_z: f32,
}

/// One frame's moving parts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SceneDescription {
    pub ball: Transform,
    pub pins: [Transform; 10],
    pub pin_visible: [bool; 10],
    pub show_aim_guide: bool,
    /// Metres, the ball's start x.
    pub aim_offset: f32,
    /// Rotation about y, in degrees.
    pub aim_angle_deg: f32,
}
