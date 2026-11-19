use crate::{AudioOutput, LoopSlot, SoundId, Voice};

/// Plays nothing, but remembers the mute, volume and loop state.
#[derive(Debug, Clone, PartialEq)]
pub struct SilentOutput {
    muted: bool,
    volume: f32,
    loops: [Option<(SoundId, Voice)>; 3],
    loop_starts: [u32; 3],
}

impl Default for SilentOutput {
    fn default() -> Self {
        Self {
            muted: false,
            volume: 1.0,
            loops: [None; 3],
            loop_starts: [0; 3],
        }
    }
}

impl SilentOutput {
    /// The sound and voice the slot would be playing.
    #[must_use]
    pub fn loop_state(&self, slot: LoopSlot) -> Option<(SoundId, Voice)> {
        self.loops[slot as usize]
    }

    /// How often the slot would have started a sound from the beginning.
    #[must_use]
    pub fn loop_starts(&self, slot: LoopSlot) -> u32 {
        self.loop_starts[slot as usize]
    }
}

impl AudioOutput for SilentOutput {
    fn play(&mut self, _sound: SoundId, _voice: Voice) {}

    fn set_loop(&mut self, slot: LoopSlot, sound: Option<(SoundId, Voice)>) {
        let i = slot as usize;
        if let Some((id, _)) = sound
            && self.loops[i].map(|(current, _)| current) != Some(id)
        {
            self.loop_starts[i] += 1;
        }
        self.loops[i] = sound;
    }

    fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    fn muted(&self) -> bool {
        self.muted
    }

    fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    fn volume(&self) -> f32 {
        self.volume
    }
}
