//! The bowling lane of Bowlingbox: lane types, the physics trait and the throw state machine.
//!
//! [`Lane`] drives a [`PhysicsWorld`] and a [`bowlingbox_score::Game`] through the phases of a
//! throw. The world knows no bowling rules, so the state machine is tested with fakes.

mod lane;
mod rapier_world;
mod types;
mod world;

pub use lane::{Lane, LaneError};
pub use rapier_world::{
    PIN_MESH_OFFSET_Y, POCKET_ANGLE_DEG, POCKET_OFFSET, POCKET_POWER, RapierWorld,
};
pub use types::{
    BALL_START_Z, Body, Impact, LANE, LaneDimensions, MAX_AIM_ANGLE_DEG, MAX_AIM_OFFSET, Phase,
    Pose, Snapshot,
};
pub use world::PhysicsWorld;

impl Default for Lane<RapierWorld, bowlingbox_score::ScoreCalculator> {
    /// The game: the Rapier world and the ten-pin score calculator.
    fn default() -> Self {
        Self::new(
            RapierWorld::new(),
            bowlingbox_score::ScoreCalculator::default(),
        )
    }
}
