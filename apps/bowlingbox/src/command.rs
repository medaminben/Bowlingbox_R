//! Player intents, independent of the input device, and how the window routes them.

/// A player intent. Amounts are metres for `MoveAim` and degrees for `RotateAim`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    MoveAim(f32),
    RotateAim(f32),
    StartCharge,
    Release,
    NewGame,
    CycleTheme,
    ToggleMute,
    VolumeUp,
    VolumeDown,
    ToggleAmbience,
    /// Ctrl+Q: leave the game.
    Quit,
    /// Esc: close what is open, else leave the game.
    Escape,
}

/// Which part of the game handles a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandTarget {
    Lane,
    View,
    Sound,
    App,
}

impl Command {
    /// The owner of the command. No wildcard arm: a new command does not compile until it has one.
    pub fn target(self) -> CommandTarget {
        match self {
            Self::MoveAim(_)
            | Self::RotateAim(_)
            | Self::StartCharge
            | Self::Release
            | Self::NewGame => CommandTarget::Lane,
            Self::CycleTheme => CommandTarget::View,
            Self::ToggleMute | Self::VolumeUp | Self::VolumeDown | Self::ToggleAmbience => {
                CommandTarget::Sound
            }
            Self::Quit | Self::Escape => CommandTarget::App,
        }
    }
}

/// What the window does with a command. While the Controls or About dialog is open the lane keeps
/// running but game, theme and sound commands are dropped; Esc closes the dialog and Ctrl+Q still
/// quits. While a menu is open, a quit key only dismisses it; the next press quits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Route {
    Run(Command),
    CloseDialog,
    /// The open menu closes itself on Esc; nothing else happens.
    Ignore,
}

pub fn route(command: Command, dialog_open: bool, menu_open: bool) -> Route {
    match command {
        Command::Escape if dialog_open => Route::CloseDialog,
        Command::Escape | Command::Quit if menu_open => Route::Ignore,
        Command::Escape => Route::Run(Command::Quit),
        Command::Release => Route::Run(Command::Release),
        other if dialog_open && other.target() != CommandTarget::App => Route::Ignore,
        other => Route::Run(other),
    }
}

/// Whether a menu click on `command` acts. A click is an explicit choice, so it skips `route`
/// (the menu is still open in that frame); only a dialog blocks the game items. Quit always acts.
pub fn menu_click_allowed(command: Command, dialog_open: bool) -> bool {
    !dialog_open || command.target() == CommandTarget::App || command == Command::Release
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_has_one_owner() {
        for command in [
            Command::MoveAim(0.1),
            Command::RotateAim(1.0),
            Command::StartCharge,
            Command::Release,
            Command::NewGame,
        ] {
            assert_eq!(command.target(), CommandTarget::Lane);
        }
        assert_eq!(Command::CycleTheme.target(), CommandTarget::View);
        for command in [
            Command::ToggleMute,
            Command::VolumeUp,
            Command::VolumeDown,
            Command::ToggleAmbience,
        ] {
            assert_eq!(command.target(), CommandTarget::Sound);
        }
        assert_eq!(Command::Quit.target(), CommandTarget::App);
        assert_eq!(Command::Escape.target(), CommandTarget::App);
    }

    const GAME: [Command; 9] = [
        Command::MoveAim(0.1),
        Command::RotateAim(1.0),
        Command::StartCharge,
        Command::NewGame,
        Command::CycleTheme,
        Command::ToggleMute,
        Command::VolumeUp,
        Command::VolumeDown,
        Command::ToggleAmbience,
    ];

    #[test]
    fn escape_closes_a_dialog_first() {
        assert_eq!(route(Command::Escape, true, false), Route::CloseDialog);
        assert_eq!(route(Command::Escape, true, true), Route::CloseDialog);
    }

    #[test]
    fn ctrl_q_still_quits_with_a_dialog_open() {
        assert_eq!(route(Command::Quit, true, false), Route::Run(Command::Quit));
    }

    #[test]
    fn no_game_command_reaches_the_lane_behind_a_dialog() {
        for command in GAME {
            assert_eq!(route(command, true, false), Route::Ignore, "{command:?}");
        }
    }

    #[test]
    fn a_release_always_passes_so_a_throw_never_sticks_in_charging() {
        assert_eq!(
            route(Command::Release, true, false),
            Route::Run(Command::Release)
        );
        assert_eq!(
            route(Command::Release, true, true),
            Route::Run(Command::Release)
        );
    }

    #[test]
    fn menu_clicks_bypass_the_popup_route() {
        // The menu is still open in the click frame; Quit must act.
        assert!(menu_click_allowed(Command::Quit, false));
        assert!(menu_click_allowed(Command::Quit, true));
        for command in GAME {
            assert!(menu_click_allowed(command, false), "{command:?}");
        }
    }

    #[test]
    fn menu_clicks_on_game_items_do_nothing_behind_a_dialog() {
        for command in GAME {
            assert!(!menu_click_allowed(command, true), "{command:?}");
        }
    }

    #[test]
    fn escape_quits_when_nothing_is_open() {
        assert_eq!(
            route(Command::Escape, false, false),
            Route::Run(Command::Quit)
        );
    }

    #[test]
    fn a_quit_key_does_not_quit_while_a_menu_is_open() {
        assert_eq!(route(Command::Escape, false, true), Route::Ignore);
        assert_eq!(route(Command::Quit, false, true), Route::Ignore);
    }

    #[test]
    fn commands_run_when_nothing_is_open() {
        assert_eq!(
            route(Command::Quit, false, false),
            Route::Run(Command::Quit)
        );
        for command in GAME {
            assert_eq!(route(command, false, true), Route::Run(command));
        }
    }
}
