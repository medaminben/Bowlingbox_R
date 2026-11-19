//! The menu bar, as in the C++ game: labels with their shortcut text and the enabled and
//! checked states of the Sound items.

use bowlingbox_render::{THEMES, ThemeId, theme};
use eframe::egui;

use crate::command::Command;
use crate::sound_settings::SoundSettings;

pub const NEW_GAME: &str = "New Game";
pub const NEW_GAME_SHORTCUT: &str = "Ctrl+N";
pub const QUIT: &str = "Quit";
pub const QUIT_SHORTCUT: &str = "Ctrl+Q";
pub const MUTE: &str = "Mute (M)";
pub const VOLUME_UP: &str = "Volume up (+)";
pub const VOLUME_DOWN: &str = "Volume down (-)";
pub const AMBIENCE: &str = "Background ambience (B)";

/// What the Sound menu shows for the current settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundMenu {
    pub muted: bool,
    pub ambience: bool,
    pub volume: VolumeButtons,
}

/// Which of the two volume items can be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeButtons {
    pub up_enabled: bool,
    pub down_enabled: bool,
}

pub fn sound_menu(settings: SoundSettings) -> SoundMenu {
    SoundMenu {
        muted: settings.muted,
        ambience: settings.ambience,
        volume: VolumeButtons {
            up_enabled: settings.volume < 100,
            down_enabled: settings.volume > 0,
        },
    }
}

/// What a click in the menu asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuAction {
    Command(Command),
    Theme(ThemeId),
    Controls,
    About,
}

/// Draws the menu bar and returns what was chosen, if anything.
pub fn show(ui: &mut egui::Ui, current: ThemeId, settings: SoundSettings) -> Option<MenuAction> {
    let sound = sound_menu(settings);
    let mut chosen = None;
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("Game", |ui| {
            if ui
                .add(egui::Button::new(NEW_GAME).shortcut_text(NEW_GAME_SHORTCUT))
                .clicked()
            {
                chosen = Some(MenuAction::Command(Command::NewGame));
                ui.close();
            }
            ui.separator();
            if ui
                .add(egui::Button::new(QUIT).shortcut_text(QUIT_SHORTCUT))
                .clicked()
            {
                chosen = Some(MenuAction::Command(Command::Quit));
                ui.close();
            }
        });
        ui.menu_button("View", |ui| {
            ui.menu_button("Theme", |ui| {
                for id in THEMES {
                    if ui.radio(id == current, theme(id).label).clicked() {
                        chosen = Some(MenuAction::Theme(id));
                        ui.close();
                    }
                }
            });
        });
        ui.menu_button("Sound", |ui| {
            if ui.checkbox(&mut { sound.muted }, MUTE).clicked() {
                chosen = Some(MenuAction::Command(Command::ToggleMute));
            }
            let up = ui.add_enabled(sound.volume.up_enabled, egui::Button::new(VOLUME_UP));
            if up.clicked() {
                chosen = Some(MenuAction::Command(Command::VolumeUp));
            }
            let down = ui.add_enabled(sound.volume.down_enabled, egui::Button::new(VOLUME_DOWN));
            if down.clicked() {
                chosen = Some(MenuAction::Command(Command::VolumeDown));
            }
            ui.separator();
            if ui.checkbox(&mut { sound.ambience }, AMBIENCE).clicked() {
                chosen = Some(MenuAction::Command(Command::ToggleAmbience));
            }
        });
        ui.menu_button("Help", |ui| {
            if ui.button("Controls").clicked() {
                chosen = Some(MenuAction::Controls);
                ui.close();
            }
            if ui.button("About").clicked() {
                chosen = Some(MenuAction::About);
                ui.close();
            }
        });
    });
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(volume: i32) -> SoundMenu {
        sound_menu(SoundSettings {
            volume,
            ..SoundSettings::default()
        })
    }

    #[test]
    fn volume_up_is_disabled_at_full_volume_only() {
        assert!(!at(100).volume.up_enabled);
        assert!(at(100).volume.down_enabled);
        assert!(at(50).volume.up_enabled && at(50).volume.down_enabled);
        assert!(at(0).volume.up_enabled);
    }

    #[test]
    fn volume_down_is_disabled_at_zero_only() {
        assert!(!at(0).volume.down_enabled);
        assert!(at(1).volume.down_enabled);
    }

    #[test]
    fn the_checks_follow_the_settings() {
        let s = sound_menu(SoundSettings {
            muted: true,
            volume: 50,
            ambience: false,
        });
        assert!(s.muted && !s.ambience);
        let s = sound_menu(SoundSettings::default());
        assert!(!s.muted && s.ambience);
    }

    #[test]
    fn labels_carry_their_shortcut_text_as_in_the_cpp_menu() {
        assert_eq!(MUTE, "Mute (M)");
        assert_eq!(VOLUME_UP, "Volume up (+)");
        assert_eq!(VOLUME_DOWN, "Volume down (-)");
        assert_eq!(AMBIENCE, "Background ambience (B)");
        assert_eq!((NEW_GAME, NEW_GAME_SHORTCUT), ("New Game", "Ctrl+N"));
        assert_eq!((QUIT, QUIT_SHORTCUT), ("Quit", "Ctrl+Q"));
    }
}
