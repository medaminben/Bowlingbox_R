//! The Controls and About dialogs. While one is open the lane keeps running, but `command::route`
//! drops the game keys.

use eframe::egui;

use crate::hud_text;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialog {
    Controls,
    About,
}

pub fn about_text(version: &str) -> String {
    format!("Bowlingbox {version}\nA 3D ten-pin bowling game, rebuilt in Rust.\nPhysics: Rapier.")
}

/// Shows the open dialog; its close button or title-bar cross clears `open`.
pub fn show(ctx: &egui::Context, open: &mut Option<Dialog>, version: &str) {
    let Some(dialog) = *open else { return };
    let (title, text) = match dialog {
        Dialog::Controls => ("Controls", hud_text::controls_help().to_owned()),
        Dialog::About => ("About", about_text(version)),
    };
    let mut window_open = true;
    let mut close = false;
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .open(&mut window_open)
        .show(ctx, |ui| {
            ui.label(text);
            ui.add_space(8.0);
            close = ui.button("Close").clicked();
        });
    if !window_open || close {
        *open = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn about_names_the_version_and_the_physics() {
        let text = about_text("2.0.0-dev");
        assert!(text.starts_with("Bowlingbox 2.0.0-dev"), "{text}");
        assert!(text.contains("Rapier"));
    }
}
