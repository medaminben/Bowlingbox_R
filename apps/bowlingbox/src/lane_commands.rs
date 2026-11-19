//! Applies the lane's commands to a lane.

use bowlingbox_lane::{Lane, LaneError, PhysicsWorld};
use bowlingbox_score::Game;

use crate::command::Command;

/// Runs a command that targets the lane. Any other command succeeds without effect.
///
/// # Errors
/// The lane's error when the command is not valid in its phase.
pub fn execute<W: PhysicsWorld, G: Game>(
    command: Command,
    lane: &mut Lane<W, G>,
) -> Result<(), LaneError> {
    match command {
        Command::MoveAim(metres) => lane.move_aim(metres),
        Command::RotateAim(degrees) => lane.rotate_aim(degrees),
        Command::StartCharge => lane.start_charge(),
        Command::Release => lane.release(),
        Command::NewGame => lane.new_game(),
        Command::CycleTheme
        | Command::ToggleMute
        | Command::VolumeUp
        | Command::VolumeDown
        | Command::ToggleAmbience
        | Command::Quit
        | Command::Escape => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixed_step::{self, FixedStep};
    use bowlingbox_lane::{POCKET_ANGLE_DEG, POCKET_OFFSET, POCKET_POWER, Phase, RapierWorld};
    use bowlingbox_score::{ScoreCalculator, ScoreSheet, Slot};

    type DefaultLane = Lane<RapierWorld, ScoreCalculator>;

    #[test]
    fn routes_commands_to_the_lane() {
        let mut lane = DefaultLane::default();
        assert!(execute(Command::MoveAim(0.1), &mut lane).is_ok());
        assert!((lane.snapshot().aim_offset - 0.1).abs() < 1e-6);
        assert!(execute(Command::RotateAim(2.0), &mut lane).is_ok());
        assert!((lane.snapshot().aim_angle - 2.0).abs() < 1e-6);
        assert!(execute(Command::StartCharge, &mut lane).is_ok());
        assert_eq!(lane.snapshot().phase, Phase::Charging);
        assert!(execute(Command::NewGame, &mut lane).is_ok());
        assert_eq!(lane.snapshot().phase, Phase::Aiming);
    }

    #[test]
    fn reports_rejections_and_ignores_non_lane_commands() {
        let mut lane = DefaultLane::default();
        assert!(execute(Command::Release, &mut lane).is_err());
        assert!(execute(Command::Quit, &mut lane).is_ok());
    }

    #[test]
    fn view_and_sound_commands_leave_the_lane_alone() {
        let mut lane = DefaultLane::default();
        execute(Command::MoveAim(0.1), &mut lane).unwrap();
        let before = lane.snapshot();
        for command in [
            Command::CycleTheme,
            Command::ToggleMute,
            Command::VolumeUp,
            Command::VolumeDown,
            Command::ToggleAmbience,
        ] {
            assert!(execute(command, &mut lane).is_ok());
        }
        assert_eq!(lane.snapshot(), before);
    }

    /// Runs the clock for one 60 Hz frame and steps the lane for the steps it yields.
    fn frame(clock: &mut FixedStep, lane: &mut DefaultLane) {
        for _ in 0..clock.advance(fixed_step::DT) {
            lane.step(fixed_step::DT);
        }
    }

    #[test]
    #[cfg_attr(debug_assertions, ignore = "slow in debug")]
    fn the_tuned_pocket_throw_is_a_strike() {
        let mut lane = DefaultLane::default();
        let mut clock = FixedStep::default();
        execute(Command::MoveAim(POCKET_OFFSET), &mut lane).unwrap();
        execute(Command::RotateAim(POCKET_ANGLE_DEG), &mut lane).unwrap();
        execute(Command::StartCharge, &mut lane).unwrap();
        for _ in 0..120 {
            if lane.snapshot().power >= POCKET_POWER {
                break;
            }
            frame(&mut clock, &mut lane);
        }
        assert!(lane.snapshot().power >= POCKET_POWER);
        execute(Command::Release, &mut lane).unwrap();
        // Rolling and settling are at most 11 s, then 1.5 s of frame done.
        for _ in 0..(14 * 60) {
            frame(&mut clock, &mut lane);
            if lane.snapshot().phase == Phase::Aiming {
                break;
            }
        }
        assert_eq!(lane.snapshot().phase, Phase::Aiming);
        assert_eq!(lane.score_sheet().rolls()[0], Slot::Strike);
    }
}
