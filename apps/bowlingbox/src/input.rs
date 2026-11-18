//! Pure mapping from keys, pointer events and the lane phase to commands.

use std::collections::HashSet;

use bowlingbox_lane::Phase;
use eframe::egui::{Key, Modifiers};

use crate::command::Command;

/// Metres per key event.
pub const KEY_OFFSET_STEP: f32 = 0.02;
/// Degrees per key event.
pub const KEY_ANGLE_STEP: f32 = 0.25;
/// Metres across the whole canvas.
pub const MOUSE_OFFSET_SPAN: f32 = 1.2;
/// Degrees per pixel.
pub const MOUSE_ANGLE_STEP: f32 = 0.05;

pub fn key_down(key: Key, modifiers: Modifiers, phase: Phase) -> Option<Command> {
    let can_aim = matches!(phase, Phase::Aiming | Phase::Charging);
    match key {
        Key::Escape if modifiers.is_none() => Some(Command::Quit),
        Key::Q if modifiers.matches_exact(Modifiers::COMMAND) => Some(Command::Quit),
        Key::N => Some(Command::NewGame),
        Key::T => Some(Command::CycleTheme),
        Key::M => Some(Command::ToggleMute),
        Key::B => Some(Command::ToggleAmbience),
        // `=` is the unshifted `+` on US layouts.
        Key::Plus | Key::Equals => Some(Command::VolumeUp),
        Key::Minus => Some(Command::VolumeDown),
        Key::ArrowLeft if can_aim => Some(Command::MoveAim(-KEY_OFFSET_STEP)),
        Key::ArrowRight if can_aim => Some(Command::MoveAim(KEY_OFFSET_STEP)),
        Key::A if can_aim => Some(Command::RotateAim(-KEY_ANGLE_STEP)),
        Key::D if can_aim => Some(Command::RotateAim(KEY_ANGLE_STEP)),
        Key::Space if phase == Phase::Aiming => Some(Command::StartCharge),
        _ => None,
    }
}

pub fn key_up(key: Key, phase: Phase) -> Option<Command> {
    (key == Key::Space && phase == Phase::Charging).then_some(Command::Release)
}

pub fn pointer_down(primary: bool, phase: Phase) -> Option<Command> {
    (primary && phase == Phase::Aiming).then_some(Command::StartCharge)
}

pub fn pointer_up(primary: bool, phase: Phase) -> Option<Command> {
    (primary && phase == Phase::Charging).then_some(Command::Release)
}

/// The pointer moved `dx` pixels. While aiming, a secondary-button drag turns the aim and a move
/// with no button shifts the ball; a primary-button drag (charging) does nothing.
pub fn pointer_move(
    dx: f32,
    primary_down: bool,
    secondary_down: bool,
    canvas_width: f32,
    phase: Phase,
) -> Option<Command> {
    if dx == 0.0 || primary_down || phase != Phase::Aiming {
        return None;
    }
    if secondary_down {
        return Some(Command::RotateAim(MOUSE_ANGLE_STEP * dx));
    }
    // Minimised: no meaningful scale.
    (canvas_width > 0.0).then(|| Command::MoveAim(MOUSE_OFFSET_SPAN * dx / canvas_width))
}

/// Theme, mute and ambience switch state, so they fire once per key press.
pub fn is_toggle(command: Command) -> bool {
    matches!(
        command,
        Command::CycleTheme | Command::ToggleMute | Command::ToggleAmbience
    )
}

/// Key auto-repeat sends the press again while a key is held; for toggles that would flash
/// through the themes.
#[derive(Debug, Default)]
pub struct KeyLatch {
    held: HashSet<Key>,
}

impl KeyLatch {
    /// False for a repeated press of a held toggle key, true for everything else.
    pub fn accept(&mut self, key: Key, command: Command) -> bool {
        !is_toggle(command) || self.held.insert(key)
    }

    pub fn release(&mut self, key: Key) {
        self.held.remove(&key);
    }

    /// Focus lost: the key releases will not arrive.
    pub fn clear(&mut self) {
        self.held.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PHASES: [Phase; 6] = [
        Phase::Aiming,
        Phase::Charging,
        Phase::Rolling,
        Phase::Settling,
        Phase::FrameDone,
        Phase::GameOver,
    ];
    const NONE: Modifiers = Modifiers::NONE;

    #[test]
    fn arrows_move_while_aiming_or_charging() {
        assert_eq!(
            key_down(Key::ArrowLeft, NONE, Phase::Aiming),
            Some(Command::MoveAim(-KEY_OFFSET_STEP))
        );
        assert_eq!(
            key_down(Key::ArrowRight, NONE, Phase::Charging),
            Some(Command::MoveAim(KEY_OFFSET_STEP))
        );
        assert_eq!(key_down(Key::ArrowLeft, NONE, Phase::Rolling), None);
    }

    #[test]
    fn a_and_d_rotate_the_aim() {
        assert_eq!(
            key_down(Key::A, NONE, Phase::Aiming),
            Some(Command::RotateAim(-KEY_ANGLE_STEP))
        );
        assert_eq!(
            key_down(Key::D, NONE, Phase::Aiming),
            Some(Command::RotateAim(KEY_ANGLE_STEP))
        );
        assert_eq!(key_down(Key::D, NONE, Phase::GameOver), None);
    }

    #[test]
    fn space_charges_and_releases() {
        assert_eq!(
            key_down(Key::Space, NONE, Phase::Aiming),
            Some(Command::StartCharge)
        );
        // Key repeat.
        assert_eq!(key_down(Key::Space, NONE, Phase::Charging), None);
        assert_eq!(key_up(Key::Space, Phase::Charging), Some(Command::Release));
        assert_eq!(key_up(Key::Space, Phase::Aiming), None);
    }

    #[test]
    fn new_game_and_quit_work_in_any_phase() {
        assert_eq!(
            key_down(Key::N, NONE, Phase::Rolling),
            Some(Command::NewGame)
        );
        assert_eq!(
            key_down(Key::N, Modifiers::COMMAND, Phase::GameOver),
            Some(Command::NewGame)
        );
        assert_eq!(
            key_down(Key::Escape, NONE, Phase::Settling),
            Some(Command::Quit)
        );
        assert_eq!(
            key_down(Key::Q, Modifiers::COMMAND, Phase::Aiming),
            Some(Command::Quit)
        );
        assert_eq!(key_down(Key::Q, NONE, Phase::Aiming), None);
        assert_eq!(
            key_down(Key::Q, Modifiers::COMMAND | Modifiers::SHIFT, Phase::Aiming),
            None
        );
    }

    #[test]
    fn pointer_buttons_mirror_space() {
        assert_eq!(
            pointer_down(true, Phase::Aiming),
            Some(Command::StartCharge)
        );
        assert_eq!(pointer_down(false, Phase::Aiming), None);
        assert_eq!(pointer_up(true, Phase::Charging), Some(Command::Release));
        assert_eq!(pointer_up(true, Phase::Rolling), None);
    }

    #[test]
    fn pointer_move_shifts_or_turns_only_while_aiming() {
        let Some(Command::MoveAim(metres)) =
            pointer_move(100.0, false, false, 1000.0, Phase::Aiming)
        else {
            panic!("expected MoveAim");
        };
        // 1.2 m across the canvas.
        assert!((metres - 0.12).abs() < 1e-5);
        let Some(Command::RotateAim(degrees)) =
            pointer_move(10.0, false, true, 1000.0, Phase::Aiming)
        else {
            panic!("expected RotateAim");
        };
        // 0.05 degrees per pixel.
        assert!((degrees - 0.5).abs() < 1e-5);
        assert_eq!(pointer_move(10.0, true, false, 1000.0, Phase::Aiming), None);
        assert_eq!(
            pointer_move(10.0, false, false, 1000.0, Phase::Rolling),
            None
        );
        assert_eq!(pointer_move(0.0, false, false, 1000.0, Phase::Aiming), None);
    }

    #[test]
    fn zero_width_canvas_does_not_move_the_ball() {
        assert_eq!(pointer_move(50.0, false, false, 0.0, Phase::Aiming), None);
        assert_eq!(pointer_move(50.0, false, false, -1.0, Phase::Aiming), None);
    }

    #[test]
    fn t_cycles_the_theme_in_every_phase() {
        for phase in PHASES {
            assert_eq!(key_down(Key::T, NONE, phase), Some(Command::CycleTheme));
        }
    }

    #[test]
    fn m_toggles_mute_in_every_phase() {
        for phase in PHASES {
            assert_eq!(key_down(Key::M, NONE, phase), Some(Command::ToggleMute));
            assert_eq!(key_up(Key::M, phase), None);
        }
    }

    #[test]
    fn plus_minus_and_b_control_the_sound_in_every_phase() {
        for phase in PHASES {
            for key in [Key::Plus, Key::Equals] {
                assert_eq!(key_down(key, NONE, phase), Some(Command::VolumeUp));
            }
            assert_eq!(key_down(Key::Minus, NONE, phase), Some(Command::VolumeDown));
            assert_eq!(key_down(Key::B, NONE, phase), Some(Command::ToggleAmbience));
        }
    }

    #[test]
    fn toggles_fire_once_per_press() {
        let mut latch = KeyLatch::default();
        assert!(latch.accept(Key::T, Command::CycleTheme));
        // Auto-repeat.
        assert!(!latch.accept(Key::T, Command::CycleTheme));
        latch.release(Key::T);
        assert!(latch.accept(Key::T, Command::CycleTheme));
    }

    #[test]
    fn movement_keys_repeat() {
        let mut latch = KeyLatch::default();
        let left = Command::MoveAim(-KEY_OFFSET_STEP);
        assert!(latch.accept(Key::ArrowLeft, left));
        assert!(latch.accept(Key::ArrowLeft, left));
    }

    #[test]
    fn keys_latch_independently() {
        let mut latch = KeyLatch::default();
        assert!(latch.accept(Key::M, Command::ToggleMute));
        assert!(latch.accept(Key::B, Command::ToggleAmbience));
        assert!(!latch.accept(Key::M, Command::ToggleMute));
    }

    #[test]
    fn clear_forgets_held_keys() {
        let mut latch = KeyLatch::default();
        assert!(latch.accept(Key::T, Command::CycleTheme));
        latch.clear();
        assert!(latch.accept(Key::T, Command::CycleTheme));
    }

    #[test]
    fn only_theme_mute_and_ambience_are_toggles() {
        assert!(is_toggle(Command::CycleTheme));
        assert!(is_toggle(Command::ToggleMute));
        assert!(is_toggle(Command::ToggleAmbience));
        assert!(!is_toggle(Command::VolumeUp));
        assert!(!is_toggle(Command::NewGame));
    }
}
