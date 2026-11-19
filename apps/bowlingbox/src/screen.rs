//! `--screen=<name>`: puts the window into a named state at start, for screenshots and manual
//! checks. It sets up that state without input and changes nothing else.

use bowlingbox_render::ThemeId;

use crate::dialogs::Dialog;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Screen {
    #[default]
    Game,
    Controls,
    About,
    SoundMenu,
    ThemeClassic,
    ThemeOutdoor,
}

/// What a screen sets up at start.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScreenState {
    pub dialog: Option<Dialog>,
    /// The Sound menu is shown open.
    pub sound_menu_open: bool,
    /// Replaces the `--theme` choice.
    pub theme: Option<ThemeId>,
    /// `--screen` was given: the run saves no settings.
    pub read_only: bool,
}

pub fn unknown_screen_message(name: &str) -> String {
    format!(
        "unknown screen '{name}'; use game, controls, about, sound-menu, theme-classic or theme-outdoor"
    )
}

impl Screen {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "game" => Self::Game,
            "controls" => Self::Controls,
            "about" => Self::About,
            "sound-menu" => Self::SoundMenu,
            "theme-classic" => Self::ThemeClassic,
            "theme-outdoor" => Self::ThemeOutdoor,
            _ => return None,
        })
    }

    pub fn state(self) -> ScreenState {
        let mut state = ScreenState::default();
        match self {
            Self::Game => {}
            Self::Controls => state.dialog = Some(Dialog::Controls),
            Self::About => state.dialog = Some(Dialog::About),
            Self::SoundMenu => state.sound_menu_open = true,
            Self::ThemeClassic => state.theme = Some(ThemeId::ClassicAlley),
            Self::ThemeOutdoor => state.theme = Some(ThemeId::Outdoor),
        }
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_parses() {
        for (name, screen) in [
            ("game", Screen::Game),
            ("controls", Screen::Controls),
            ("about", Screen::About),
            ("sound-menu", Screen::SoundMenu),
            ("theme-classic", Screen::ThemeClassic),
            ("theme-outdoor", Screen::ThemeOutdoor),
        ] {
            assert_eq!(Screen::parse(name), Some(screen), "{name}");
        }
        assert_eq!(Screen::parse("nope"), None);
        assert_eq!(Screen::parse(""), None);
    }

    #[test]
    fn the_unknown_name_message_lists_the_names() {
        assert_eq!(
            unknown_screen_message("x"),
            "unknown screen 'x'; use game, controls, about, sound-menu, theme-classic or theme-outdoor"
        );
    }

    #[test]
    fn each_screen_sets_only_its_own_state() {
        assert_eq!(Screen::Game.state(), ScreenState::default());
        assert_eq!(Screen::Controls.state().dialog, Some(Dialog::Controls));
        assert_eq!(Screen::About.state().dialog, Some(Dialog::About));
        assert_eq!(
            Screen::SoundMenu.state(),
            ScreenState {
                sound_menu_open: true,
                ..ScreenState::default()
            }
        );
        assert_eq!(
            Screen::ThemeClassic.state(),
            ScreenState {
                theme: Some(ThemeId::ClassicAlley),
                ..ScreenState::default()
            }
        );
        assert_eq!(Screen::ThemeOutdoor.state().theme, Some(ThemeId::Outdoor));
    }
}
