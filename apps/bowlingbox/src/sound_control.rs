//! The window's sound side: the settings, the director and what changes when either moves.
//! The output stays with the caller, so tests can use a recording one.

use bowlingbox_audio::AudioOutput;
use bowlingbox_lane::{Lane, PhysicsWorld, Snapshot};
use bowlingbox_render::ThemeId;
use bowlingbox_score::Game;

use crate::command::Command;
use crate::fixed_step::DT;
use crate::hud_text;
use crate::sound_director::SoundDirector;
use crate::sound_settings::SoundSettings;
use crate::theme_sounds::ambient_for;

pub struct SoundControl {
    settings: SoundSettings,
    director: SoundDirector,
}

impl SoundControl {
    /// Applies the start settings to `out` and starts the theme's ambience without a chime.
    pub fn new(out: &mut dyn AudioOutput, muted: bool, theme: ThemeId) -> Self {
        let mut control = Self {
            settings: SoundSettings {
                muted,
                ..SoundSettings::default()
            },
            director: SoundDirector::new(),
        };
        control.apply_settings(out);
        control.director.set_ambient(out, ambient_for(theme), false);
        control
    }

    pub fn settings(&self) -> SoundSettings {
        self.settings
    }

    /// Runs a sound command; any other command changes nothing.
    pub fn apply(&mut self, out: &mut dyn AudioOutput, command: Command) {
        self.settings = self.settings.after(command);
        self.apply_settings(out);
    }

    /// The theme changed: its ambience takes over, with the chime.
    pub fn theme_changed(&mut self, out: &mut dyn AudioOutput, theme: ThemeId) {
        self.director.set_ambient(out, ambient_for(theme), true);
    }

    pub fn new_game(&mut self, out: &mut dyn AudioOutput) {
        self.director.on_new_game(out);
    }

    /// Runs `steps` fixed steps, then drains the impacts of the frame, also while muted so no
    /// backlog builds up, and lets the director react. Returns the snapshot after the steps.
    pub fn step_frame<W: PhysicsWorld, G: Game>(
        &mut self,
        out: &mut dyn AudioOutput,
        lane: &mut Lane<W, G>,
        steps: u32,
    ) -> Snapshot {
        for _ in 0..steps {
            lane.step(DT);
        }
        let impacts = lane.drain_impacts();
        let snapshot = lane.snapshot();
        #[allow(clippy::cast_precision_loss)] // at most a handful of steps
        let dt = steps as f32 * DT;
        self.director
            .update(out, &snapshot, lane.score_sheet(), &impacts, dt);
        snapshot
    }

    /// `volume NN %` or `sound muted`, plus `, no ambience`.
    pub fn status(&self) -> String {
        hud_text::sound_status(self.settings)
    }

    fn apply_settings(&mut self, out: &mut dyn AudioOutput) {
        out.set_muted(self.settings.muted);
        out.set_volume(self.settings.gain());
        self.director
            .set_ambience_enabled(out, self.settings.ambience);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording_output::RecordingOutput;
    use bowlingbox_audio::{LoopSlot, SilentOutput, SoundId};
    use bowlingbox_lane::{Body, Impact, Pose};
    use bowlingbox_score::ScoreCalculator;

    /// A world that records one ball-pin impact on every step and rolls the ball down the lane.
    #[derive(Default)]
    struct ImpactWorld {
        pending: Vec<Impact>,
        steps: u16,
    }

    impl PhysicsWorld for ImpactWorld {
        fn step(&mut self, _dt: f32) {
            self.steps += 1;
            self.pending.push(Impact {
                a: Body::Ball,
                b: Body::Pin,
                impulse: 3.0,
                position: [0.0, 0.0, 17.5],
            });
        }
        fn place_ball(&mut self, _offset: f32) {}
        fn launch_ball(&mut self, _offset: f32, _angle_deg: f32, _speed: f32) {}
        fn reset_rack(&mut self) {}
        fn rebuild(&mut self) {
            self.pending.clear();
        }
        fn remove_pin(&mut self, _i: usize) {}
        fn ball_pose(&self) -> Pose {
            Pose {
                position: [0.0, 0.1 * f32::from(self.steps), 0.0],
                ..Pose::default()
            }
        }
        fn pin_pose(&self, _i: usize) -> Pose {
            Pose::default()
        }
        fn is_pin_standing(&self, _i: usize) -> bool {
            true
        }
        fn is_pin_in_play(&self, _i: usize) -> bool {
            true
        }
        fn ball_in_pit(&self) -> bool {
            false
        }
        fn ball_speed(&self) -> f32 {
            0.0
        }
        fn all_slow(&self) -> bool {
            false
        }
        fn drain_impacts(&mut self) -> Vec<Impact> {
            std::mem::take(&mut self.pending)
        }
    }

    /// A lane whose world adds an impact on every step, thrown so that it steps the world.
    fn rolling_lane() -> Lane<ImpactWorld, ScoreCalculator> {
        let mut lane = Lane::new(ImpactWorld::default(), ScoreCalculator::default());
        lane.start_charge().unwrap();
        lane.release().unwrap();
        assert_eq!(lane.snapshot().phase, bowlingbox_lane::Phase::Rolling);
        lane
    }

    #[test]
    fn mute_flag_starts_muted() {
        let mut out = SilentOutput::default();
        let control = SoundControl::new(&mut out, true, ThemeId::NeonHall);
        assert!(control.settings().muted);
        assert!(out.muted());
        assert_eq!(control.status(), "sound muted");
    }

    #[test]
    fn start_applies_defaults_and_starts_ambience_without_a_chime() {
        let mut out = RecordingOutput::new();
        let control = SoundControl::new(&mut out, false, ThemeId::ClassicAlley);
        assert!(!out.muted());
        assert!((out.volume() - 0.64).abs() < 1e-6);
        assert_eq!(
            out.loop_sound(LoopSlot::Ambient),
            Some(SoundId::AmbientClassic)
        );
        assert_eq!(out.played_count(), 0);
        assert_eq!(control.status(), "volume 80 %");
    }

    #[test]
    fn sound_commands_update_settings_and_output() {
        let mut out = RecordingOutput::new();
        let mut control = SoundControl::new(&mut out, false, ThemeId::NeonHall);
        control.apply(&mut out, Command::ToggleMute);
        assert!(control.settings().muted && out.muted());
        // Changing the volume unmutes.
        control.apply(&mut out, Command::VolumeDown);
        assert!(!control.settings().muted && !out.muted());
        assert_eq!(control.settings().volume, 70);
        assert!((out.volume() - 0.49).abs() < 1e-6);
        assert_eq!(control.status(), "volume 70 %");
        control.apply(&mut out, Command::ToggleAmbience);
        assert!(!control.settings().ambience);
        assert_eq!(out.loop_sound(LoopSlot::Ambient), None);
        control.apply(&mut out, Command::ToggleAmbience);
        assert_eq!(
            out.loop_sound(LoopSlot::Ambient),
            Some(SoundId::AmbientNeon)
        );
    }

    #[test]
    fn muting_and_unmuting_keeps_the_ambience_and_roll_loops_running() {
        let mut out = SilentOutput::default();
        let mut control = SoundControl::new(&mut out, false, ThemeId::NeonHall);
        let mut lane = rolling_lane();
        control.step_frame(&mut out, &mut lane, 2);
        control.step_frame(&mut out, &mut lane, 2);
        let roll = out.loop_state(LoopSlot::Roll);
        assert!(roll.is_some(), "the roll loop should play");
        assert!(out.loop_state(LoopSlot::Ambient).is_some());
        assert_eq!(out.loop_starts(LoopSlot::Ambient), 1);

        control.apply(&mut out, Command::ToggleMute);
        assert!(out.muted());
        control.apply(&mut out, Command::ToggleMute);

        assert!(!out.muted());
        assert_eq!(out.loop_starts(LoopSlot::Ambient), 1, "ambience restarted");
        assert_eq!(out.loop_state(LoopSlot::Roll), roll);
    }

    #[test]
    fn a_theme_change_announces_with_the_new_ambience() {
        let mut out = RecordingOutput::new();
        let mut control = SoundControl::new(&mut out, false, ThemeId::NeonHall);
        control.theme_changed(&mut out, ThemeId::Outdoor);
        assert_eq!(out.count(SoundId::ThemeSwitch), 1);
        assert_eq!(
            out.loop_sound(LoopSlot::Ambient),
            Some(SoundId::AmbientOutdoor)
        );
    }

    #[test]
    fn a_new_game_plays_the_chime() {
        let mut out = RecordingOutput::new();
        let mut control = SoundControl::new(&mut out, false, ThemeId::NeonHall);
        control.new_game(&mut out);
        assert_eq!(out.count(SoundId::NewGame), 1);
    }

    #[test]
    fn impacts_are_drained_every_frame_even_while_muted() {
        let mut out = RecordingOutput::new();
        let mut control = SoundControl::new(&mut out, true, ThemeId::NeonHall);
        let mut lane = rolling_lane();
        for _ in 0..3 {
            control.step_frame(&mut out, &mut lane, 4);
            assert_eq!(lane.drain_impacts().len(), 0, "impacts left in the lane");
        }
        // A frame with no steps leaves nothing behind either.
        control.step_frame(&mut out, &mut lane, 0);
        assert_eq!(lane.drain_impacts().len(), 0);
    }

    #[test]
    fn the_director_gets_the_summed_step_time() {
        let mut out = RecordingOutput::new();
        let mut control = SoundControl::new(&mut out, false, ThemeId::NeonHall);
        let mut lane = rolling_lane();
        // The first update only records the snapshot.
        control.step_frame(&mut out, &mut lane, 2);
        assert_eq!(out.count(SoundId::PinHit), 0);
        // One impact per step, all played: nothing has cooled down yet.
        control.step_frame(&mut out, &mut lane, 2);
        assert_eq!(out.count(SoundId::PinHit), 2);
        // 1/60 s is shorter than the 0.03 s cooldown ...
        control.step_frame(&mut out, &mut lane, 1);
        assert_eq!(out.count(SoundId::PinHit), 2);
        // ... two steps (1/30 s) added to it are not.
        control.step_frame(&mut out, &mut lane, 2);
        assert_eq!(out.count(SoundId::PinHit), 4);
    }
}
