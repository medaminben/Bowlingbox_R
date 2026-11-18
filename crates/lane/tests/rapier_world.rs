//! The physics tests of the C++ `test_Alley.cpp`, against the Rapier world.
//!
//! The two whole-game tests take about half a minute in debug and run with `--release`.

use std::f32::consts::PI;

use bowlingbox_lane::{
    Body, LANE, Lane, MAX_AIM_ANGLE_DEG, MAX_AIM_OFFSET, POCKET_ANGLE_DEG, POCKET_OFFSET,
    POCKET_POWER, Phase, RapierWorld, Snapshot,
};
use bowlingbox_score::{ScoreCalculator, ScoreSheet, Slot};

const DT: f32 = 1.0 / 60.0;
/// Impacts weaker than this are not reported (N·s).
const IMPACT_THRESHOLD: f32 = 0.3;
const MAX_IMPACTS: usize = 256;

type RapierLane = Lane<RapierWorld, ScoreCalculator>;

/// Time at which the 1 Hz charge curve `power(t) = (1 - cos 2πt) / 2` reaches `power`.
fn charge_time_for(power: f32) -> f32 {
    (1.0 - 2.0 * power).acos() / (2.0 * PI)
}

fn standing_count(s: &Snapshot) -> usize {
    s.pin_standing.iter().filter(|&&b| b).count()
}

/// Aims, charges to `power` and releases.
fn launch(lane: &mut RapierLane, offset: f32, angle_deg: f32, power: f32) {
    let s = lane.snapshot();
    lane.move_aim(offset - s.aim_offset).unwrap();
    lane.rotate_aim(angle_deg - s.aim_angle).unwrap();
    lane.start_charge().unwrap();
    lane.step(charge_time_for(power));
    lane.release().unwrap();
}

/// Full throw: launch, then step until the ball is scored and the lane is back in Aiming
/// (or `GameOver`).
fn throw_ball(lane: &mut RapierLane, offset: f32, angle_deg: f32, power: f32) {
    launch(lane, offset, angle_deg, power);
    for _ in 0..60 * 60 {
        if matches!(lane.snapshot().phase, Phase::Aiming | Phase::GameOver) {
            return;
        }
        lane.step(DT);
    }
    panic!("the throw did not end within a minute");
}

fn pocket_throw(lane: &mut RapierLane) {
    throw_ball(lane, POCKET_OFFSET, POCKET_ANGLE_DEG, POCKET_POWER);
}

#[test]
fn gutter_ball_scores_zero_and_keeps_the_rack() {
    let mut lane = RapierLane::default();
    throw_ball(&mut lane, MAX_AIM_OFFSET, 0.0, 0.6);
    let s = lane.snapshot();
    assert_eq!(s.phase, Phase::Aiming);
    assert_eq!(s.last_pins_down, Some(0));
    assert_eq!(standing_count(&s), 10);
    assert_eq!(lane.score_sheet().rolls()[0], Slot::Pins(0));
    assert_eq!(lane.score_sheet().pins_left(), 10);
}

#[test]
fn pocket_throw_is_a_strike() {
    let mut lane = RapierLane::default();
    pocket_throw(&mut lane);
    let s = lane.snapshot();
    assert_eq!(s.last_pins_down, Some(10));
    assert_eq!(lane.score_sheet().rolls()[0], Slot::Strike);
    assert_eq!(standing_count(&s), 10, "fresh rack for the next frame");
    // A strike's frame total is unknown until its two bonus balls are thrown.
    assert_eq!(lane.score_sheet().frame_totals()[0], None);
}

#[test]
fn same_inputs_give_identical_results() {
    let mut lane = RapierLane::default();
    let mut other = RapierLane::default();
    throw_ball(&mut lane, 0.2, 1.5, 0.5);
    throw_ball(&mut other, 0.2, 1.5, 0.5);
    let a = lane.snapshot();
    let b = other.snapshot();
    assert_eq!(a.last_pins_down, b.last_pins_down);
    assert_eq!(a.pin_standing, b.pin_standing);
    for i in 0..10 {
        assert_eq!(a.pins[i].position, b.pins[i].position, "pin {i}");
    }
}

#[test]
fn second_ball_keeps_standing_pins() {
    let mut lane = RapierLane::default();
    throw_ball(&mut lane, 0.42, 0.0, 0.4); // edge hit: partial knockdown
    let s = lane.snapshot();
    let down = s.last_pins_down.expect("a roll was scored");
    assert!(
        (1..10).contains(&down),
        "partial knockdown expected, got {down}"
    );
    assert_eq!(standing_count(&s), usize::from(10 - down));
    assert_eq!(lane.score_sheet().pins_left(), 10 - down);
    // Fallen pins were removed.
    assert_eq!(s.pin_in_play, s.pin_standing);
}

#[test]
fn fallen_pins_stay_visible_until_aiming() {
    let mut lane = RapierLane::default();
    launch(&mut lane, POCKET_OFFSET, POCKET_ANGLE_DEG, POCKET_POWER);
    for _ in 0..60 * 20 {
        if lane.snapshot().phase == Phase::FrameDone {
            break;
        }
        lane.step(DT);
    }
    let s = lane.snapshot();
    assert_eq!(s.phase, Phase::FrameDone);
    assert_eq!(s.pin_in_play, [true; 10]);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "slow in debug")]
fn twenty_gutter_balls_end_the_game_with_zero() {
    let mut lane = RapierLane::default();
    for _ in 0..20 {
        throw_ball(&mut lane, MAX_AIM_OFFSET, 0.0, 0.5);
    }
    assert_eq!(lane.snapshot().phase, Phase::GameOver);
    assert!(lane.score_sheet().is_over());
    assert_eq!(lane.score_sheet().score(), 0);
    assert!(lane.start_charge().is_err());
}

#[test]
#[cfg_attr(debug_assertions, ignore = "slow in debug")]
fn twelve_strikes_score_three_hundred() {
    let mut lane = RapierLane::default();
    for i in 0..12 {
        pocket_throw(&mut lane);
        assert_eq!(lane.snapshot().last_pins_down, Some(10), "ball {}", i + 1);
    }
    assert_eq!(lane.snapshot().phase, Phase::GameOver);
    assert_eq!(lane.score_sheet().score(), 300);
}

#[test]
fn a_pocket_throw_reports_ball_pin_impacts() {
    let mut lane = RapierLane::default();
    launch(&mut lane, POCKET_OFFSET, POCKET_ANGLE_DEG, POCKET_POWER);
    let mut ball_hit_pin = false;
    for _ in 0..60 * 4 {
        lane.step(DT);
        let impacts = lane.drain_impacts();
        assert!(impacts.len() <= MAX_IMPACTS);
        for im in &impacts {
            assert!(im.a <= im.b, "{im:?}");
            assert!(im.impulse >= IMPACT_THRESHOLD, "{im:?}");
            if im.a == Body::Ball && im.b == Body::Pin {
                ball_hit_pin = true;
                // Near the rack, at most pin height above the deck.
                assert!(im.position[2] > LANE.head_pin_z - 0.5, "{im:?}");
                assert!(
                    (-0.01..=LANE.pin_height).contains(&im.position[1]),
                    "{im:?}"
                );
            }
        }
        if ball_hit_pin {
            break;
        }
    }
    assert!(ball_hit_pin);
    assert_eq!(lane.drain_impacts(), Vec::new());
}

#[test]
fn no_pin_impacts_before_the_ball_reaches_the_rack() {
    let mut lane = RapierLane::default();
    launch(&mut lane, 0.0, 0.0, 0.05);
    for _ in 0..60 {
        lane.step(DT);
    }
    for im in lane.drain_impacts() {
        assert_ne!(im.a, Body::Pin, "{im:?}");
        assert_ne!(im.b, Body::Pin, "{im:?}");
    }
}

#[test]
fn new_game_drops_pending_impacts() {
    let mut lane = RapierLane::default();
    launch(&mut lane, POCKET_OFFSET, POCKET_ANGLE_DEG, POCKET_POWER);
    for _ in 0..60 * 3 {
        lane.step(DT);
    }
    lane.new_game().unwrap();
    assert_eq!(lane.drain_impacts(), Vec::new());
}

/// Review focus: the ball never leaves the world sideways, and the roll ends.
#[test]
fn edge_throw_ends_rolling() {
    for side in [1.0_f32, -1.0] {
        let mut lane = RapierLane::default();
        launch(
            &mut lane,
            side * MAX_AIM_OFFSET,
            side * MAX_AIM_ANGLE_DEG,
            1.0,
        );
        let mut rolled = 0.0;
        while lane.snapshot().phase == Phase::Rolling {
            let ball = lane.snapshot().ball.position;
            let outer = LANE.half_width + LANE.gutter_width + 0.1;
            assert!(ball[0].abs() < outer, "ball left sideways at {ball:?}");
            lane.step(DT);
            rolled += DT;
            assert!(rolled < 8.5, "Rolling did not end");
        }
        for _ in 0..60 * 10 {
            if lane.snapshot().phase == Phase::Aiming {
                break;
            }
            lane.step(DT);
        }
        assert_eq!(lane.snapshot().phase, Phase::Aiming, "side {side}");
        assert_eq!(lane.score_sheet().rolls()[0], Slot::Pins(0), "side {side}");
    }
}

/// Review focus: a power-0 throw (4 m/s) stops on the lane; the 0.1 m/s rule ends Rolling.
#[test]
fn slow_throw_stops_on_the_lane() {
    let mut lane = RapierLane::default();
    launch(&mut lane, 0.0, 0.0, 0.0);
    let mut rolled = 0.0;
    while lane.snapshot().phase == Phase::Rolling {
        lane.step(DT);
        rolled += DT;
    }
    let s = lane.snapshot();
    assert!(rolled < 8.0, "ended by the timeout after {rolled} s");
    assert!(
        s.ball.position[2] < LANE.head_pin_z - 0.5,
        "ball at {:?}",
        s.ball.position
    );
    assert!(
        s.ball.position[1] > 0.0,
        "ball left the lane: {:?}",
        s.ball.position
    );
    assert_eq!(s.phase, Phase::Settling);
    while lane.snapshot().phase == Phase::Settling {
        lane.step(DT);
    }
    assert_eq!(lane.snapshot().phase, Phase::FrameDone);
    assert_eq!(lane.snapshot().last_pins_down, Some(0));
}
