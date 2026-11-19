//! Turns lane snapshots, the score sheet and physics impacts into sound cues.
//!
//! Pure logic: it only talks to the [`AudioOutput`] it is given on each call.

use bowlingbox_audio::{AudioOutput, LoopSlot, SoundId, Voice};
use bowlingbox_lane::{Body, Impact, LANE, Phase, Snapshot};
use bowlingbox_score::ScoreSheet;

use crate::hud_text::{ThrowResult, throw_result};

/// m/s: loudest, highest roll.
const ROLL_FULL_SPEED: f32 = 12.0;
/// m/s.
const ROLL_MIN_SPEED: f32 = 0.2;
/// Ball centre below this: in the pit.
const PIT_Y: f32 = -0.1;
/// Lane half width plus gutter.
const PAN_HALF_WIDTH: f32 = 0.765;
const GUTTER_PITCH: f32 = 0.85;
/// N·s, matches the physics world's record threshold.
const MIN_IMPULSE: f32 = 0.3;
const HIT_FULL_IMPULSE: f32 = 4.0;
const CLATTER_FULL_IMPULSE: f32 = 1.5;
const MAX_IMPACTS_PER_UPDATE: usize = 4;
/// Seconds, per kind.
const IMPACT_COOLDOWN: f32 = 0.03;

// Mix: the ambience files are normalised to -20 dBFS RMS, the rest to a -1 dBFS peak. The pin
// crash is the loudest moment; the ambience sits about 15 dB under the roll.
const AMBIENT_VOLUME: f32 = 0.18;
/// At full speed.
const ROLL_VOLUME: f32 = 0.35;
const CHARGE_VOLUME: f32 = 0.3;
const RELEASE_VOLUME: f32 = 0.8;
const GUTTER_VOLUME: f32 = 0.7;
const STINGER_VOLUME: f32 = 0.6;
const CHIME_VOLUME: f32 = 0.4;

const CLATTER_PITCHES: [f32; 5] = [1.0, 0.96, 1.04, 0.98, 1.02];

fn pan(x: f32) -> f32 {
    (x / PAN_HALF_WIDTH).clamp(-1.0, 1.0)
}

fn voice(volume: f32, pitch: f32, pan: f32) -> Voice {
    Voice { volume, pitch, pan }
}

/// Strike and spare get a stinger; a gutter ball or a plain count does not.
fn stinger_for(s: &Snapshot, sheet: &dyn ScoreSheet) -> Option<SoundId> {
    match throw_result(s, sheet) {
        ThrowResult::Strike => Some(SoundId::Strike),
        ThrowResult::Spare => Some(SoundId::Spare),
        ThrowResult::Gutter | ThrowResult::Pins | ThrowResult::None => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ImpactKind {
    Hit,
    Clatter,
}

fn kind_of(i: &Impact) -> Option<ImpactKind> {
    if i.a == Body::Other || i.b == Body::Other || i.impulse < MIN_IMPULSE {
        return None;
    }
    let ball_pin = matches!(
        (i.a, i.b),
        (Body::Ball, Body::Pin) | (Body::Pin, Body::Ball)
    );
    if ball_pin {
        Some(ImpactKind::Hit)
    } else if i.a == Body::Pin || i.b == Body::Pin {
        Some(ImpactKind::Clatter)
    } else {
        None
    }
}

pub struct SoundDirector {
    prev: Option<Snapshot>,
    /// Unset until the first `set_ambient`.
    ambient: Option<SoundId>,
    ambience: bool,
    ball_speed: f32,
    gutter_played: bool,
    since_hit: f32,
    since_clatter: f32,
    clatter_index: usize,
}

impl SoundDirector {
    pub fn new() -> Self {
        Self {
            prev: None,
            ambient: None,
            ambience: true,
            ball_speed: 0.0,
            gutter_played: false,
            since_hit: IMPACT_COOLDOWN,
            since_clatter: IMPACT_COOLDOWN,
            clatter_index: 0,
        }
    }

    /// Call once per frame after stepping the lane; `dt` is the simulated time of the frame.
    pub fn update(
        &mut self,
        out: &mut dyn AudioOutput,
        s: &Snapshot,
        sheet: &dyn ScoreSheet,
        impacts: &[Impact],
        dt: f32,
    ) {
        self.since_hit += dt;
        self.since_clatter += dt;
        let Some(prev) = self.prev.take() else {
            self.prev = Some(s.clone()); // nothing to compare with yet
            return;
        };
        if dt > 0.0 {
            let (a, b) = (prev.ball.position, s.ball.position);
            let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            self.ball_speed = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() / dt;
        }
        self.play_transitions(out, &prev, s, sheet);
        Self::update_charge(out, s);
        self.update_roll(out, s);
        self.play_impacts(out, impacts);
        self.prev = Some(s.clone());
    }

    pub fn on_new_game(&mut self, out: &mut dyn AudioOutput) {
        out.play(SoundId::NewGame, voice(CHIME_VOLUME, 1.0, 0.0));
        out.set_loop(LoopSlot::Roll, None);
        out.set_loop(LoopSlot::Charge, None);
        self.prev = None;
        self.gutter_played = false;
    }

    /// Plays `ambient` as the background loop (if ambience is on); `announce` adds the theme chime.
    pub fn set_ambient(&mut self, out: &mut dyn AudioOutput, ambient: SoundId, announce: bool) {
        self.ambient = Some(ambient);
        self.update_ambience(out);
        if announce {
            out.play(SoundId::ThemeSwitch, voice(CHIME_VOLUME, 1.0, 0.0));
        }
    }

    /// Starts or stops the theme's background loop; the other sounds are unaffected.
    pub fn set_ambience_enabled(&mut self, out: &mut dyn AudioOutput, enabled: bool) {
        self.ambience = enabled;
        self.update_ambience(out);
    }

    #[cfg(test)]
    pub fn ambience_enabled(&self) -> bool {
        self.ambience
    }

    fn update_ambience(&self, out: &mut dyn AudioOutput) {
        match self.ambient {
            Some(ambient) if self.ambience => out.set_loop(
                LoopSlot::Ambient,
                Some((ambient, voice(AMBIENT_VOLUME, 1.0, 0.0))),
            ),
            _ => out.set_loop(LoopSlot::Ambient, None),
        }
    }

    fn play_transitions(
        &mut self,
        out: &mut dyn AudioOutput,
        prev: &Snapshot,
        s: &Snapshot,
        sheet: &dyn ScoreSheet,
    ) {
        // Not just Charging -> Rolling: charge and release can both land inside one frame.
        if !matches!(prev.phase, Phase::Rolling | Phase::Settling) && s.phase == Phase::Rolling {
            self.gutter_played = false;
            out.play(
                SoundId::Release,
                voice(RELEASE_VOLUME, 1.0, pan(s.ball.position[0])),
            );
        }
        if prev.phase != Phase::FrameDone
            && s.phase == Phase::FrameDone
            && let Some(id) = stinger_for(s, sheet)
        {
            out.play(id, voice(STINGER_VOLUME, 1.0, 0.0));
        }
    }

    fn update_charge(out: &mut dyn AudioOutput, s: &Snapshot) {
        let tone = (s.phase == Phase::Charging).then(|| {
            (
                SoundId::ChargeTone,
                voice(CHARGE_VOLUME, 0.8 + 0.6 * s.power, 0.0),
            )
        });
        out.set_loop(LoopSlot::Charge, tone);
    }

    fn update_roll(&mut self, out: &mut dyn AudioOutput, s: &Snapshot) {
        let p = s.ball.position;
        let in_play = matches!(s.phase, Phase::Rolling | Phase::Settling);
        let in_gutter = p[0].abs() > LANE.half_width;
        if in_gutter && s.phase == Phase::Rolling && !self.gutter_played {
            self.gutter_played = true;
            out.play(SoundId::GutterDrop, voice(GUTTER_VOLUME, 1.0, pan(p[0])));
        }
        if !in_play || p[1] <= PIT_Y || self.ball_speed <= ROLL_MIN_SPEED {
            out.set_loop(LoopSlot::Roll, None);
            return;
        }
        let t = (self.ball_speed / ROLL_FULL_SPEED).clamp(0.0, 1.0);
        let mut pitch = 0.8 + 0.4 * t;
        if in_gutter {
            pitch *= GUTTER_PITCH;
        }
        out.set_loop(
            LoopSlot::Roll,
            Some((
                SoundId::Roll,
                voice(ROLL_VOLUME * (0.25 + 0.75 * t), pitch, pan(p[0])),
            )),
        );
    }

    fn play_impacts(&mut self, out: &mut dyn AudioOutput, impacts: &[Impact]) {
        let mut loud: Vec<(&Impact, ImpactKind)> = impacts
            .iter()
            .filter_map(|i| kind_of(i).map(|kind| (i, kind)))
            .collect();
        // Stable: equal impulses keep their order.
        loud.sort_by(|x, y| y.0.impulse.total_cmp(&x.0.impulse));

        let hit_ready = self.since_hit >= IMPACT_COOLDOWN;
        let clatter_ready = self.since_clatter >= IMPACT_COOLDOWN;
        let (mut hit_played, mut clatter_played) = (false, false);
        let mut played = 0;
        for (i, kind) in loud {
            if played == MAX_IMPACTS_PER_UPDATE {
                break;
            }
            let x = pan(i.position[0]);
            match kind {
                ImpactKind::Hit => {
                    if !hit_ready {
                        continue;
                    }
                    let volume = (i.impulse / HIT_FULL_IMPULSE).clamp(0.15, 1.0);
                    out.play(SoundId::PinHit, voice(volume, 1.0, x));
                    hit_played = true;
                }
                ImpactKind::Clatter => {
                    if !clatter_ready {
                        continue;
                    }
                    let pitch = CLATTER_PITCHES[self.clatter_index];
                    self.clatter_index = (self.clatter_index + 1) % CLATTER_PITCHES.len();
                    let volume = (i.impulse / CLATTER_FULL_IMPULSE).clamp(0.1, 0.8);
                    out.play(SoundId::PinClatter, voice(volume, pitch, x));
                    clatter_played = true;
                }
            }
            played += 1;
        }
        if hit_played {
            self.since_hit = 0.0;
        }
        if clatter_played {
            self.since_clatter = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording_output::RecordingOutput;
    use bowlingbox_lane::Pose;
    use bowlingbox_score::{Game, ScoreCalculator};

    const DT: f32 = 1.0 / 60.0;

    fn at(phase: Phase, x: f32, z: f32, y: f32) -> Snapshot {
        Snapshot {
            phase,
            ball: Pose {
                position: [x, y, z],
                ..Pose::default()
            },
            pins: [Pose::default(); 10],
            pin_standing: [true; 10],
            pin_in_play: [true; 10],
            aim_offset: 0.0,
            aim_angle: 0.0,
            power: 0.0,
            last_pins_down: None,
        }
    }

    fn snap(phase: Phase) -> Snapshot {
        at(phase, 0.0, 0.0, 0.109)
    }

    fn snap_at(phase: Phase, x: f32, z: f32) -> Snapshot {
        at(phase, x, z, 0.109)
    }

    fn impact(a: Body, b: Body, impulse: f32, x: f32) -> Impact {
        Impact {
            a,
            b,
            impulse,
            position: [x, 0.0, 17.5],
        }
    }

    struct Fixture {
        out: RecordingOutput,
        director: SoundDirector,
        sheet: ScoreCalculator,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                out: RecordingOutput::new(),
                director: SoundDirector::new(),
                sheet: ScoreCalculator::default(),
            }
        }

        fn update(&mut self, s: &Snapshot) {
            self.update_with(s, &[]);
        }

        fn update_with(&mut self, s: &Snapshot, impacts: &[Impact]) {
            self.director
                .update(&mut self.out, s, &self.sheet, impacts, DT);
        }
    }

    fn near(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-4, "{a} != {b}");
    }

    #[test]
    fn first_update_only_records_the_snapshot() {
        let mut f = Fixture::new();
        f.update_with(
            &snap_at(Phase::Rolling, 0.0, 5.0),
            &[impact(Body::Ball, Body::Pin, 3.0, 0.0)],
        );
        assert_eq!(f.out.played_count(), 0);
        assert_eq!(f.out.loop_sound(LoopSlot::Roll), None);
    }

    #[test]
    fn release_plays_once_when_charging_turns_into_rolling() {
        let mut f = Fixture::new();
        f.update(&snap(Phase::Charging));
        f.update(&snap_at(Phase::Rolling, 0.0, 0.1));
        f.update(&snap_at(Phase::Rolling, 0.0, 0.2));
        assert_eq!(f.out.count(SoundId::Release), 1);
    }

    // Charge and release inside one frame: the director never sees Charging.
    #[test]
    fn aiming_straight_into_rolling_releases_and_rearms_the_gutter_drop() {
        let mut f = Fixture::new();
        f.update(&snap(Phase::Aiming));
        f.update(&snap_at(Phase::Rolling, 0.6, 0.1));
        f.update(&snap_at(Phase::Rolling, 0.6, 0.2));
        assert_eq!(f.out.count(SoundId::Release), 1);
        assert_eq!(f.out.count(SoundId::GutterDrop), 1);
        f.update(&snap_at(Phase::FrameDone, 0.6, 18.0));
        f.update(&snap(Phase::Aiming));
        f.update(&snap_at(Phase::Rolling, 0.6, 0.1));
        assert_eq!(f.out.count(SoundId::Release), 2);
        assert_eq!(f.out.count(SoundId::GutterDrop), 2);
    }

    #[test]
    fn charge_loop_pitch_follows_power_and_stops_after_release() {
        let mut f = Fixture::new();
        f.update(&snap(Phase::Aiming));
        let mut s = snap(Phase::Charging);
        s.power = 0.5;
        f.update(&s);
        assert_eq!(
            f.out.loop_sound(LoopSlot::Charge),
            Some(SoundId::ChargeTone)
        );
        near(f.out.loop_voice(LoopSlot::Charge).pitch, 1.1);
        near(f.out.loop_voice(LoopSlot::Charge).volume, CHARGE_VOLUME);
        f.update(&snap_at(Phase::Rolling, 0.0, 0.1));
        assert_eq!(f.out.loop_sound(LoopSlot::Charge), None);
    }

    #[test]
    fn roll_loop_volume_pitch_and_pan_follow_the_ball() {
        let mut f = Fixture::new();
        f.update(&snap_at(Phase::Rolling, 0.3825, 0.0));
        f.update(&snap_at(Phase::Rolling, 0.3825, 0.1)); // 0.1 m in 1/60 s = 6 m/s
        assert_eq!(f.out.loop_sound(LoopSlot::Roll), Some(SoundId::Roll));
        let v = f.out.loop_voice(LoopSlot::Roll);
        near(v.volume, ROLL_VOLUME * 0.625); // 0.25 + 0.75 * 0.5
        near(v.pitch, 1.0); // 0.8 + 0.4 * 0.5
        near(v.pan, 0.5); // 0.3825 / 0.765: +x is the bowler's right, the right speaker
    }

    #[test]
    fn a_ball_on_the_left_pans_left() {
        let mut f = Fixture::new();
        f.update(&snap_at(Phase::Rolling, -0.3825, 0.0));
        f.update(&snap_at(Phase::Rolling, -0.3825, 0.1));
        near(f.out.loop_voice(LoopSlot::Roll).pan, -0.5);
    }

    #[test]
    fn gutter_lowers_the_roll_pitch_and_drops_once() {
        let mut f = Fixture::new();
        f.update(&snap_at(Phase::Rolling, 0.6, 0.0));
        f.update(&snap_at(Phase::Rolling, 0.6, 0.1));
        f.update(&snap_at(Phase::Rolling, 0.6, 0.2));
        near(f.out.loop_voice(LoopSlot::Roll).pitch, 0.85);
        assert_eq!(f.out.count(SoundId::GutterDrop), 1);
    }

    #[test]
    fn roll_stops_in_the_pit_when_slow_and_outside_rolling() {
        let mut f = Fixture::new();
        f.update(&snap_at(Phase::Rolling, 0.0, 0.0));
        f.update(&snap_at(Phase::Rolling, 0.0, 0.1));
        assert!(f.out.loop_sound(LoopSlot::Roll).is_some());
        f.update(&at(Phase::Settling, 0.0, 18.5, -0.5)); // in the pit
        assert_eq!(f.out.loop_sound(LoopSlot::Roll), None);
        f.update(&snap_at(Phase::Settling, 0.0, 18.0));
        f.update(&snap_at(Phase::Settling, 0.0, 18.0)); // on the lane but not moving
        assert_eq!(f.out.loop_sound(LoopSlot::Roll), None);
        f.update(&snap_at(Phase::Aiming, 0.0, 5.0)); // teleport while aiming: never a roll
        assert_eq!(f.out.loop_sound(LoopSlot::Roll), None);
    }

    #[test]
    fn impacts_map_to_hit_and_clatter_with_scaled_volume_and_pan() {
        let mut f = Fixture::new();
        f.update(&snap(Phase::Rolling));
        f.update_with(
            &snap(Phase::Rolling),
            &[
                impact(Body::Pin, Body::Ball, 2.0, 0.3825), // order does not matter
                impact(Body::Pin, Body::Lane, 0.75, -0.3825),
                impact(Body::Pin, Body::Pin, 0.2, 0.0), // below threshold
                impact(Body::Ball, Body::Lane, 5.0, 0.0), // no sound
                impact(Body::Pin, Body::Other, 5.0, 0.0), // unknown body
            ],
        );
        assert_eq!(f.out.played_count(), 2);
        assert_eq!(f.out.played[0].0, SoundId::PinHit);
        near(f.out.played[0].1.volume, 0.5);
        near(f.out.played[0].1.pan, 0.5);
        assert_eq!(f.out.played[1].0, SoundId::PinClatter);
        near(f.out.played[1].1.volume, 0.5);
        near(f.out.played[1].1.pan, -0.5);
    }

    #[test]
    fn at_most_four_impact_sounds_per_update_strongest_first() {
        let mut f = Fixture::new();
        f.update(&snap(Phase::Rolling));
        let burst: Vec<Impact> = [0.4, 0.9, 0.5, 1.2, 0.6, 0.7]
            .into_iter()
            .map(|impulse| impact(Body::Pin, Body::Pin, impulse, 0.0))
            .collect();
        f.update_with(&snap(Phase::Rolling), &burst);
        assert_eq!(f.out.played_count(), 4);
        near(f.out.played[0].1.volume, 0.8); // 1.2 / 1.5 capped at 0.8
        near(f.out.played[3].1.volume, 0.6 / 1.5);
        near(f.out.played[0].1.pitch, 1.0);
        near(f.out.played[1].1.pitch, 0.96);
    }

    #[test]
    fn the_same_kind_waits_for_its_cooldown() {
        let mut f = Fixture::new();
        let clatter = [impact(Body::Pin, Body::Pin, 1.0, 0.0)];
        f.update(&snap(Phase::Rolling));
        f.update_with(&snap(Phase::Rolling), &clatter); // plays
        f.update_with(&snap(Phase::Rolling), &clatter); // 16 ms later: too soon
        f.update_with(&snap(Phase::Rolling), &clatter); // 33 ms after the first: plays
        assert_eq!(f.out.count(SoundId::PinClatter), 2);
        // The other kind is independent.
        f.update_with(
            &snap(Phase::Rolling),
            &[impact(Body::Ball, Body::Pin, 1.0, 0.0)],
        );
        assert_eq!(f.out.count(SoundId::PinHit), 1);
    }

    #[test]
    fn a_big_crash_in_one_frame_is_capped_and_then_cooled_down() {
        let mut f = Fixture::new();
        f.update(&snap(Phase::Rolling));
        let crash: Vec<Impact> = (0..48)
            .map(|n| {
                let kind = if n % 3 == 0 { Body::Ball } else { Body::Pin };
                impact(
                    kind,
                    Body::Pin,
                    1.0 + f32::from(u8::try_from(n).unwrap()) * 0.05,
                    0.0,
                )
            })
            .collect();
        f.update_with(&snap(Phase::Rolling), &crash);
        assert_eq!(f.out.played_count(), 4);
        // The next frame is 16 ms later: both kinds are still cooling down.
        f.update_with(&snap(Phase::Rolling), &crash);
        assert_eq!(f.out.played_count(), 4);
        f.update_with(&snap(Phase::Rolling), &crash);
        assert_eq!(f.out.played_count(), 8);
    }

    #[test]
    fn strike_stinger_plays_once_on_entering_frame_done() {
        let mut f = Fixture::new();
        f.update(&snap(Phase::Settling));
        f.sheet.roll(10).unwrap();
        let mut s = snap(Phase::FrameDone);
        s.last_pins_down = Some(10);
        f.update(&s);
        f.update(&s);
        assert_eq!(f.out.count(SoundId::Strike), 1);
    }

    #[test]
    fn ten_after_a_gutter_ball_plays_the_spare_stinger() {
        let mut f = Fixture::new();
        f.update(&snap(Phase::Settling));
        f.sheet.roll(0).unwrap();
        let mut s = snap(Phase::FrameDone);
        s.last_pins_down = Some(0);
        f.update(&s);
        assert_eq!(f.out.played_count(), 0); // a gutter ball has no stinger
        f.update(&snap(Phase::Settling));
        f.sheet.roll(10).unwrap();
        s.last_pins_down = Some(10);
        f.update(&s);
        assert_eq!(f.out.count(SoundId::Spare), 1);
        assert_eq!(f.out.count(SoundId::Strike), 0);
    }

    #[test]
    fn a_plain_count_plays_no_stinger() {
        let mut f = Fixture::new();
        f.update(&snap(Phase::Settling));
        f.sheet.roll(7).unwrap();
        let mut s = snap(Phase::FrameDone);
        s.last_pins_down = Some(7);
        f.update(&s);
        assert_eq!(f.out.played_count(), 0);
    }

    #[test]
    fn a_gutter_ball_drops_once_with_a_low_roll_and_no_pin_sound_or_stinger() {
        let mut f = Fixture::new();
        f.update(&snap(Phase::Aiming));
        let mut z = 0.1;
        f.update(&snap_at(Phase::Rolling, 0.62, z));
        while z < 17.0 {
            z += 0.1;
            f.update(&snap_at(Phase::Rolling, 0.62, z));
        }
        near(
            f.out.loop_voice(LoopSlot::Roll).pitch,
            (0.8 + 0.4 * 0.5) * GUTTER_PITCH,
        );
        f.update(&at(Phase::Settling, 0.62, 18.5, -0.5));
        f.sheet.roll(0).unwrap();
        let mut done = snap_at(Phase::FrameDone, 0.62, 18.5);
        done.last_pins_down = Some(0);
        f.update(&done);
        assert_eq!(f.out.count(SoundId::Release), 1);
        assert_eq!(f.out.count(SoundId::GutterDrop), 1);
        for pin_sound in [
            SoundId::PinHit,
            SoundId::PinClatter,
            SoundId::Strike,
            SoundId::Spare,
        ] {
            assert_eq!(f.out.count(pin_sound), 0, "{pin_sound:?}");
        }
        assert_eq!(f.out.loop_sound(LoopSlot::Roll), None);
    }

    #[test]
    fn ambient_sets_the_loop_and_announces_only_when_asked() {
        let mut f = Fixture::new();
        f.director
            .set_ambient(&mut f.out, SoundId::AmbientClassic, false);
        assert_eq!(
            f.out.loop_sound(LoopSlot::Ambient),
            Some(SoundId::AmbientClassic)
        );
        near(f.out.loop_voice(LoopSlot::Ambient).volume, AMBIENT_VOLUME);
        assert_eq!(f.out.count(SoundId::ThemeSwitch), 0);
        f.director
            .set_ambient(&mut f.out, SoundId::AmbientOutdoor, true);
        assert_eq!(
            f.out.loop_sound(LoopSlot::Ambient),
            Some(SoundId::AmbientOutdoor)
        );
        assert_eq!(f.out.count(SoundId::ThemeSwitch), 1);
        f.director
            .set_ambient(&mut f.out, SoundId::AmbientNeon, true);
        assert_eq!(
            f.out.loop_sound(LoopSlot::Ambient),
            Some(SoundId::AmbientNeon)
        );
    }

    #[test]
    fn a_theme_switch_during_a_roll_changes_only_the_ambience() {
        let mut f = Fixture::new();
        f.director
            .set_ambient(&mut f.out, SoundId::AmbientNeon, false);
        f.update(&snap_at(Phase::Rolling, 0.0, 0.0));
        f.update(&snap_at(Phase::Rolling, 0.0, 0.1));
        let roll = f.out.loop_voice(LoopSlot::Roll);
        f.director
            .set_ambient(&mut f.out, SoundId::AmbientOutdoor, true);
        assert_eq!(
            f.out.loop_sound(LoopSlot::Ambient),
            Some(SoundId::AmbientOutdoor)
        );
        assert_eq!(f.out.count(SoundId::ThemeSwitch), 1);
        assert_eq!(f.out.loop_sound(LoopSlot::Roll), Some(SoundId::Roll));
        assert_eq!(f.out.loop_voice(LoopSlot::Roll), roll);
    }

    #[test]
    fn a_theme_switch_with_ambience_off_starts_no_ambience_but_still_chimes() {
        let mut f = Fixture::new();
        f.director
            .set_ambient(&mut f.out, SoundId::AmbientNeon, false);
        f.director.set_ambience_enabled(&mut f.out, false);
        f.update(&snap_at(Phase::Rolling, 0.0, 0.0));
        f.update(&snap_at(Phase::Rolling, 0.0, 0.1));
        f.director
            .set_ambient(&mut f.out, SoundId::AmbientClassic, true);
        assert_eq!(f.out.loop_sound(LoopSlot::Ambient), None);
        assert_eq!(f.out.count(SoundId::ThemeSwitch), 1);
        assert_eq!(f.out.loop_sound(LoopSlot::Roll), Some(SoundId::Roll));
    }

    #[test]
    fn new_game_chimes_and_stops_roll_and_charge_but_not_the_ambience() {
        let mut f = Fixture::new();
        f.director
            .set_ambient(&mut f.out, SoundId::AmbientNeon, false);
        f.update(&snap_at(Phase::Rolling, 0.0, 0.0));
        f.update(&snap_at(Phase::Rolling, 0.0, 0.1));
        assert!(f.out.loop_sound(LoopSlot::Roll).is_some());
        f.director.on_new_game(&mut f.out);
        assert_eq!(f.out.count(SoundId::NewGame), 1);
        assert_eq!(f.out.loop_sound(LoopSlot::Roll), None);
        assert_eq!(f.out.loop_sound(LoopSlot::Charge), None);
        assert_eq!(
            f.out.loop_sound(LoopSlot::Ambient),
            Some(SoundId::AmbientNeon)
        );
    }

    #[test]
    fn ambience_toggle_stops_and_restores_the_theme_loop() {
        let mut f = Fixture::new();
        f.director
            .set_ambient(&mut f.out, SoundId::AmbientOutdoor, false);
        f.director.set_ambience_enabled(&mut f.out, false);
        assert!(!f.director.ambience_enabled());
        assert_eq!(f.out.loop_sound(LoopSlot::Ambient), None);
        // Switching theme keeps it off.
        f.director
            .set_ambient(&mut f.out, SoundId::AmbientClassic, true);
        assert_eq!(f.out.loop_sound(LoopSlot::Ambient), None);
        assert_eq!(f.out.count(SoundId::ThemeSwitch), 1);
        f.director.set_ambience_enabled(&mut f.out, true);
        assert_eq!(
            f.out.loop_sound(LoopSlot::Ambient),
            Some(SoundId::AmbientClassic)
        );
    }

    #[test]
    fn ambience_stays_silent_until_a_theme_is_set() {
        let mut f = Fixture::new();
        f.director.set_ambience_enabled(&mut f.out, true);
        assert_eq!(f.out.loop_sound(LoopSlot::Ambient), None);
    }
}
