use crate::types::{Impact, Pose};

/// Rigid-body world: static lane, gutters and pit, one ball and ten pins.
///
/// It knows no bowling rules or phases. [`crate::Lane`] drives it through this trait, so the
/// physics engine can be swapped or faked in tests.
pub trait PhysicsWorld {
    fn step(&mut self, dt: f32);

    /// Puts the ball at rest on the approach at `z = BALL_START_Z`.
    fn place_ball(&mut self, offset: f32);
    fn launch_ball(&mut self, offset: f32, angle_deg: f32, speed: f32);

    /// All ten pins standing and in play.
    fn reset_rack(&mut self);
    /// A fresh world, deterministic between frames. Drops pending impacts: [`crate::Lane`]
    /// relies on that when it racks a new frame.
    fn rebuild(&mut self);
    /// Takes a pin out of the world and drops pending impacts.
    fn remove_pin(&mut self, i: usize);

    fn ball_pose(&self) -> Pose;
    fn pin_pose(&self, i: usize) -> Pose;
    fn is_pin_standing(&self, i: usize) -> bool;
    fn is_pin_in_play(&self, i: usize) -> bool;
    fn ball_in_pit(&self) -> bool;
    fn ball_speed(&self) -> f32;
    /// Every body in the world is nearly at rest.
    fn all_slow(&self) -> bool;

    /// Impacts recorded since the last call, oldest first; empties the buffer.
    fn drain_impacts(&mut self) -> Vec<Impact>;
}
