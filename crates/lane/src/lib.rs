//! The bowling lane of Bowlingbox: lane types, the physics trait and the throw state machine.
//!
//! [`Lane`] drives a [`PhysicsWorld`] and a [`bowlingbox_score::Game`] through the phases of a
//! throw. The world knows no bowling rules, so the state machine is tested with fakes.

mod lane;
mod types;
mod world;

pub use lane::{Lane, LaneError};
pub use types::{
    BALL_START_Z, Body, Impact, LANE, LaneDimensions, MAX_AIM_ANGLE_DEG, MAX_AIM_OFFSET, Phase,
    Pose, Snapshot,
};
pub use world::PhysicsWorld;
