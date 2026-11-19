use crate::{FRAMES, ScoreSheet, Slot};

/// One frame of the score sheet, cut out of the 21 roll slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameView {
    /// 0 to 9.
    pub index: usize,
    /// Slots `2 * index`, `2 * index + 1` and, for the tenth frame, `2 * index + 2`.
    pub rolls: [Slot; 3],
    /// 3 for the tenth frame, otherwise 2.
    pub roll_count: usize,
    /// Running total, `None` while the frame is not final.
    pub total: Option<u16>,
}

impl FrameView {
    #[must_use]
    pub fn is_last(&self) -> bool {
        self.index == FRAMES - 1
    }
}

/// Splits the roll slots and frame totals into ten frames. Missing entries read as
/// [`Slot::NotPlayed`] and `None`.
#[must_use]
pub fn frames(rolls: &[Slot], totals: &[Option<u16>]) -> [FrameView; FRAMES] {
    std::array::from_fn(|index| {
        let roll_count = if index == FRAMES - 1 { 3 } else { 2 };
        let mut view_rolls = [Slot::NotPlayed; 3];
        for (r, slot) in view_rolls.iter_mut().take(roll_count).enumerate() {
            *slot = rolls.get(2 * index + r).copied().unwrap_or_default();
        }
        FrameView {
            index,
            rolls: view_rolls,
            roll_count,
            total: totals.get(index).copied().flatten(),
        }
    })
}

/// The frames of a score sheet.
#[must_use]
pub fn frames_of(sheet: &dyn ScoreSheet) -> [FrameView; FRAMES] {
    frames(&sheet.rolls(), &sheet.frame_totals())
}
