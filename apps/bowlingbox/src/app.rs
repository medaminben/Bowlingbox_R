//! The window. It only shows what the `score` crate computes; the logic lives there.

use bowlingbox_score::{Game, ScoreCalculator, ScoreSheet};
use eframe::egui;

use crate::command::{Command, Route, command_for_key, route};

const VERSION: &str = env!("BK_VERSION_FULL");

#[derive(Default)]
pub struct App {
    game: ScoreCalculator,
    show_about: bool,
}

impl App {
    fn run(&mut self, ctx: &egui::Context, command: Command) {
        match command {
            Command::NewGame => self.game.reset(),
            Command::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Game", |ui| {
                if ui.button("New Game").clicked() {
                    self.run(ui.ctx(), Command::NewGame);
                    ui.close();
                }
                if ui.button("Quit").clicked() {
                    self.run(ui.ctx(), Command::Quit);
                    ui.close();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("About").clicked() {
                    self.show_about = true;
                    ui.close();
                }
            });
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let pressed = ctx.input(|i| {
            i.events.iter().find_map(|event| match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => command_for_key(*key, *modifiers),
                _ => None,
            })
        });
        if let Some(command) = pressed {
            match route(command, self.show_about, ctx.any_popup_open()) {
                Route::Run(command) => self.run(&ctx, command),
                Route::CloseAbout => self.show_about = false,
                Route::Ignore => {}
            }
        }

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.label(format!("Score {}", self.game.score()));
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                ui.heading("Bowlingbox — the lane arrives in milestone 3");
            });
        });

        if self.show_about {
            egui::Window::new("About Bowlingbox")
                .collapsible(false)
                .resizable(false)
                .open(&mut self.show_about)
                .show(&ctx, |ui| {
                    ui.label(format!("Bowlingbox {VERSION}"));
                });
        }
    }
}
