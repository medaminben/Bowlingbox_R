//! The player's camera.

use glam::camera::lh::{proj::directx, view};
use glam::{Mat4, Vec3};

const FOV_Y_DEG: f32 = 45.0;
const NEAR: f32 = 0.1;
const FAR: f32 = 150.0;

/// A perspective camera looking at `target` from `eye`.
///
/// Handedness: the C++ camera is right-handed (`glm::lookAt`) and draws +x on the screen's left
/// when looking down +z. The lane crate's +x is the bowler's right and must appear on the right,
/// so this camera is left-handed: glam's left-handed `look_at_mat4` and its DirectX/WebGPU
/// `perspective` read the world as x right, y up, z forward. Depth runs 0..1 and NDC y points up,
/// as in wgpu. World data and meshes are unchanged;
/// only the picture is mirrored relative to C++, so front faces are clockwise on screen (use
/// `FrontFace::Cw`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    /// Width over height.
    pub aspect: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            eye: Vec3::new(0.0, 1.8, -3.5),
            target: Vec3::new(0.0, 0.0, 6.0),
            aspect: 16.0 / 9.0,
        }
    }
}

impl Camera {
    pub fn look_at(&mut self, eye: Vec3, target: Vec3) {
        self.eye = eye;
        self.target = target;
    }

    /// Sets the aspect from a viewport size; a zero side counts as 1.
    #[allow(clippy::cast_precision_loss)]
    pub fn set_viewport(&mut self, width: u32, height: u32) {
        self.aspect = width.max(1) as f32 / height.max(1) as f32;
    }

    #[must_use]
    pub fn view(&self) -> Mat4 {
        view::look_at_mat4(self.eye, self.target, Vec3::Y)
    }

    #[must_use]
    pub fn projection(&self) -> Mat4 {
        directx::perspective(FOV_Y_DEG.to_radians(), self.aspect, NEAR, FAR)
    }
}
