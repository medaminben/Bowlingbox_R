//! What the scoreboard shows, without drawing: the text of each cell and the geometry of the
//! 10-frame grid. Port of the C++ `ScoreboardLayout`, on the score crate's notation.

use bowlingbox_score::{FRAMES, ROLL_SLOTS, ScoreSheet, frames_of, notation, total_notation};

/// One frame of the board: the marks per ball box and the running total.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameCell {
    /// Two marks, three in the tenth frame; "" for a box that shows nothing.
    pub marks: Vec<String>,
    /// The running total, "" while the frame is not final.
    pub total: String,
}

/// The ten frame cells of `sheet`.
pub fn frames(sheet: &dyn ScoreSheet) -> [FrameCell; FRAMES] {
    frames_of(sheet).map(|frame| FrameCell {
        marks: frame.rolls[..frame.roll_count]
            .iter()
            .map(|slot| notation(*slot))
            .collect(),
        total: total_notation(frame.total),
    })
}

/// The grand total line, "Total 20".
pub fn total_text(sheet: &dyn ScoreSheet) -> String {
    format!("Total {}", sheet.score())
}

/// A rectangle in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Geometry of the grid: frames 0..=8 are two cells wide, frame 9 is three; the top row holds
/// the rolls, the bottom row the score.
#[derive(Clone, Copy, Debug)]
pub struct ScoreboardLayout {
    pub origin_x: f32,
    pub origin_y: f32,
    pub cell: f32,
}

#[allow(clippy::cast_precision_loss)] // at most 21
impl ScoreboardLayout {
    pub fn frame_rect(&self, frame: usize) -> Rect {
        let frame = frame.min(FRAMES - 1);
        let width = if frame == FRAMES - 1 { 3.0 } else { 2.0 };
        Rect {
            x: self.origin_x + 2.0 * self.cell * frame as f32,
            y: self.origin_y,
            w: width * self.cell,
            h: 2.0 * self.cell,
        }
    }

    /// The box of roll slot `roll` (0..=20), as in the 21-slot sheet.
    pub fn roll_cell_rect(&self, roll: usize) -> Rect {
        let frame = (roll / 2).min(FRAMES - 1);
        let within = roll - 2 * frame;
        let f = self.frame_rect(frame);
        Rect {
            x: f.x + self.cell * within as f32,
            y: f.y,
            w: self.cell,
            h: self.cell,
        }
    }

    pub fn score_rect(&self, frame: usize) -> Rect {
        let f = self.frame_rect(frame);
        Rect {
            x: f.x,
            y: f.y + self.cell,
            w: f.w,
            h: self.cell,
        }
    }

    pub fn total_width(&self) -> f32 {
        self.cell * ROLL_SLOTS as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bowlingbox_score::{Game, ScoreCalculator};

    #[test]
    fn cells_carry_marks_totals_and_the_grand_total() {
        // C++ painter_writes_marks_scores_and_total: strike, then 3 and a spare.
        let mut sheet = ScoreCalculator::default();
        for pins in [10, 3, 7, 4] {
            sheet.roll(pins).unwrap();
        }
        let cells = frames(&sheet);
        assert_eq!(cells[0].marks, ["X", ""]);
        assert_eq!(cells[0].total, "20");
        assert_eq!(cells[1].marks, ["3", "/"]);
        assert_eq!(cells[1].total, "34");
        assert_eq!(cells[2].marks, ["4", ""]);
        assert_eq!(cells[2].total, "");
        assert_eq!(cells[9].marks, ["", "", ""]);
        assert_eq!(total_text(&sheet), "Total 38");
    }

    #[test]
    fn an_empty_sheet_is_blank_with_a_zero_total() {
        let sheet = ScoreCalculator::default();
        let cells = frames(&sheet);
        assert!(cells.iter().all(|c| c.total.is_empty()));
        assert!(cells.iter().flat_map(|c| &c.marks).all(String::is_empty));
        assert_eq!(cells[0].marks.len(), 2);
        assert_eq!(cells[9].marks.len(), 3);
        assert_eq!(total_text(&sheet), "Total 0");
    }

    #[test]
    fn glyphs_follow_the_bowlingbox_encoding() {
        // C++ glyphs_follow_bowlingbox_encoding, through the sheet: miss "-", spare "/",
        // strike "X", a zero total "0", three digits for a perfect game.
        let mut sheet = ScoreCalculator::default();
        for pins in [0, 0, 7, 3] {
            sheet.roll(pins).unwrap();
        }
        let cells = frames(&sheet);
        assert_eq!(cells[0].marks, ["-", "-"]);
        assert_eq!(cells[0].total, "0");
        assert_eq!(cells[1].marks, ["7", "/"]);

        let mut perfect = ScoreCalculator::default();
        for _ in 0..12 {
            perfect.roll(10).unwrap();
        }
        let cells = frames(&perfect);
        assert_eq!(cells[9].marks, ["X", "X", "X"]);
        assert_eq!(cells[9].total, "300");
        assert_eq!(total_text(&perfect), "Total 300");
    }

    #[test]
    fn the_tenth_frame_shows_three_boxes() {
        let mut sheet = ScoreCalculator::default();
        for _ in 0..18 {
            sheet.roll(0).unwrap();
        }
        for pins in [4, 6, 5] {
            sheet.roll(pins).unwrap();
        }
        let cells = frames(&sheet);
        assert_eq!(cells[9].marks, ["4", "/", "5"]);
        assert_eq!(cells[9].total, "15");
    }

    /// Nine gutter frames, then the given balls of the tenth.
    fn tenth(balls: &[u8]) -> ScoreCalculator {
        let mut sheet = ScoreCalculator::default();
        for _ in 0..18 {
            sheet.roll(0).unwrap();
        }
        for pins in balls {
            sheet.roll(*pins).unwrap();
        }
        sheet
    }

    #[test]
    fn tenth_frame_strike_strike_seven() {
        let cells = frames(&tenth(&[10, 10, 7]));
        assert_eq!(cells[9].marks, ["X", "X", "7"]);
        assert_eq!(cells[9].total, "27");
    }

    #[test]
    fn tenth_frame_strike_then_spare() {
        let cells = frames(&tenth(&[10, 3, 7]));
        assert_eq!(cells[9].marks, ["X", "3", "/"]);
        assert_eq!(cells[9].total, "20");
    }

    #[test]
    fn tenth_frame_spare_then_strike() {
        let cells = frames(&tenth(&[5, 5, 10]));
        assert_eq!(cells[9].marks, ["5", "/", "X"]);
        assert_eq!(cells[9].total, "20");
    }

    #[test]
    fn the_tenth_frame_total_waits_for_its_bonus_ball() {
        assert_eq!(frames(&tenth(&[10, 10]))[9].total, "");
        assert_eq!(frames(&tenth(&[5, 5]))[9].total, "");
        assert_eq!(frames(&tenth(&[10, 10]))[9].marks, ["X", "X", ""]);
    }

    #[test]
    fn frames_are_two_cells_wide_except_the_last() {
        let l = ScoreboardLayout {
            origin_x: 10.0,
            origin_y: 20.0,
            cell: 30.0,
        };
        let f0 = l.frame_rect(0);
        assert_eq!((f0.x, f0.y, f0.w, f0.h), (10.0, 20.0, 60.0, 60.0));
        let f9 = l.frame_rect(9);
        assert_eq!((f9.x, f9.w), (10.0 + 9.0 * 60.0, 90.0));
        assert_eq!(l.total_width(), 21.0 * 30.0);
    }

    #[test]
    fn roll_cells_map_the_21_slot_encoding() {
        let l = ScoreboardLayout {
            origin_x: 0.0,
            origin_y: 0.0,
            cell: 10.0,
        };
        let r0 = l.roll_cell_rect(0);
        assert_eq!((r0.x, r0.w), (0.0, 10.0));
        assert_eq!(l.roll_cell_rect(3).x, 30.0);
        assert_eq!(l.roll_cell_rect(20).x, 180.0 + 20.0);
        let s4 = l.score_rect(4);
        assert_eq!((s4.x, s4.y, s4.w), (80.0, 10.0, 20.0));
    }
}
