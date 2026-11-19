/// What one roll slot of the score sheet shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Slot {
    /// Not played yet.
    #[default]
    NotPlayed,
    /// The cell after a strike in frames 1 to 9.
    Empty,
    Strike,
    Spare,
    /// A plain pin count; `Pins(0)` is a miss.
    Pins(u8),
}

impl Slot {
    /// True for slots with nothing to show: not played yet, or the cell after a strike.
    #[must_use]
    pub fn is_blank(self) -> bool {
        matches!(self, Self::NotPlayed | Self::Empty)
    }
}

/// Score sheet notation of a roll slot: blank is "", a strike "X", a spare "/", a miss "-",
/// otherwise the pin count.
///
/// ```
/// use bowlingbox_score::{notation, Slot};
///
/// assert_eq!(notation(Slot::Strike), "X");
/// assert_eq!(notation(Slot::Pins(0)), "-");
/// assert_eq!(notation(Slot::Empty), "");
/// ```
#[must_use]
pub fn notation(slot: Slot) -> String {
    match slot {
        Slot::NotPlayed | Slot::Empty => String::new(),
        Slot::Strike => "X".to_string(),
        Slot::Spare => "/".to_string(),
        Slot::Pins(0) => "-".to_string(),
        Slot::Pins(pins) => pins.to_string(),
    }
}

/// Notation of a frame total: "" while it is not final, otherwise the number ("0" for zero).
#[must_use]
pub fn total_notation(total: Option<u16>) -> String {
    total.map_or_else(String::new, |total| total.to_string())
}
