//! The window. It only shows what the `score` crate computes; the logic lives there.

use bowlingbox_score::{ScoreCalculator, ScoreSheet};

#[derive(Default)]
pub struct App {
    game: ScoreCalculator,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        eframe::egui::CentralPanel::default().show(ui, |ui| {
            ui.heading(format!("Score: {}", self.game.score()));
        });
    }
}
