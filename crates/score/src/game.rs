use std::fmt;

use crate::{FRAMES, PINS, ROLL_SLOTS, Slot};

/// Why a ball was refused. The game is unchanged then.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RollError {
    GameOver,
    TooManyPins,
}

impl fmt::Display for RollError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::GameOver => "the game is over",
            Self::TooManyPins => "more pins than are standing",
        })
    }
}

impl std::error::Error for RollError {}

/// Read-only view of a bowling game. Displays depend on this, not on a concrete calculator.
pub trait ScoreSheet {
    /// The 21 roll slots.
    fn rolls(&self) -> [Slot; ROLL_SLOTS];
    /// Running totals per frame. A frame stays `None` until it is final (two balls for an open
    /// frame, plus the bonus balls for a strike or spare).
    fn frame_totals(&self) -> [Option<u16>; FRAMES];
    /// The running total of the whole game.
    fn score(&self) -> u16;
    /// Running total through `frame` (clamped to 0..=9), counting the balls thrown so far.
    fn frame_score(&self, frame: usize) -> u16;
    /// Pins standing for the next ball; 0 once the game is over.
    fn pins_left(&self) -> u8;
    fn is_over(&self) -> bool;
}

/// A score sheet that also accepts rolls.
pub trait Game: ScoreSheet {
    /// Records a ball.
    ///
    /// # Errors
    ///
    /// [`RollError::GameOver`] when no ball is left (this wins over a bad count), or
    /// [`RollError::TooManyPins`] when `pins` exceeds the pins standing.
    fn roll(&mut self, pins: u8) -> Result<(), RollError>;
    fn reset(&mut self);
}

/// Ten-pin score calculator (codingdojo bowling kata).
///
/// ```
/// use bowlingbox_score::{Game, ScoreCalculator, ScoreSheet};
///
/// let mut game = ScoreCalculator::default();
/// for pins in [5, 5, 10, 3, 4] {
///     game.roll(pins).unwrap();
/// }
/// // Spare (5 + 5 + 10), strike (10 + 3 + 4), open frame (3 + 4).
/// assert_eq!(game.score(), 44);
/// ```
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct ScoreCalculator {
    balls: [u8; ROLL_SLOTS],
    count: usize,
}

/// The frame being played (`FRAMES` once the game is over) and the pins standing.
struct Position {
    frame: usize,
    standing: u8,
}

impl ScoreCalculator {
    /// The ball at `index`, 0 if it was not thrown.
    fn at(&self, index: usize) -> u8 {
        if index < self.count {
            self.balls.get(index).copied().unwrap_or(0)
        } else {
            0
        }
    }

    /// Walks the thrown balls frame by frame to find the next ball. Frames 1 to 9 end after a
    /// strike or two balls. The tenth frame gets a third ball after a strike or a spare, and
    /// its rack is reset whenever it is cleared.
    fn position(&self) -> Position {
        let mut i = 0;
        for frame in 0..FRAMES - 1 {
            if i == self.count {
                return Position {
                    frame,
                    standing: PINS,
                };
            }
            if self.at(i) == PINS {
                i += 1;
                continue;
            }
            if i + 1 == self.count {
                return Position {
                    frame,
                    standing: PINS - self.at(i),
                };
            }
            i += 2;
        }
        let last = FRAMES - 1;
        let (first, second) = (self.at(i), self.at(i + 1));
        let standing_after = |pins: u8| if pins == PINS { PINS } else { PINS - pins };
        match self.count.saturating_sub(i) {
            0 => Position {
                frame: last,
                standing: PINS,
            },
            1 => Position {
                frame: last,
                standing: standing_after(first),
            },
            2 if first == PINS => Position {
                frame: last,
                standing: standing_after(second),
            },
            2 if first + second == PINS => Position {
                frame: last,
                standing: PINS,
            },
            _ => Position {
                frame: FRAMES,
                standing: 0,
            },
        }
    }

    /// Whether the frame starting at ball `i` is a strike or a spare, using the balls thrown.
    fn is_bonus_frame(&self, i: usize) -> bool {
        let strike = self.at(i) == PINS;
        strike || u16::from(self.at(i)) + u16::from(self.at(i + 1)) == u16::from(PINS)
    }

    fn frame_points(&self, i: usize) -> u16 {
        let bonus = if self.is_bonus_frame(i) {
            self.at(i + 2)
        } else {
            0
        };
        u16::from(self.at(i)) + u16::from(self.at(i + 1)) + u16::from(bonus)
    }

    /// Index of the first ball of the next frame.
    fn next_frame(&self, i: usize, frame: usize) -> usize {
        if self.at(i) == PINS && frame < FRAMES - 1 {
            i + 1
        } else {
            i + 2
        }
    }
}

impl ScoreSheet for ScoreCalculator {
    fn rolls(&self) -> [Slot; ROLL_SLOTS] {
        let mut out = [Slot::NotPlayed; ROLL_SLOTS];
        let mut i = 0;
        for frame in 0..FRAMES - 1 {
            if i >= self.count {
                break;
            }
            let slot = 2 * frame;
            if self.at(i) == PINS {
                out[slot] = Slot::Strike;
                out[slot + 1] = Slot::Empty;
                i += 1;
                continue;
            }
            out[slot] = Slot::Pins(self.at(i));
            if i + 1 < self.count {
                out[slot + 1] = if self.at(i) + self.at(i + 1) == PINS {
                    Slot::Spare
                } else {
                    Slot::Pins(self.at(i + 1))
                };
            }
            i += 2;
        }
        let mut standing = PINS;
        let mut fresh_rack = true;
        for slot in out.iter_mut().skip(2 * (FRAMES - 1)) {
            if i >= self.count {
                break;
            }
            let pins = self.at(i);
            *slot = if fresh_rack && pins == PINS {
                Slot::Strike
            } else if !fresh_rack && pins == standing {
                Slot::Spare
            } else {
                Slot::Pins(pins)
            };
            fresh_rack = pins == standing;
            standing = if fresh_rack { PINS } else { standing - pins };
            i += 1;
        }
        out
    }

    fn frame_totals(&self) -> [Option<u16>; FRAMES] {
        let mut out = [None; FRAMES];
        let (mut total, mut i) = (0, 0);
        for (frame, entry) in out.iter_mut().enumerate() {
            if i >= self.count {
                break;
            }
            let needed = if self.is_bonus_frame(i) { 3 } else { 2 };
            if i + needed > self.count {
                break;
            }
            total += self.frame_points(i);
            *entry = Some(total);
            i = self.next_frame(i, frame);
        }
        out
    }

    fn score(&self) -> u16 {
        self.frame_score(FRAMES - 1)
    }

    fn frame_score(&self, frame: usize) -> u16 {
        let last = frame.min(FRAMES - 1);
        let (mut total, mut i) = (0, 0);
        for f in 0..=last {
            if i >= self.count {
                break;
            }
            total += self.frame_points(i);
            i = self.next_frame(i, f);
        }
        total
    }

    fn pins_left(&self) -> u8 {
        self.position().standing
    }

    fn is_over(&self) -> bool {
        self.position().frame == FRAMES
    }
}

impl Game for ScoreCalculator {
    fn roll(&mut self, pins: u8) -> Result<(), RollError> {
        let position = self.position();
        if position.frame == FRAMES {
            return Err(RollError::GameOver);
        }
        if pins > position.standing {
            return Err(RollError::TooManyPins);
        }
        let slot = self.balls.get_mut(self.count).ok_or(RollError::GameOver)?;
        *slot = pins;
        self.count += 1;
        Ok(())
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}
