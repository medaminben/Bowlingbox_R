//! The output trait and the choice between the device and silence.

use crate::{KiraOutput, SilentOutput, SoundId};

/// Each slot plays at most one looping sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoopSlot {
    Roll,
    Charge,
    Ambient,
}

impl LoopSlot {
    pub const ALL: [LoopSlot; 3] = [LoopSlot::Roll, LoopSlot::Charge, LoopSlot::Ambient];
}

/// How one sound is played.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Voice {
    /// 0..1.
    pub volume: f32,
    /// Playback rate, above 0.
    pub pitch: f32,
    /// -1 left .. +1 right.
    pub pan: f32,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            volume: 1.0,
            pitch: 1.0,
            pan: 0.0,
        }
    }
}

pub trait AudioOutput {
    fn play(&mut self, sound: SoundId, voice: Voice);
    /// `None` stops the slot; the same sound updates the voice without restarting;
    /// another sound restarts the slot with that sound.
    fn set_loop(&mut self, slot: LoopSlot, sound: Option<(SoundId, Voice)>);
    fn set_muted(&mut self, muted: bool);
    fn muted(&self) -> bool;
    /// Master gain in 0..1 for everything; mute silences on top of it.
    fn set_volume(&mut self, volume: f32);
    fn volume(&self) -> f32;
}

/// The default device, or silence with the warning to show when there is none.
#[must_use]
pub fn open_output() -> (Box<dyn AudioOutput>, Option<String>) {
    open_output_with(|| KiraOutput::new().map(|o| Box::new(o) as Box<dyn AudioOutput>))
}

/// [`open_output`] with the device opening replaced, so tests need no device.
#[must_use]
pub fn open_output_with(
    factory: impl FnOnce() -> Result<Box<dyn AudioOutput>, String>,
) -> (Box<dyn AudioOutput>, Option<String>) {
    match factory() {
        Ok(output) => (output, None),
        Err(reason) => (
            Box::new(SilentOutput::default()),
            Some(format!("sound disabled: {reason}")),
        ),
    }
}
