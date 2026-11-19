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

/// What the window does with a command. While the About window or a menu is open, a quit key
/// only dismisses it; the next press quits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Run(Command),
    CloseAbout,
    /// The open menu closes itself on Esc; nothing else happens.
    Ignore,
}

pub fn route(command: Command, about_open: bool, menu_open: bool) -> Route {
    match command {
        Command::Quit if about_open => Route::CloseAbout,
        Command::Quit if menu_open => Route::Ignore,
        other => Route::Run(other),
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

    #[test]
    fn quit_closes_the_about_window_first() {
        assert_eq!(route(Command::Quit, true, false), Route::CloseAbout);
        assert_eq!(route(Command::Quit, true, true), Route::CloseAbout);
    }

    #[test]
    fn quit_does_not_quit_while_a_menu_is_open() {
        assert_eq!(route(Command::Quit, false, true), Route::Ignore);
    }

    #[test]
    fn commands_run_when_nothing_is_open() {
        assert_eq!(
            route(Command::Quit, false, false),
            Route::Run(Command::Quit)
        );
        assert_eq!(
            route(Command::NewGame, true, true),
            Route::Run(Command::NewGame)
        );
    }
}
