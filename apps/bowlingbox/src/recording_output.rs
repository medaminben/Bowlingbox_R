//! An [`AudioOutput`] that records what it is asked to play, for tests. No device is opened.

use bowlingbox_audio::{AudioOutput, LoopSlot, SoundId, Voice};

#[derive(Default)]
pub struct RecordingOutput {
    pub played: Vec<(SoundId, Voice)>,
    loops: [Option<(SoundId, Voice)>; 3],
    muted: bool,
    volume: f32,
}

impl RecordingOutput {
    pub fn new() -> Self {
        Self {
            volume: 1.0,
            ..Self::default()
        }
    }

    pub fn played_count(&self) -> usize {
        self.played.len()
    }

    pub fn count(&self, id: SoundId) -> usize {
        self.played
            .iter()
            .filter(|(played, _)| *played == id)
            .count()
    }

    /// The sound a slot loops now, if any.
    pub fn loop_sound(&self, slot: LoopSlot) -> Option<SoundId> {
        self.loops[slot as usize].map(|(id, _)| id)
    }

    /// The voice a slot was last set to; the default voice while the slot is stopped.
    pub fn loop_voice(&self, slot: LoopSlot) -> Voice {
        self.loops[slot as usize].map_or_else(Voice::default, |(_, voice)| voice)
    }
}

impl AudioOutput for RecordingOutput {
    fn play(&mut self, sound: SoundId, voice: Voice) {
        self.played.push((sound, voice));
    }
    fn set_loop(&mut self, slot: LoopSlot, sound: Option<(SoundId, Voice)>) {
        self.loops[slot as usize] = sound;
    }
    fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }
    fn muted(&self) -> bool {
        self.muted
    }
    fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
    }
    fn volume(&self) -> f32 {
        self.volume
    }
}
