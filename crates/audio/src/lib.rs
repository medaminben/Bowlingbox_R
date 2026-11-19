//! The audio of Bowlingbox: the 13 embedded sounds, the [`AudioOutput`] trait,
//! a kira output that plays through the default device, and a silent fallback.
//!
//! This crate depends on none of the game crates; the app decides which sound to cue.

mod catalogue;
mod kira_output;
mod output;
mod silent;

pub use catalogue::{
    ALL_SOUNDS, SoundId, SoundInfo, decode_info, embedded_data, file_name, is_loop,
};
pub use kira_output::KiraOutput;
pub use output::{AudioOutput, LoopSlot, Voice, open_output, open_output_with};
pub use silent::SilentOutput;
