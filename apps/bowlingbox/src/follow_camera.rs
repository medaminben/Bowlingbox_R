//! The camera rig: waits behind the foul line, follows the ball down the lane while it rolls and
//! eases back afterwards.

use bowlingbox_lane::{LANE, Phase, Snapshot};
use bowlingbox_render::Camera;
use glam::Vec3;

pub const HOME_Z: f32 = -3.5;
pub const HEIGHT: f32 = 1.8;
pub const LOOK_AHEAD: f32 = 9.0;
/// Metres between the camera and a rolling ball.
pub const BEHIND_BALL: f32 = 3.5;
/// The camera stops this far before the end of the deck.
pub const DECK_MARGIN: f32 = 5.5;
/// 1/s: how fast the camera catches up.
pub const EASE_RATE: f32 = 6.0;
/// Closer than this to its goal, the camera lands on it, so the window can go back to sleep.
const SETTLE: f32 = 1e-3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FollowCamera {
    z: f32,
}

impl Default for FollowCamera {
    fn default() -> Self {
        Self { z: HOME_Z }
    }
}

impl FollowCamera {
    #[cfg(test)]
    pub fn z(self) -> f32 {
        self.z
    }

    /// The camera is at rest at its home spot.
    pub fn is_home(self) -> bool {
        self.z == HOME_Z
    }

    pub fn update(&mut self, snapshot: &Snapshot, dt: f32) {
        let target = match snapshot.phase {
            Phase::Rolling | Phase::Settling | Phase::FrameDone => {
                (snapshot.ball.position[2] - BEHIND_BALL).clamp(HOME_Z, LANE.length - DECK_MARGIN)
            }
            _ => HOME_Z,
        };
        self.z += (target - self.z) * (EASE_RATE * dt).min(1.0);
        if (target - self.z).abs() < SETTLE {
            self.z = target;
        }
    }

    pub fn apply_to(self, camera: &mut Camera) {
        camera.look_at(
            Vec3::new(0.0, HEIGHT, self.z),
            Vec3::new(0.0, 0.0, self.z + LOOK_AHEAD),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bowlingbox_lane::Pose;

    fn snapshot(phase: Phase, ball_z: f32) -> Snapshot {
        Snapshot {
            phase,
            ball: Pose {
                position: [0.0, 0.1, ball_z],
                ..Pose::default()
            },
            pins: [Pose::default(); 10],
            pin_standing: [true; 10],
            pin_in_play: [true; 10],
            aim_offset: 0.0,
            aim_angle: 0.0,
            power: 0.0,
            last_pins_down: None,
        }
    }

    fn run(cam: &mut FollowCamera, s: &Snapshot, frames: u32) {
        for _ in 0..frames {
            cam.update(s, 1.0 / 60.0);
        }
    }

    #[test]
    fn stays_home_while_aiming() {
        let mut cam = FollowCamera::default();
        run(&mut cam, &snapshot(Phase::Aiming, 10.0), 60);
        assert_eq!(cam.z(), HOME_Z);
    }

    #[test]
    fn follows_a_rolling_ball_but_not_past_the_deck() {
        let mut cam = FollowCamera::default();
        run(&mut cam, &snapshot(Phase::Rolling, 8.0), 600);
        assert!((cam.z() - (8.0 - BEHIND_BALL)).abs() < 1e-3);
        run(&mut cam, &snapshot(Phase::Rolling, 100.0), 600);
        assert!((cam.z() - (LANE.length - DECK_MARGIN)).abs() < 1e-3);
    }

    #[test]
    fn eases_back_home_and_then_rests() {
        let mut cam = FollowCamera::default();
        run(&mut cam, &snapshot(Phase::Rolling, 12.0), 300);
        assert!(!cam.is_home());
        run(&mut cam, &snapshot(Phase::Aiming, 0.0), 300);
        assert!(cam.is_home());
    }

    #[test]
    fn looks_down_the_lane_from_the_rig_height() {
        let mut cam = FollowCamera::default();
        run(&mut cam, &snapshot(Phase::Rolling, 8.0), 600);
        let mut camera = Camera::default();
        cam.apply_to(&mut camera);
        assert_eq!(camera.eye, Vec3::new(0.0, HEIGHT, cam.z()));
        assert_eq!(camera.target, Vec3::new(0.0, 0.0, cam.z() + LOOK_AHEAD));
    }
}
