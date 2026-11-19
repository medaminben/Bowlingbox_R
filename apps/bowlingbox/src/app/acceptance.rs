//! App-level smoke tests for the acceptance list in `docs/ACCEPTANCE.md`. Each test names its item.
//! Items 2 (the pocket strike), 5, 9 and 10 are not here: see that file.

use bowlingbox_audio::{AudioOutput, SilentOutput};
use bowlingbox_lane::{MAX_AIM_OFFSET, Phase, Snapshot};
use bowlingbox_render::ThemeId;
use bowlingbox_score::{ScoreSheet, Slot};
use eframe::egui::{self, Key, Modifiers};

use super::*;
use crate::cli::Cli;
use crate::fixed_step::{self, FixedStep};
use crate::recording_output::RecordingOutput;
use crate::settings_store::test_storage::Fake;
use crate::{hud_text, input, menus, scoreboard};
use clap::Parser as _;
use eframe::App as _;

type DefaultLane = Lane<RapierWorld, ScoreCalculator>;

fn app(theme: ThemeId, stored: SoundSettings, mute: bool) -> App {
    App::new(
        theme,
        false,
        (stored, mute),
        ScreenState::default(),
        Box::new(SilentOutput::default()),
    )
}

fn standing(s: &Snapshot) -> usize {
    s.pin_standing.iter().filter(|up| **up).count()
}

/// One 60 Hz frame of the app's clock.
fn frame(clock: &mut FixedStep, lane: &mut DefaultLane) {
    for _ in 0..clock.advance(fixed_step::DT) {
        lane.step(fixed_step::DT);
    }
}

/// A key press as the window handles it, in the lane's current phase.
fn press(lane: &mut DefaultLane, key: Key) {
    let phase = lane.snapshot().phase;
    if let Some(command) = input::key_down(key, Modifiers::NONE, phase) {
        let _ = lane_commands::execute(command, lane);
    }
}

fn play_until(clock: &mut FixedStep, lane: &mut DefaultLane, phases: &[Phase]) {
    for _ in 0..(20 * 60) {
        frame(clock, lane);
        if phases.contains(&lane.snapshot().phase) {
            return;
        }
    }
    panic!("the lane did not reach {phases:?}");
}

/// Space held for `power`, then released.
fn throw(clock: &mut FixedStep, lane: &mut DefaultLane, power: f32) {
    press(lane, Key::Space);
    while lane.snapshot().power < power {
        frame(clock, lane);
    }
    lane_commands::execute(Command::Release, lane).unwrap();
}

// Item 1
#[test]
fn item_1_launch_shows_the_lane_ten_pins_a_ball_and_an_empty_board() {
    let lane = DefaultLane::default();
    let s = lane.snapshot();
    assert_eq!(s.phase, Phase::Aiming);
    assert_eq!(standing(&s), 10);
    assert_eq!(s.pin_in_play, [true; 10]);
    assert!((s.ball.position[2] - bowlingbox_lane::BALL_START_Z).abs() < 0.01);
    let scene = scene_adapter::to_scene(&s);
    assert_eq!(scene.pin_visible, [true; 10]);
    assert!(scene.show_aim_guide);
    assert_eq!(lane.score_sheet().score(), 0);
    for cell in scoreboard::frames(lane.score_sheet()) {
        assert!(cell.marks.iter().all(String::is_empty) && cell.total.is_empty());
    }
    assert_eq!(scoreboard::total_text(lane.score_sheet()), "Total 0");
    assert_eq!(hud_text::result(&s, lane.score_sheet()), "");
}

// Item 3
#[test]
#[cfg_attr(debug_assertions, ignore = "slow in debug")]
fn item_3_holding_right_to_the_edge_is_a_gutter_with_a_dash() {
    let mut lane = DefaultLane::default();
    let mut clock = FixedStep::default();
    for _ in 0..100 {
        press(&mut lane, Key::ArrowRight);
    }
    assert!((lane.snapshot().aim_offset - MAX_AIM_OFFSET).abs() < 1e-6);
    throw(&mut clock, &mut lane, 0.6);
    play_until(&mut clock, &mut lane, &[Phase::FrameDone]);
    let s = lane.snapshot();
    assert_eq!(hud_text::result(&s, lane.score_sheet()), "Gutter");
    assert_eq!(lane.score_sheet().rolls()[0], Slot::Pins(0));
    assert_eq!(scoreboard::frames(lane.score_sheet())[0].marks[0], "-");
    assert_eq!(standing(&s), 10);
}

// Item 4
#[test]
#[cfg_attr(debug_assertions, ignore = "slow in debug")]
fn item_4_after_a_partial_hit_the_standing_pins_stay_put() {
    let mut lane = DefaultLane::default();
    let mut clock = FixedStep::default();
    lane_commands::execute(Command::MoveAim(0.42), &mut lane).unwrap();
    throw(&mut clock, &mut lane, 0.4);
    play_until(&mut clock, &mut lane, &[Phase::FrameDone]);
    let before = lane.snapshot();
    let down = before.last_pins_down.expect("a roll was scored");
    assert!((1..10).contains(&down), "a partial hit, got {down}");
    play_until(&mut clock, &mut lane, &[Phase::Aiming]);
    let after = lane.snapshot();
    assert_eq!(standing(&after), usize::from(10 - down));
    for i in (0..10).filter(|&i| after.pin_standing[i]) {
        let (a, b) = (before.pins[i].position, after.pins[i].position);
        let moved: f32 = a.iter().zip(b).map(|(a, b)| (a - b).powi(2)).sum();
        assert!(moved.sqrt() < 0.001, "pin {i} moved");
    }
}

// Item 6
#[test]
fn item_6_n_restarts_during_every_phase_of_a_roll() {
    let mut lane = DefaultLane::default();
    let mut clock = FixedStep::default();
    throw(&mut clock, &mut lane, 0.5);
    for _ in 0..30 {
        frame(&mut clock, &mut lane);
    }
    assert_eq!(lane.snapshot().phase, Phase::Rolling);
    press(&mut lane, Key::N);
    let s = lane.snapshot();
    assert_eq!((s.phase, standing(&s)), (Phase::Aiming, 10));
    assert_eq!(lane.score_sheet().score(), 0);
    press(&mut lane, Key::Space);
    assert_eq!(lane.snapshot().phase, Phase::Charging);
    press(&mut lane, Key::N);
    assert_eq!(lane.snapshot().phase, Phase::Aiming);
}

#[test]
fn item_6_escape_quits_unless_a_dialog_or_menu_takes_it() {
    let escape = input::key_down(Key::Escape, Modifiers::NONE, Phase::Aiming).unwrap();
    assert_eq!(route(escape, false, false), Route::Run(Command::Quit));
    assert_eq!(Command::Quit.target(), crate::command::CommandTarget::App);
    assert_eq!(route(escape, true, false), Route::CloseDialog);
    assert_eq!(route(escape, false, true), Route::Ignore);
}

// Item 7
#[test]
fn item_7_theme_names_start_that_theme_and_an_unknown_one_warns() {
    let start = |name: &str| {
        Cli::try_parse_from(["bowlingbox", &format!("--theme={name}")])
            .unwrap()
            .startup(None)
    };
    for (name, id) in [
        ("neon", ThemeId::NeonHall),
        ("classic", ThemeId::ClassicAlley),
        ("outdoor", ThemeId::Outdoor),
    ] {
        let startup = start(name);
        assert_eq!(startup.theme, id);
        assert_eq!(startup.warnings, Vec::<String>::new());
        assert_eq!(app(startup.theme, SoundSettings::default(), true).theme, id);
    }
    let space = start("space");
    assert_eq!(space.theme, ThemeId::NeonHall);
    assert_eq!(
        space.warnings,
        ["unknown theme 'space'; use neon, classic or outdoor"]
    );
}

// Item 8
#[test]
fn item_8_t_cycles_the_themes_and_the_menu_reads_the_same_state() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeId::NeonHall, SoundSettings::default(), true);
    let mut seen = Vec::new();
    for _ in 0..3 {
        let command = input::key_down(Key::T, Modifiers::NONE, Phase::Aiming).unwrap();
        app.dispatch(&ctx, command);
        seen.push(app.theme);
    }
    assert_eq!(
        seen,
        [ThemeId::ClassicAlley, ThemeId::Outdoor, ThemeId::NeonHall]
    );
    // The radio items are drawn from `app.theme` (`menus::show`); a pick goes the same way.
    app.dispatch_theme(ThemeId::Outdoor);
    assert_eq!(app.theme, ThemeId::Outdoor);
    // Behind a dialog nothing changes.
    app.dialog = Some(Dialog::About);
    app.dispatch(&ctx, Command::CycleTheme);
    app.dispatch_theme(ThemeId::NeonHall);
    assert_eq!(app.theme, ThemeId::Outdoor);
}

// Item 11
#[test]
fn item_11_keys_menu_and_status_stay_in_sync() {
    let mut out = RecordingOutput::new();
    let mut control =
        SoundControl::new(&mut out, SoundSettings::default(), false, ThemeId::NeonHall);
    let key = |key: Key| input::key_down(key, Modifiers::NONE, Phase::Aiming).unwrap();
    let check = |control: &SoundControl, out: &RecordingOutput, status: &str| {
        let s = control.settings();
        assert_eq!(out.muted(), s.muted);
        assert!((out.volume() - s.gain()).abs() < 1e-6);
        assert_eq!(menus::sound_menu(s).muted, s.muted);
        assert_eq!(menus::sound_menu(s).ambience, s.ambience);
        assert_eq!(control.status(), status);
    };
    control.apply(&mut out, key(Key::M));
    check(&control, &out, "sound muted");
    control.apply(&mut out, key(Key::Plus));
    check(&control, &out, "volume 90 %");
    control.apply(&mut out, key(Key::Minus));
    control.apply(&mut out, key(Key::Minus));
    check(&control, &out, "volume 70 %");
    control.apply(&mut out, key(Key::B));
    check(&control, &out, "volume 70 %, no ambience");
    assert_eq!(out.loop_sound(bowlingbox_audio::LoopSlot::Ambient), None);
    control.apply(&mut out, key(Key::B));
    assert!(
        out.loop_sound(bowlingbox_audio::LoopSlot::Ambient)
            .is_some()
    );
    for _ in 0..10 {
        control.apply(&mut out, key(Key::Plus));
    }
    assert!(!menus::sound_menu(control.settings()).volume.up_enabled);
    for _ in 0..10 {
        control.apply(&mut out, key(Key::Minus));
    }
    assert!(!menus::sound_menu(control.settings()).volume.down_enabled);
}

#[test]
fn item_11_settings_come_back_through_storage() {
    let changed = SoundSettings {
        muted: true,
        volume: 30,
        ambience: false,
    };
    let mut storage = Fake::default();
    app(ThemeId::NeonHall, changed, false).save(&mut storage);
    let restored = settings_store::load(Some(&storage));
    assert_eq!(restored, changed);
    let next = app(ThemeId::NeonHall, restored, false);
    assert_eq!(next.sound.settings(), changed);
    // A first start, with nothing stored, gives the defaults.
    assert_eq!(settings_store::load(None), SoundSettings::default());
}

// Item 12
#[test]
fn item_12_mute_starts_muted_and_a_silent_output_runs_the_game() {
    let mut out = RecordingOutput::new();
    let control = SoundControl::new(&mut out, SoundSettings::default(), true, ThemeId::NeonHall);
    assert!(out.muted() && control.settings().muted);
    assert_eq!(control.status(), "sound muted");
    assert_eq!(out.played_count(), 0);
    // No device: `open_output_with` gives a `SilentOutput` and one `sound disabled:` line
    // (crates/audio/tests/output.rs); the app plays on it.
    let mut silent = SilentOutput::default();
    let mut lane = DefaultLane::default();
    let mut control = SoundControl::new(
        &mut silent,
        SoundSettings::default(),
        false,
        ThemeId::Outdoor,
    );
    let snapshot = control.step_frame(&mut silent, &mut lane, 2);
    assert_eq!(snapshot.phase, Phase::Aiming);
}
