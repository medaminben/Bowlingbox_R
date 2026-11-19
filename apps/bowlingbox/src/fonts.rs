//! The game's font: Rubik Medium, embedded, first in egui's proportional family.

use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

/// The name Rubik is registered under.
pub const RUBIK: &str = "Rubik";

/// egui's default fonts with Rubik in front of the proportional family; the defaults stay behind
/// it for the glyphs Rubik lacks.
pub fn definitions() -> FontDefinitions {
    let mut defs = FontDefinitions::default();
    defs.font_data.insert(
        RUBIK.to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/Rubik-Medium.ttf"
        ))),
    );
    defs.families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, RUBIK.to_owned());
    defs
}

pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(definitions());
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Color32, FontId, RawInput};

    #[test]
    fn rubik_is_the_first_proportional_font() {
        let defs = definitions();
        assert!(defs.font_data.contains_key(RUBIK));
        assert_eq!(defs.families[&FontFamily::Proportional][0], RUBIK);
    }

    #[test]
    fn the_monospace_family_keeps_its_default_font() {
        let defs = definitions();
        assert!(!defs.families[&FontFamily::Monospace].contains(&RUBIK.to_owned()));
    }

    #[test]
    fn the_embedded_font_loads_and_lays_out_text() {
        let ctx = egui::Context::default();
        install(&ctx);
        let mut width = 0.0;
        let mut output = ctx.run_ui(RawInput::default(), |ui| {
            let galley = ui.painter().layout_no_wrap(
                "Total 120".to_owned(),
                FontId::proportional(20.0),
                Color32::WHITE,
            );
            width = galley.size().x;
        });
        output.textures_delta.clear();
        assert!(width > 20.0, "width {width}");
    }
}
