//! The player's sound preferences: what the Sound menu and keys change.

// Used by the window once it drives the sound director.
#![allow(dead_code)]

use crate::command::Command;

pub const VOLUME_STEP: i32 = 10;
pub const DEFAULT_VOLUME: i32 = 80;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundSettings {
    pub muted: bool,
    /// Percent, 0..=100.
    pub volume: i32,
    /// The theme's background loop.
    pub ambience: bool,
}

impl Default for SoundSettings {
    fn default() -> Self {
        Self {
            muted: false,
            volume: DEFAULT_VOLUME,
            ambience: true,
        }
    }
}

impl SoundSettings {
    /// Moves the volume by `steps * VOLUME_STEP`, clamped to 0..=100. Changing it unmutes.
    pub fn step_volume(&mut self, steps: i32) {
        self.volume = (self.volume + steps * VOLUME_STEP).clamp(0, 100);
        self.muted = false;
    }

    /// Master gain for the audio output; squared so equal steps sound about equally loud.
    #[allow(clippy::cast_precision_loss)] // 0..=100 is exact in f32
    pub fn gain(self) -> f32 {
        let v = self.volume.clamp(0, 100) as f32 / 100.0;
        v * v
    }

    /// The settings after a sound command; any other command leaves them unchanged.
    #[must_use]
    pub fn after(self, command: Command) -> Self {
        let mut next = self;
        match command {
            Command::ToggleMute => next.muted = !next.muted,
            Command::VolumeUp => next.step_volume(1),
            Command::VolumeDown => next.step_volume(-1),
            Command::ToggleAmbience => next.ambience = !next.ambience,
            Command::MoveAim(_)
            | Command::RotateAim(_)
            | Command::StartCharge
            | Command::Release
            | Command::NewGame
            | Command::CycleTheme
            | Command::Quit => {}
        }
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_audible_with_ambience() {
        let s = SoundSettings::default();
        assert!(!s.muted && s.ambience);
        assert_eq!(s.volume, 80);
    }

    #[test]
    fn step_volume_clamps_and_unmutes() {
        let mut s = SoundSettings {
            muted: true,
            ..SoundSettings::default()
        };
        s.step_volume(1);
        assert_eq!(s.volume, 90);
        assert!(!s.muted);
        s.step_volume(5);
        assert_eq!(s.volume, 100);
        s.step_volume(-20);
        assert_eq!(s.volume, 0);
    }

    #[test]
    fn gain_is_the_squared_fraction() {
        let mut s = SoundSettings::default();
        assert!((s.gain() - 0.64).abs() < 1e-6);
        s.volume = 100;
        assert!((s.gain() - 1.0).abs() < 1e-6);
        s.volume = 0;
        assert!(s.gain().abs() < 1e-6);
        s.volume = 250; // out of range is clamped
        assert!((s.gain() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn sound_commands_change_the_settings() {
        let s = SoundSettings::default();
        assert!(s.after(Command::ToggleMute).muted);
        assert!(
            !s.after(Command::ToggleMute)
                .after(Command::ToggleMute)
                .muted
        );
        assert_eq!(s.after(Command::VolumeUp).volume, 90);
        assert_eq!(s.after(Command::VolumeDown).volume, 70);
        assert!(!s.after(Command::ToggleAmbience).ambience);
        let muted = s.after(Command::ToggleMute);
        assert!(!muted.after(Command::VolumeDown).muted);
    }

    #[test]
    fn other_commands_leave_the_settings_alone() {
        let s = SoundSettings {
            muted: true,
            volume: 30,
            ambience: false,
        };
        for command in [
            Command::MoveAim(0.1),
            Command::RotateAim(1.0),
            Command::StartCharge,
            Command::Release,
            Command::NewGame,
            Command::CycleTheme,
            Command::Quit,
        ] {
            assert_eq!(s.after(command), s);
        }
    }
}
