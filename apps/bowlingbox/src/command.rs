//! Window commands and the keys that trigger them. No window needed to test this.

use eframe::egui::{Key, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    NewGame,
    Quit,
}

/// The command a key press maps to, if any: Esc and Ctrl+Q quit.
pub fn command_for_key(key: Key, modifiers: Modifiers) -> Option<Command> {
    match key {
        Key::Escape if modifiers.is_none() => Some(Command::Quit),
        Key::Q if modifiers.matches_exact(Modifiers::COMMAND) => Some(Command::Quit),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_quits() {
        assert_eq!(
            command_for_key(Key::Escape, Modifiers::NONE),
            Some(Command::Quit)
        );
    }

    #[test]
    fn ctrl_q_quits() {
        assert_eq!(
            command_for_key(Key::Q, Modifiers::COMMAND),
            Some(Command::Quit)
        );
    }

    #[test]
    fn plain_q_and_other_keys_do_nothing() {
        assert_eq!(command_for_key(Key::Q, Modifiers::NONE), None);
        assert_eq!(command_for_key(Key::A, Modifiers::COMMAND), None);
        assert_eq!(command_for_key(Key::Enter, Modifiers::NONE), None);
    }

    #[test]
    fn q_with_extra_modifiers_does_nothing() {
        assert_eq!(
            command_for_key(Key::Q, Modifiers::COMMAND | Modifiers::SHIFT),
            None
        );
    }
}
