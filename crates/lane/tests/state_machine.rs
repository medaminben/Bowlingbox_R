//! The throw state machine against a scripted world and scorers. No physics engine.

use std::cell::RefCell;
use std::rc::Rc;

use bowlingbox_lane::{
    Body, Impact, Lane, LaneError, MAX_AIM_ANGLE_DEG, MAX_AIM_OFFSET, Phase, PhysicsWorld, Pose,
    Snapshot,
};
use bowlingbox_score::{FRAMES, Game, ROLL_SLOTS, RollError, ScoreCalculator, ScoreSheet, Slot};

const DT: f32 = 1.0 / 60.0;

/// State shared between a [`FakeWorld`] and the test that scripts and inspects it.
struct WorldState {
    /// Pins a launch knocks down, first standing ones first.
    knock_down: usize,
    /// The ball is in the pit right after the launch.
    pit_on_launch: bool,
    launched: bool,
    ball_speed: f32,
    all_slow: bool,
    standing: [bool; 10],
    in_play: [bool; 10],
    ball_x: f32,
    rebuilds: usize,
    removed: Vec<usize>,
    stepped: f32,
    launches: Vec<(f32, f32, f32)>,
    queued: Vec<Impact>,
}

impl Default for WorldState {
    fn default() -> Self {
        Self {
            knock_down: 0,
            pit_on_launch: true,
            launched: false,
            ball_speed: 0.0,
            all_slow: true,
            standing: [true; 10],
            in_play: [true; 10],
            ball_x: 0.0,
            rebuilds: 0,
            removed: Vec::new(),
            stepped: 0.0,
            launches: Vec::new(),
            queued: Vec::new(),
        }
    }
}

#[derive(Clone, Default)]
struct FakeWorld(Rc<RefCell<WorldState>>);

impl PhysicsWorld for FakeWorld {
    fn step(&mut self, dt: f32) {
        self.0.borrow_mut().stepped += dt;
    }

    fn place_ball(&mut self, offset: f32) {
        let mut s = self.0.borrow_mut();
        s.launched = false;
        s.ball_x = offset;
    }

    fn launch_ball(&mut self, offset: f32, angle_deg: f32, speed: f32) {
        let mut s = self.0.borrow_mut();
        s.launched = s.pit_on_launch;
        s.launches.push((offset, angle_deg, speed));
        let mut left = s.knock_down;
        for i in 0..10 {
            if left > 0 && s.in_play[i] && s.standing[i] {
                s.standing[i] = false;
                left -= 1;
            }
        }
    }

    fn reset_rack(&mut self) {
        let mut s = self.0.borrow_mut();
        s.standing = [true; 10];
        s.in_play = [true; 10];
    }

    fn rebuild(&mut self) {
        self.0.borrow_mut().rebuilds += 1;
        self.reset_rack();
    }

    fn remove_pin(&mut self, i: usize) {
        let mut s = self.0.borrow_mut();
        s.in_play[i] = false;
        s.removed.push(i);
    }

    fn ball_pose(&self) -> Pose {
        Pose {
            position: [self.0.borrow().ball_x, 0.0, 0.0],
            ..Pose::default()
        }
    }

    fn pin_pose(&self, _i: usize) -> Pose {
        Pose::default()
    }

    fn is_pin_standing(&self, i: usize) -> bool {
        let s = self.0.borrow();
        s.in_play[i] && s.standing[i]
    }

    fn is_pin_in_play(&self, i: usize) -> bool {
        self.0.borrow().in_play[i]
    }

    fn ball_in_pit(&self) -> bool {
        self.0.borrow().launched
    }

    fn ball_speed(&self) -> f32 {
        self.0.borrow().ball_speed
    }

    fn all_slow(&self) -> bool {
        self.0.borrow().all_slow
    }

    fn drain_impacts(&mut self) -> Vec<Impact> {
        std::mem::take(&mut self.0.borrow_mut().queued)
    }
}

/// Records every roll; reports over once `rolls_until_over` rolls were made.
struct FakeGame {
    recorded: Vec<u8>,
    rolls_until_over: usize,
    pins_left: u8,
}

impl FakeGame {
    fn new() -> Self {
        Self {
            recorded: Vec::new(),
            rolls_until_over: 100,
            pins_left: 10,
        }
    }
}

impl ScoreSheet for FakeGame {
    fn rolls(&self) -> [Slot; ROLL_SLOTS] {
        [Slot::NotPlayed; ROLL_SLOTS]
    }
    fn frame_totals(&self) -> [Option<u16>; FRAMES] {
        [None; FRAMES]
    }
    fn score(&self) -> u16 {
        0
    }
    fn frame_score(&self, _frame: usize) -> u16 {
        0
    }
    fn pins_left(&self) -> u8 {
        self.pins_left
    }
    fn is_over(&self) -> bool {
        self.recorded.len() >= self.rolls_until_over
    }
}

impl Game for FakeGame {
    fn roll(&mut self, pins: u8) -> Result<(), RollError> {
        if self.is_over() {
            return Err(RollError::GameOver);
        }
        self.recorded.push(pins);
        Ok(())
    }
    fn reset(&mut self) {
        self.recorded.clear();
    }
}

fn standing_count(s: &Snapshot) -> usize {
    s.pin_standing.iter().filter(|&&b| b).count()
}

/// Starts a throw with 0.25 s of charge and steps `seconds` after the release.
fn throw_and_wait<G: Game>(lane: &mut Lane<FakeWorld, G>, seconds: f32) {
    lane.start_charge().unwrap();
    lane.step(0.25);
    lane.release().unwrap();
    let mut t = 0.0;
    while t < seconds {
        lane.step(DT);
        t += DT;
    }
}

fn scored_lane() -> (Lane<FakeWorld, ScoreCalculator>, FakeWorld) {
    let world = FakeWorld::default();
    (Lane::new(world.clone(), ScoreCalculator::default()), world)
}

fn injected(game: FakeGame) -> (Lane<FakeWorld, FakeGame>, FakeWorld) {
    let world = FakeWorld::default();
    (Lane::new(world.clone(), game), world)
}

// ------------------------------------------------------------ phase-only tests

#[test]
fn initial_state_is_aiming_with_full_rack() {
    let (lane, _) = scored_lane();
    let s = lane.snapshot();
    assert_eq!(s.phase, Phase::Aiming);
    assert_eq!(standing_count(&s), 10);
    assert_eq!(s.last_pins_down, None);
    assert_eq!(s.aim_offset, 0.0);
    assert_eq!(lane.score_sheet().score(), 0);
    assert!(!lane.score_sheet().is_over());
}

#[test]
fn release_in_aiming_is_an_error() {
    let (mut lane, _) = scored_lane();
    assert_eq!(lane.release(), Err(LaneError::WrongPhase(Phase::Aiming)));
    assert_eq!(lane.snapshot().phase, Phase::Aiming);
}

#[test]
fn start_charge_twice_is_an_error() {
    let (mut lane, _) = scored_lane();
    lane.start_charge().unwrap();
    assert_eq!(
        lane.start_charge(),
        Err(LaneError::WrongPhase(Phase::Charging))
    );
    assert_eq!(lane.snapshot().phase, Phase::Charging);
}

#[test]
fn move_aim_clamps_to_range() {
    let (mut lane, _) = scored_lane();
    lane.move_aim(5.0).unwrap();
    assert_eq!(lane.snapshot().aim_offset, MAX_AIM_OFFSET);
    assert_eq!(lane.snapshot().ball.position[0], MAX_AIM_OFFSET);
    lane.rotate_aim(-90.0).unwrap();
    assert_eq!(lane.snapshot().aim_angle, -MAX_AIM_ANGLE_DEG);
}

#[test]
fn aim_while_rolling_is_an_error_and_changes_nothing() {
    let world = FakeWorld::default();
    world.0.borrow_mut().pit_on_launch = false;
    let mut lane = Lane::new(world, ScoreCalculator::default());
    lane.start_charge().unwrap();
    lane.step(0.25);
    lane.release().unwrap();
    assert_eq!(lane.snapshot().phase, Phase::Rolling);
    assert_eq!(
        lane.move_aim(0.1),
        Err(LaneError::WrongPhase(Phase::Rolling))
    );
    assert_eq!(
        lane.rotate_aim(1.0),
        Err(LaneError::WrongPhase(Phase::Rolling))
    );
    let s = lane.snapshot();
    assert_eq!((s.aim_offset, s.aim_angle), (0.0, 0.0));
}

#[test]
fn charging_power_follows_one_hertz_cosine() {
    let (mut lane, _) = scored_lane();
    lane.start_charge().unwrap();
    lane.step(0.25);
    assert!((lane.snapshot().power - 0.5).abs() < 1e-4);
    lane.step(0.25);
    assert!((lane.snapshot().power - 1.0).abs() < 1e-4);
    lane.step(0.5);
    assert!(lane.snapshot().power.abs() < 1e-4);
}

#[test]
fn release_launches_with_speed_from_power() {
    let (mut lane, world) = scored_lane();
    lane.move_aim(0.2).unwrap();
    lane.rotate_aim(1.5).unwrap();
    lane.start_charge().unwrap();
    lane.step(0.5); // power 1
    lane.release().unwrap();
    let launches = world.0.borrow().launches.clone();
    assert_eq!(launches.len(), 1);
    let (offset, angle, speed) = launches[0];
    assert_eq!((offset, angle), (0.2, 1.5));
    assert!((speed - 12.0).abs() < 1e-3);
}

#[test]
fn new_game_resets_everything() {
    let (mut lane, _) = scored_lane();
    lane.move_aim(0.3).unwrap();
    lane.start_charge().unwrap();
    assert_eq!(lane.new_game(), Ok(()));
    let s = lane.snapshot();
    assert_eq!(s.phase, Phase::Aiming);
    assert_eq!(s.aim_offset, 0.0);
    assert_eq!(standing_count(&s), 10);
    assert_eq!(lane.score_sheet().score(), 0);
}

#[test]
fn new_game_works_from_every_phase_and_drops_impacts() {
    let (mut lane, world) = scored_lane();
    world.0.borrow_mut().knock_down = 3;
    throw_and_wait(&mut lane, 0.1);
    assert_eq!(lane.snapshot().phase, Phase::FrameDone);
    world.0.borrow_mut().queued.push(Impact {
        a: Body::Ball,
        b: Body::Pin,
        impulse: 1.0,
        position: [0.0; 3],
    });
    assert_eq!(lane.new_game(), Ok(()));
    assert_eq!(lane.drain_impacts().len(), 0);
    assert_eq!(lane.snapshot().last_pins_down, None);
    assert_eq!(lane.score_sheet().rolls()[0], Slot::NotPlayed);
}

// ------------------------------------------------------------ timers

#[test]
fn step_moves_the_rolling_timer_by_dt() {
    let world = FakeWorld::default();
    {
        let mut s = world.0.borrow_mut();
        s.pit_on_launch = false;
        s.ball_speed = 5.0;
    }
    let mut lane = Lane::new(world.clone(), ScoreCalculator::default());
    lane.start_charge().unwrap();
    lane.step(0.25);
    lane.release().unwrap();
    // A fast ball in the open keeps rolling for the full 8 s, which is `> 8`.
    for _ in 0..4 {
        lane.step(2.0);
        assert_eq!(lane.snapshot().phase, Phase::Rolling);
    }
    assert!((world.0.borrow().stepped - 8.0).abs() < 1e-4);
    lane.step(2.0);
    assert_eq!(lane.snapshot().phase, Phase::Settling);
}

#[test]
fn a_stopped_ball_ends_rolling_after_one_second() {
    let world = FakeWorld::default();
    world.0.borrow_mut().pit_on_launch = false;
    let mut lane = Lane::new(world, ScoreCalculator::default());
    lane.start_charge().unwrap();
    lane.release().unwrap();
    lane.step(0.5);
    lane.step(0.5);
    assert_eq!(lane.snapshot().phase, Phase::Rolling); // 1.0 is not above 1.0
    lane.step(0.5);
    assert_eq!(lane.snapshot().phase, Phase::Settling);
}

#[test]
fn settling_ends_after_three_seconds_when_pins_keep_moving() {
    let world = FakeWorld::default();
    {
        let mut s = world.0.borrow_mut();
        s.all_slow = false;
        s.knock_down = 2;
    }
    let mut lane = Lane::new(world, ScoreCalculator::default());
    lane.start_charge().unwrap();
    lane.release().unwrap();
    lane.step(DT); // Rolling to Settling: the ball is in the pit
    assert_eq!(lane.snapshot().phase, Phase::Settling);
    lane.step(3.0);
    assert_eq!(lane.snapshot().phase, Phase::Settling);
    lane.step(0.5);
    assert_eq!(lane.snapshot().phase, Phase::FrameDone);
    assert_eq!(lane.snapshot().last_pins_down, Some(2));
}

#[test]
fn frame_done_lasts_one_and_a_half_seconds() {
    let (mut lane, _) = scored_lane();
    lane.start_charge().unwrap();
    lane.release().unwrap();
    lane.step(DT);
    lane.step(DT);
    assert_eq!(lane.snapshot().phase, Phase::FrameDone);
    lane.step(1.0);
    assert_eq!(lane.snapshot().phase, Phase::FrameDone);
    lane.step(0.5);
    assert_eq!(lane.snapshot().phase, Phase::Aiming);
}

// ------------------------------------------------------------ injected collaborators

#[test]
fn fallen_pins_are_credited_to_the_scorer() {
    let (mut lane, world) = injected(FakeGame::new());
    world.0.borrow_mut().knock_down = 7;
    throw_and_wait(&mut lane, 0.1);
    assert_eq!(lane.snapshot().phase, Phase::FrameDone);
    assert_eq!(lane.score_sheet().recorded, [7]);
    assert_eq!(lane.snapshot().last_pins_down, Some(7));
}

#[test]
fn second_ball_removes_fallen_pins() {
    let mut game = FakeGame::new();
    game.pins_left = 3;
    let (mut lane, world) = injected(game);
    world.0.borrow_mut().knock_down = 7;
    throw_and_wait(&mut lane, 2.0);
    let s = lane.snapshot();
    assert_eq!(s.phase, Phase::Aiming);
    assert_eq!(standing_count(&s), 3);
    assert_eq!(s.pin_in_play, s.pin_standing);
    assert_eq!(world.0.borrow().removed.len(), 7);
}

#[test]
fn full_rack_is_rebuilt_when_the_scorer_says_so() {
    let (mut lane, world) = injected(FakeGame::new());
    world.0.borrow_mut().knock_down = 10;
    let before = world.0.borrow().rebuilds;
    throw_and_wait(&mut lane, 2.0);
    assert_eq!(world.0.borrow().rebuilds, before + 1);
    assert_eq!(standing_count(&lane.snapshot()), 10);
}

#[test]
fn game_over_is_decided_by_the_scorer() {
    let mut game = FakeGame::new();
    game.rolls_until_over = 1;
    let (mut lane, _) = injected(game);
    throw_and_wait(&mut lane, 2.0);
    assert_eq!(lane.snapshot().phase, Phase::GameOver);
    assert!(lane.score_sheet().is_over());
    assert_eq!(
        lane.start_charge(),
        Err(LaneError::WrongPhase(Phase::GameOver))
    );
}

#[test]
fn tenth_frame_strike_then_open_balls_plays_three_balls() {
    let (mut lane, world) = scored_lane();
    world.0.borrow_mut().knock_down = 0;
    for _ in 0..18 {
        throw_and_wait(&mut lane, 2.0);
    }
    world.0.borrow_mut().knock_down = 10;
    throw_and_wait(&mut lane, 2.0);
    world.0.borrow_mut().knock_down = 3;
    throw_and_wait(&mut lane, 2.0);
    assert_eq!(lane.snapshot().phase, Phase::Aiming);
    assert_eq!(standing_count(&lane.snapshot()), 7); // fallen pins removed, no new rack
    world.0.borrow_mut().knock_down = 4;
    throw_and_wait(&mut lane, 2.0);
    assert_eq!(lane.snapshot().phase, Phase::GameOver);
    assert_eq!(lane.score_sheet().score(), 17);
}

#[test]
fn impacts_are_forwarded_once() {
    let (mut lane, world) = injected(FakeGame::new());
    world.0.borrow_mut().queued.push(Impact {
        a: Body::Ball,
        b: Body::Pin,
        impulse: 2.0,
        position: [0.0, 0.2, 17.4],
    });
    let first = lane.drain_impacts();
    assert_eq!(first.len(), 1);
    assert_eq!(
        (first[0].a, first[0].b, first[0].impulse),
        (Body::Ball, Body::Pin, 2.0)
    );
    assert_eq!(lane.drain_impacts().len(), 0);
}
