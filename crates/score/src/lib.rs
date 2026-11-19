//! The ten-pin score calculator of Bowlingbox, a port of the C++ `Score` library.
//!
//! [`ScoreCalculator`] records balls and implements [`Game`] and [`ScoreSheet`]. Displays
//! depend on [`ScoreSheet`] and use [`notation`] and [`frames_of`] to draw the sheet.

mod frames;
mod game;
mod slot;

pub use frames::{FrameView, frames, frames_of};
pub use game::{Game, RollError, ScoreCalculator, ScoreSheet};
pub use slot::{Slot, notation, total_notation};

/// Frames in a game.
pub const FRAMES: usize = 10;
/// Pins in a full rack.
pub const PINS: u8 = 10;
/// Roll slots of a score sheet: nine frames of two and three for the last frame.
pub const ROLL_SLOTS: usize = 21;
