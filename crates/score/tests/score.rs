//! Port of the C++ `test_Score.cpp`. `RollStatus::Negative` is impossible with `u8`, so those
//! assertions are dropped.

use bowlingbox_score::{
    FRAMES, FrameView, Game, PINS, ROLL_SLOTS, RollError, ScoreCalculator, ScoreSheet, Slot,
    frames, frames_of, notation, total_notation,
};

fn roll_n_times(game: &mut ScoreCalculator, n: usize, pins: u8) {
    for _ in 0..n {
        assert_eq!(game.roll(pins), Ok(()));
    }
}

fn roll_all(game: &mut ScoreCalculator, balls: &[u8]) {
    for &pins in balls {
        assert_eq!(game.roll(pins), Ok(()), "roll({pins}) was refused");
    }
}

#[test]
fn roll_all_zeros() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 20, 0);
    assert_eq!(game.score(), 0);
}

#[test]
fn roll_all_ones() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 20, 1);
    assert_eq!(game.score(), 20);
}

#[test]
fn roll_a_spare() {
    let mut game = ScoreCalculator::default();
    roll_all(&mut game, &[5, 5, 6]);
    roll_n_times(&mut game, 17, 0);
    assert_eq!(game.score(), 22);
}

#[test]
fn roll_a_perfect_game() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 12, 10);
    assert_eq!(game.score(), 300);
}

#[test]
fn play_a_normal_game() {
    let mut game = ScoreCalculator::default();
    roll_all(&mut game, &[5, 0, 10, 10, 5, 5, 4]);
    roll_n_times(&mut game, 11, 0);
    assert_eq!(game.score(), 68);
}

#[test]
fn is_over_returns_true() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 20, 0);
    assert!(game.is_over());
}

#[test]
fn is_over_returns_false_spare() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 18, 0);
    roll_all(&mut game, &[1, 9]);
    assert!(!game.is_over());
}

#[test]
fn is_over_returns_true_nothing() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 18, 0);
    roll_all(&mut game, &[1, 2]);
    assert!(game.is_over());
}

#[test]
fn is_over_returns_false_strike() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 18, 0);
    roll_all(&mut game, &[10, 10]);
    assert!(!game.is_over());
}

#[test]
fn is_over_returns_true_strike() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 18, 0);
    roll_all(&mut game, &[10, 10, 10]);
    assert!(game.is_over());
}

#[test]
fn frame_score() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 4, 10);
    assert_eq!(game.frame_score(1), 60);
    assert_eq!(game.frame_score(9), 90);
}

#[test]
fn frame_score_perfect_game() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 12, 10);
    assert_eq!(game.frame_score(9), 300);
}

#[test]
fn frame_score_reset_game() {
    let mut game = ScoreCalculator::default();
    roll_all(&mut game, &[10, 10]);
    game.reset();
    assert_eq!(game.frame_score(9), 0);
}

#[test]
fn roll_almost_perfect_game() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 10, 10);
    assert_eq!(game.score(), 270);
}

#[test]
fn roll_is_accepted_while_playing() {
    let mut game = ScoreCalculator::default();
    assert_eq!(game.roll(3), Ok(()));
    assert_eq!(game.roll(4), Ok(()));
}

#[test]
fn roll_is_refused_when_game_is_over() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 20, 0);
    assert_eq!(game.roll(1), Err(RollError::GameOver));
}

#[test]
fn game_over_is_reported_before_a_bad_count() {
    // The C++ test also rolled -1; a negative count cannot be expressed with `u8`.
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 20, 0);
    assert_eq!(game.roll(11), Err(RollError::GameOver));
}

#[test]
fn describe_names_every_status() {
    // The C++ `Accepted` and `Negative` statuses do not exist: success is `Ok`, and `u8` is
    // never negative.
    assert_eq!(RollError::GameOver.to_string(), "the game is over");
    assert_eq!(
        RollError::TooManyPins.to_string(),
        "more pins than are standing"
    );
    let _: &dyn std::error::Error = &RollError::GameOver;
}

#[test]
fn new_game_starts_blank_without_reset() {
    let game = ScoreCalculator::default();
    assert!(game.rolls().iter().all(|&slot| slot == Slot::NotPlayed));
    assert!(game.frame_totals().iter().all(Option::is_none));
    assert_eq!(game.pins_left(), PINS);
    assert!(!game.is_over());
}

#[test]
fn usable_through_the_interfaces() {
    let mut calculator = ScoreCalculator::default();
    let game: &mut dyn Game = &mut calculator;
    assert_eq!(game.roll(10), Ok(()));
    let sheet: &dyn ScoreSheet = &calculator;
    assert_eq!(sheet.rolls()[0], Slot::Strike);
    assert_eq!(sheet.rolls()[1], Slot::Empty);
    assert_eq!(sheet.rolls().len(), ROLL_SLOTS);
    assert_eq!(sheet.frame_totals().len(), FRAMES);
}

#[test]
fn follows_score_sheet_conventions() {
    assert_eq!(notation(Slot::Empty), "");
    assert_eq!(notation(Slot::NotPlayed), "");
    assert_eq!(notation(Slot::Strike), "X");
    assert_eq!(notation(Slot::Spare), "/");
    assert_eq!(notation(Slot::Pins(0)), "-");
    assert_eq!(total_notation(Some(0)), "0");
    assert_eq!(total_notation(None), "");
    assert_eq!(notation(Slot::Pins(7)), "7");
    assert_eq!(total_notation(Some(300)), "300");
}

#[test]
fn slot_is_blank_for_unplayed_and_empty_only() {
    assert!(Slot::NotPlayed.is_blank());
    assert!(Slot::Empty.is_blank());
    assert!(!Slot::Strike.is_blank());
    assert!(!Slot::Spare.is_blank());
    assert!(!Slot::Pins(0).is_blank());
}

// ---------------------------------------------------------------- tenth frame

#[test]
fn tenth_strike_then_open_balls_gets_both_bonus_balls() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 18, 0);
    roll_all(&mut game, &[10, 3]);
    assert!(!game.is_over());
    assert_eq!(game.pins_left(), 7);
    assert_eq!(game.roll(8), Err(RollError::TooManyPins));
    roll_all(&mut game, &[4]);
    assert!(game.is_over());
    assert_eq!(game.score(), 17);
}

#[test]
fn tenth_strike_then_spare() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 18, 0);
    roll_all(&mut game, &[10, 3, 7]);
    let r = game.rolls();
    assert_eq!(r[18], Slot::Strike);
    assert_eq!(r[19], Slot::Pins(3));
    assert_eq!(r[20], Slot::Spare);
    assert_eq!(game.score(), 20);
}

#[test]
fn tenth_two_strikes_then_open_ball() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 18, 0);
    roll_all(&mut game, &[10, 10, 3]);
    let r = game.rolls();
    assert_eq!(r[18], Slot::Strike);
    assert_eq!(r[19], Slot::Strike);
    assert_eq!(r[20], Slot::Pins(3));
    assert_eq!(game.score(), 23);
}

#[test]
fn tenth_spare_then_strike() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 18, 0);
    roll_all(&mut game, &[3, 7, 10]);
    let r = game.rolls();
    assert_eq!(r[18], Slot::Pins(3));
    assert_eq!(r[19], Slot::Spare);
    assert_eq!(r[20], Slot::Strike);
    assert_eq!(game.score(), 20);
}

#[test]
fn tenth_gutter_then_ten_is_a_spare() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 18, 0);
    roll_all(&mut game, &[0, 10, 5]);
    let r = game.rolls();
    assert_eq!(r[18], Slot::Pins(0));
    assert_eq!(r[19], Slot::Spare);
    assert_eq!(r[20], Slot::Pins(5));
    assert_eq!(game.score(), 15);
}

// ----------------------------------------------------------------- validation

#[test]
fn refuses_impossible_pin_counts() {
    // The C++ test also rolled -1 (`Negative`); not expressible with `u8`.
    let mut game = ScoreCalculator::default();
    assert_eq!(game.roll(11), Err(RollError::TooManyPins));
    assert_eq!(game.roll(7), Ok(()));
    assert_eq!(game.roll(4), Err(RollError::TooManyPins));
    assert_eq!(game.roll(3), Ok(()));
    assert_eq!(game.score(), 10);
    assert_eq!(game.rolls()[1], Slot::Spare);
}

#[test]
fn regular_frame_gutter_then_ten_is_a_spare() {
    let mut game = ScoreCalculator::default();
    roll_all(&mut game, &[0, 10]);
    assert_eq!(game.rolls()[0], Slot::Pins(0));
    assert_eq!(game.rolls()[1], Slot::Spare);
}

#[test]
fn no_pins_left_after_the_game() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 20, 0);
    assert!(game.is_over());
    assert_eq!(game.pins_left(), 0);
}

// --------------------------------------------------------------------- totals

#[test]
fn frame_total_is_blank_until_its_bonus_is_known() {
    let mut game = ScoreCalculator::default();
    roll_all(&mut game, &[10, 3]);
    assert_eq!(game.frame_totals()[0], None);
    assert_eq!(game.frame_totals()[1], None);
    assert_eq!(game.score(), 16);
    roll_all(&mut game, &[4]);
    assert_eq!(game.frame_totals()[0], Some(17));
    assert_eq!(game.frame_totals()[1], Some(24));
    assert_eq!(game.frame_totals()[2], None);
}

#[test]
fn frame_score_clamps_the_frame_index() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 20, 1);
    assert_eq!(game.frame_score(10), 20);
    assert_eq!(game.frame_score(usize::MAX), 20);
    // The C++ test also asked for frame -1 (clamped to 0, giving 2); `usize` has no negatives.
    assert_eq!(game.frame_score(0), 2);
}

// --------------------------------------------------------------------- frames

#[test]
fn empty_sheet_has_ten_blank_frames() {
    let game = ScoreCalculator::default();
    let views = frames_of(&game);
    for (f, view) in views.iter().enumerate() {
        assert_eq!(view.index, f);
        assert_eq!(view.roll_count, if f == FRAMES - 1 { 3 } else { 2 });
        assert_eq!(view.is_last(), f == FRAMES - 1);
        assert_eq!(view.rolls[0], Slot::NotPlayed);
        assert_eq!(view.total, None);
    }
}

#[test]
fn frames_follow_the_21_slot_encoding() {
    let mut game = ScoreCalculator::default();
    roll_all(&mut game, &[10, 7, 3]);
    let views = frames_of(&game);
    assert_eq!(views[0].rolls[0], Slot::Strike);
    assert_eq!(views[0].rolls[1], Slot::Empty);
    assert_eq!(views[0].total, Some(20));
    assert_eq!(views[1].rolls[0], Slot::Pins(7));
    assert_eq!(views[1].rolls[1], Slot::Spare);
    assert_eq!(views[1].total, None);
}

#[test]
fn tenth_frame_has_three_rolls() {
    let mut game = ScoreCalculator::default();
    roll_n_times(&mut game, 18, 0);
    roll_all(&mut game, &[7, 3, 5]);
    let views = frames_of(&game);
    let last: &FrameView = &views[9];
    assert_eq!(last.roll_count, 3);
    assert_eq!(last.rolls[0], Slot::Pins(7));
    assert_eq!(last.rolls[1], Slot::Spare);
    assert_eq!(last.rolls[2], Slot::Pins(5));
    assert_eq!(last.total, Some(15));
}

#[test]
fn short_vectors_read_as_not_played() {
    let views = frames(&[Slot::Pins(3)], &[]);
    assert_eq!(views[0].rolls[0], Slot::Pins(3));
    assert_eq!(views[0].rolls[1], Slot::NotPlayed);
    assert_eq!(views[9].rolls[2], Slot::NotPlayed);
    assert_eq!(views[0].total, None);
}
