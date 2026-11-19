//! The words shown to the player: hints, results, the status line and the help text. Pure
//! functions, port of the C++ `HudText`.

#![allow(
    dead_code,
    reason = "the HUD drawing that uses it comes in the next task"
)]

use bowlingbox_lane::{Phase, Snapshot};
use bowlingbox_render::{THEMES, theme};
use bowlingbox_score::{PINS, ScoreSheet, Slot};

use crate::sound_settings::SoundSettings;

/// The hint under the 3D view for the phase; empty while nothing needs saying.
pub fn hint(phase: Phase) -> &'static str {
    match phase {
        Phase::Aiming => "<- -> move  A D angle  Space charge  T theme",
        Phase::Charging => "release to throw",
        Phase::GameOver => "N: new game   T: theme   M: mute   Esc: quit",
        Phase::Rolling | Phase::Settling | Phase::FrameDone => "",
    }
}

pub fn phase_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Aiming => "Aiming",
        Phase::Charging => "Charging",
        Phase::Rolling => "Rolling",
        Phase::Settling => "Settling",
        Phase::FrameDone => "Frame done",
        Phase::GameOver => "Game over",
    }
}

/// What the last throw was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThrowResult {
    None,
    Strike,
    Spare,
    Gutter,
    Pins,
}

/// What the last throw was, by the sheet's marks (a ten on a second ball is a spare);
/// [`ThrowResult::None`] outside `FrameDone`.
pub fn throw_result(s: &Snapshot, sheet: &dyn ScoreSheet) -> ThrowResult {
    let Some(pins) = s.last_pins_down.filter(|_| s.phase == Phase::FrameDone) else {
        return ThrowResult::None;
    };
    let last = sheet
        .rolls()
        .into_iter()
        .rev()
        .find(|slot| !slot.is_blank());
    match last {
        Some(Slot::Spare) => ThrowResult::Spare,
        Some(Slot::Strike) => ThrowResult::Strike,
        _ if pins == PINS => ThrowResult::Strike,
        _ if pins == 0 => ThrowResult::Gutter,
        _ => ThrowResult::Pins,
    }
}

/// "STRIKE!", "SPARE!", "Gutter", "7 pins", "Game over - score N" or "".
pub fn result(s: &Snapshot, sheet: &dyn ScoreSheet) -> String {
    if s.phase == Phase::GameOver {
        return format!("Game over - score {}", sheet.score());
    }
    match throw_result(s, sheet) {
        ThrowResult::Strike => "STRIKE!".to_owned(),
        ThrowResult::Spare => "SPARE!".to_owned(),
        ThrowResult::Gutter => "Gutter".to_owned(),
        ThrowResult::Pins => format!("{} pins", s.last_pins_down.unwrap_or(0)),
        ThrowResult::None => String::new(),
    }
}

/// The status bar line.
pub fn status(s: &Snapshot, sheet: &dyn ScoreSheet) -> String {
    let standing = s.pin_standing.iter().filter(|up| **up).count();
    format!(
        "{}   |   score {}   |   pins standing {standing}",
        phase_name(s.phase),
        sheet.score()
    )
}

/// The sound part of the status bar: `volume 80 %` or `sound muted`, plus `, no ambience`.
pub fn sound_status(settings: SoundSettings) -> String {
    let mut text = if settings.muted {
        "sound muted".to_owned()
    } else {
        format!("volume {} %", settings.volume)
    };
    if !settings.ambience {
        text.push_str(", no ambience");
    }
    text
}

/// Text of the Help > Controls dialog.
pub fn controls_help() -> &'static str {
    "Left / Right or mouse move: position the ball\n\
     A / D or right-drag: aim angle\n\
     Hold Space or left button: charge, release to throw\n\
     T: change the scenery\n\
     M: mute sound    + / -: volume    B: background ambience\n\
     N: new game    Esc / Ctrl+Q: quit"
}

/// The warning for `--theme=<name>` with an unknown name.
pub fn unknown_theme_message(name: &str) -> String {
    let names: Vec<&str> = THEMES.iter().map(|id| theme(*id).name).collect();
    let list = match names.as_slice() {
        [] => String::new(),
        [only] => (*only).to_owned(),
        [init @ .., last] => format!("{} or {last}", init.join(", ")),
    };
    format!("unknown theme '{name}'; use {list}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use bowlingbox_lane::Pose;
    use bowlingbox_score::{Game, ScoreCalculator};

    fn snap(phase: Phase) -> Snapshot {
        Snapshot {
            phase,
            ball: Pose::default(),
            pins: [Pose::default(); 10],
            pin_standing: [true; 10],
            pin_in_play: [true; 10],
            aim_offset: 0.0,
            aim_angle: 0.0,
            power: 0.0,
            last_pins_down: None,
        }
    }

    fn done(pins: u8) -> Snapshot {
        Snapshot {
            last_pins_down: Some(pins),
            ..snap(Phase::FrameDone)
        }
    }

    #[test]
    fn sound_status_shows_volume_mute_and_ambience() {
        let mut s = SoundSettings {
            volume: 70,
            ..SoundSettings::default()
        };
        assert_eq!(sound_status(s), "volume 70 %");
        s.muted = true;
        assert_eq!(sound_status(s), "sound muted");
        s.ambience = false;
        assert_eq!(sound_status(s), "sound muted, no ambience");
    }

    #[test]
    fn result_names_strikes_spares_and_gutters() {
        let mut sheet = ScoreCalculator::default();
        sheet.roll(10).unwrap();
        assert_eq!(result(&done(10), &sheet), "STRIKE!");
        sheet.roll(3).unwrap();
        assert_eq!(result(&done(3), &sheet), "3 pins");
        sheet.roll(7).unwrap();
        assert_eq!(result(&done(7), &sheet), "SPARE!");
        sheet.roll(0).unwrap();
        assert_eq!(result(&done(0), &sheet), "Gutter");
        assert_eq!(result(&snap(Phase::Aiming), &sheet), "");
        assert_eq!(
            result(&snap(Phase::GameOver), &sheet),
            "Game over - score 30"
        );
    }

    #[test]
    fn ten_pins_after_a_gutter_ball_is_a_spare() {
        let mut sheet = ScoreCalculator::default();
        sheet.roll(0).unwrap();
        sheet.roll(10).unwrap();
        assert_eq!(result(&done(10), &sheet), "SPARE!");
    }

    #[test]
    fn throw_result_ten_after_a_gutter_ball_is_a_spare() {
        let mut sheet = ScoreCalculator::default();
        sheet.roll(0).unwrap();
        assert_eq!(throw_result(&done(0), &sheet), ThrowResult::Gutter);
        sheet.roll(10).unwrap();
        assert_eq!(throw_result(&done(10), &sheet), ThrowResult::Spare);
    }

    #[test]
    fn throw_result_names_strikes_and_counts_and_nothing_outside_frame_done() {
        let mut sheet = ScoreCalculator::default();
        sheet.roll(10).unwrap();
        assert_eq!(throw_result(&done(10), &sheet), ThrowResult::Strike);
        sheet.roll(4).unwrap();
        assert_eq!(throw_result(&done(4), &sheet), ThrowResult::Pins);
        assert_eq!(
            throw_result(&snap(Phase::Rolling), &sheet),
            ThrowResult::None
        );
        // No throw yet: nothing to name even in FrameDone.
        assert_eq!(
            throw_result(&snap(Phase::FrameDone), &sheet),
            ThrowResult::None
        );
    }

    #[test]
    fn status_line_shows_phase_score_and_standing_pins() {
        let mut sheet = ScoreCalculator::default();
        sheet.roll(4).unwrap();
        let mut s = snap(Phase::Settling);
        s.pin_standing[0] = false;
        assert_eq!(
            status(&s, &sheet),
            "Settling   |   score 4   |   pins standing 9"
        );
    }

    #[test]
    fn controls_help_names_every_binding() {
        let help = controls_help();
        for key in [
            "Left / Right",
            "A / D",
            "Space",
            "N",
            "Esc",
            "Ctrl+Q",
            "T: change the scenery",
            "M: mute sound",
            "+ / -: volume",
            "B: background ambience",
        ] {
            assert!(help.contains(key), "{key}");
        }
    }

    #[test]
    fn hint_offers_the_theme_key_only_while_standing_still() {
        assert!(hint(Phase::Aiming).contains("T theme"));
        assert!(hint(Phase::GameOver).contains("T: theme"));
        assert_eq!(hint(Phase::Charging), "release to throw");
        assert_eq!(hint(Phase::Rolling), "");
    }

    #[test]
    fn phase_names_are_capitalised_words() {
        assert_eq!(phase_name(Phase::FrameDone), "Frame done");
        assert_eq!(phase_name(Phase::Aiming), "Aiming");
    }

    #[test]
    fn unknown_theme_message_lists_every_name() {
        assert_eq!(
            unknown_theme_message("space"),
            "unknown theme 'space'; use neon, classic or outdoor"
        );
        assert_eq!(
            unknown_theme_message(""),
            "unknown theme ''; use neon, classic or outdoor"
        );
    }
}
