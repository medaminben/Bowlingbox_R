use std::f32::consts::PI;
use std::fmt;

use bowlingbox_score::Game;

use crate::types::{Impact, MAX_AIM_ANGLE_DEG, MAX_AIM_OFFSET, Phase, Snapshot};
use crate::world::PhysicsWorld;

const CHARGE_HZ: f32 = 1.0;
const MIN_SPEED: f32 = 4.0;
const SPEED_RANGE: f32 = 8.0;
const ROLLING_TIMEOUT: f32 = 8.0;
const BALL_STOPPED_AFTER: f32 = 1.0;
const BALL_STOPPED_SPEED: f32 = 0.1;
const SETTLE_TIMEOUT: f32 = 3.0;
const FRAME_DONE_DURATION: f32 = 1.5;

/// A command that is not valid in the current phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaneError {
    WrongPhase(Phase),
}

impl fmt::Display for LaneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongPhase(phase) => write!(f, "not valid while {phase}"),
        }
    }
}

impl std::error::Error for LaneError {}

/// The throw state machine over a physics world and a scorer.
///
/// Input is valid in `Aiming` and `Charging` (`new_game`: any phase); otherwise it returns
/// an error and changes nothing.
pub struct Lane<W: PhysicsWorld, G: Game> {
    world: W,
    game: G,
    phase: Phase,
    phase_time: f32,
    aim_offset: f32,
    aim_angle: f32,
    power: f32,
    last_pins_down: Option<u8>,
}

impl<W: PhysicsWorld, G: Game> Lane<W, G> {
    #[must_use]
    pub fn new(mut world: W, mut game: G) -> Self {
        game.reset();
        world.reset_rack();
        world.place_ball(0.0);
        Self {
            world,
            game,
            phase: Phase::Aiming,
            phase_time: 0.0,
            aim_offset: 0.0,
            aim_angle: 0.0,
            power: 0.0,
            last_pins_down: None,
        }
    }

    fn can_aim(&self) -> Result<(), LaneError> {
        match self.phase {
            Phase::Aiming | Phase::Charging => Ok(()),
            phase => Err(LaneError::WrongPhase(phase)),
        }
    }

    /// Moves the ball sideways, clamped to the lane and the gutters.
    ///
    /// # Errors
    /// `WrongPhase` unless aiming or charging.
    pub fn move_aim(&mut self, delta: f32) -> Result<(), LaneError> {
        self.can_aim()?;
        self.aim_offset = (self.aim_offset + delta).clamp(-MAX_AIM_OFFSET, MAX_AIM_OFFSET);
        self.world.place_ball(self.aim_offset);
        Ok(())
    }

    /// Turns the throw direction in degrees, clamped.
    ///
    /// # Errors
    /// `WrongPhase` unless aiming or charging.
    pub fn rotate_aim(&mut self, delta_deg: f32) -> Result<(), LaneError> {
        self.can_aim()?;
        self.aim_angle = (self.aim_angle + delta_deg).clamp(-MAX_AIM_ANGLE_DEG, MAX_AIM_ANGLE_DEG);
        Ok(())
    }

    /// Aiming to Charging.
    ///
    /// # Errors
    /// `WrongPhase` unless aiming.
    pub fn start_charge(&mut self) -> Result<(), LaneError> {
        if self.phase != Phase::Aiming {
            return Err(LaneError::WrongPhase(self.phase));
        }
        self.power = 0.0;
        self.enter(Phase::Charging);
        Ok(())
    }

    /// Charging to Rolling: launches the ball.
    ///
    /// # Errors
    /// `WrongPhase` unless charging.
    pub fn release(&mut self) -> Result<(), LaneError> {
        if self.phase != Phase::Charging {
            return Err(LaneError::WrongPhase(self.phase));
        }
        self.world.launch_ball(
            self.aim_offset,
            self.aim_angle,
            MIN_SPEED + SPEED_RANGE * self.power,
        );
        self.enter(Phase::Rolling);
        Ok(())
    }

    /// Resets scorer and world from any phase. Always succeeds.
    ///
    /// # Errors
    /// Never; the `Result` keeps the signature of the other commands.
    pub fn new_game(&mut self) -> Result<(), LaneError> {
        self.game.reset();
        self.world.rebuild();
        self.aim_offset = 0.0;
        self.aim_angle = 0.0;
        self.power = 0.0;
        self.last_pins_down = None;
        self.world.place_ball(0.0);
        let _ = self.world.drain_impacts();
        self.enter(Phase::Aiming);
        Ok(())
    }

    /// Advances the phase timer and the world by `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        self.phase_time += dt;
        match self.phase {
            Phase::Charging => {
                self.power = 0.5 * (1.0 - (2.0 * PI * CHARGE_HZ * self.phase_time).cos());
            }
            Phase::Rolling => {
                self.world.step(dt);
                if self.world.ball_in_pit()
                    || (self.phase_time > BALL_STOPPED_AFTER
                        && self.world.ball_speed() < BALL_STOPPED_SPEED)
                    || self.phase_time > ROLLING_TIMEOUT
                {
                    self.enter(Phase::Settling);
                }
            }
            Phase::Settling => {
                self.world.step(dt);
                if self.world.all_slow() || self.phase_time > SETTLE_TIMEOUT {
                    self.finish_throw();
                }
            }
            Phase::FrameDone => {
                if self.phase_time >= FRAME_DONE_DURATION {
                    if self.game.is_over() {
                        self.enter(Phase::GameOver);
                    } else {
                        self.prepare_next_ball();
                    }
                }
            }
            Phase::Aiming | Phase::GameOver => {}
        }
    }

    fn enter(&mut self, phase: Phase) {
        self.phase = phase;
        self.phase_time = 0.0;
    }

    fn is_down(&self, i: usize) -> bool {
        self.world.is_pin_in_play(i) && !self.world.is_pin_standing(i)
    }

    fn finish_throw(&mut self) {
        let down = (0..10).filter(|&i| self.is_down(i)).count();
        let down = u8::try_from(down).unwrap_or(10);
        if self.game.roll(down).is_err() {
            // The scorer refused the roll, which the FrameDone transition normally prevents.
            self.last_pins_down = None;
            self.enter(Phase::GameOver);
            return;
        }
        self.last_pins_down = Some(down);
        self.enter(Phase::FrameDone);
    }

    fn prepare_next_ball(&mut self) {
        if self.game.pins_left() == bowlingbox_score::PINS {
            self.world.rebuild();
        } else {
            for i in 0..10 {
                if self.is_down(i) {
                    self.world.remove_pin(i);
                }
            }
        }
        self.world.place_ball(self.aim_offset);
        self.enter(Phase::Aiming);
    }

    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            phase: self.phase,
            ball: self.world.ball_pose(),
            pins: std::array::from_fn(|i| self.world.pin_pose(i)),
            pin_standing: std::array::from_fn(|i| self.world.is_pin_standing(i)),
            pin_in_play: std::array::from_fn(|i| self.world.is_pin_in_play(i)),
            aim_offset: self.aim_offset,
            aim_angle: self.aim_angle,
            power: self.power,
            last_pins_down: self.last_pins_down,
        }
    }

    #[must_use]
    pub fn score_sheet(&self) -> &G {
        &self.game
    }

    /// New physics contacts since the last call, oldest first.
    pub fn drain_impacts(&mut self) -> Vec<Impact> {
        self.world.drain_impacts()
    }
}
