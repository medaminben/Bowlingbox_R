//! The window: menu, key and pointer input, the fixed-step clock, the 3D view and the debug view.
//! The rules live in the `lane` and `score` crates.

use bowlingbox_lane::{Lane, Phase, RapierWorld};
use bowlingbox_render::{Camera, THEMES, ThemeId, next_theme, theme};
use bowlingbox_score::ScoreCalculator;
use eframe::egui;

use crate::command::{Command, CommandTarget, Route, route};
use crate::fixed_step::{self, FixedStep};
use crate::follow_camera::FollowCamera;
use crate::input::{self, KeyLatch};
use crate::{debug_view, lane_commands, scene_adapter, view3d};

const VERSION: &str = env!("BK_VERSION_FULL");

pub struct App {
    lane: Lane<RapierWorld, ScoreCalculator>,
    clock: FixedStep,
    follow: FollowCamera,
    theme: ThemeId,
    /// Draw the top-down physics view instead of the 3D view (`--debug-view`).
    debug_view: bool,
    latch: KeyLatch,
    /// The lane drawing of the last frame, where pointer presses count.
    canvas: Option<egui::Rect>,
    show_about: bool,
    /// The last frame asked for no repaint: the window slept until the next input.
    idle: bool,
}

impl App {
    pub fn new(theme: ThemeId, debug_view: bool) -> Self {
        Self {
            lane: Lane::default(),
            clock: FixedStep::default(),
            follow: FollowCamera::default(),
            theme,
            debug_view,
            latch: KeyLatch::default(),
            canvas: None,
            show_about: false,
            idle: false,
        }
    }

    fn run(&mut self, ctx: &egui::Context, command: Command) {
        match command.target() {
            // A command the phase does not allow changes nothing.
            CommandTarget::Lane => {
                let _ = lane_commands::execute(command, &mut self.lane);
            }
            CommandTarget::App => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            CommandTarget::View => self.theme = next_theme(self.theme),
            // Sound arrives in a later milestone.
            CommandTarget::Sound => {}
        }
    }

    fn dispatch(&mut self, ctx: &egui::Context, command: Command) {
        match route(command, self.show_about, ctx.any_popup_open()) {
            Route::Run(command) => self.run(ctx, command),
            Route::CloseAbout => self.show_about = false,
            Route::Ignore => {}
        }
    }

    fn phase(&self) -> Phase {
        self.lane.snapshot().phase
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            let command = match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => input::key_down(key, modifiers, self.phase())
                    .filter(|command| self.latch.accept(key, *command)),
                egui::Event::Key {
                    key,
                    pressed: false,
                    ..
                } => {
                    self.latch.release(key);
                    input::key_up(key, self.phase())
                }
                egui::Event::WindowFocused(false) => {
                    self.latch.clear();
                    None
                }
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    ..
                } => {
                    if pressed {
                        let on_canvas = self.canvas.is_some_and(|rect| rect.contains(pos));
                        let blocked = self.show_about || ctx.any_popup_open();
                        (on_canvas && !blocked)
                            .then(|| input::pointer_down(true, self.phase()))
                            .flatten()
                    } else {
                        input::pointer_up(true, self.phase())
                    }
                }
                _ => None,
            };
            if let Some(command) = command {
                self.dispatch(ctx, command);
            }
        }

        let (dx, primary, secondary, over_canvas) = ctx.input(|i| {
            (
                i.pointer.delta().x,
                i.pointer.primary_down(),
                i.pointer.secondary_down(),
                i.pointer
                    .latest_pos()
                    .zip(self.canvas)
                    .is_some_and(|(pos, rect)| rect.contains(pos)),
            )
        });
        let width = self.canvas.map_or(0.0, |rect| rect.width());
        let free = over_canvas && !self.show_about && !ctx.any_popup_open();
        let moved = input::pointer_move(dx, primary, secondary, width, self.phase());
        if let Some(command) = moved.filter(|_| free) {
            self.dispatch(ctx, command);
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
            ui.menu_button("View", |ui| {
                ui.menu_button("Theme", |ui| {
                    for id in THEMES {
                        ui.radio_value(&mut self.theme, id, theme(id).label);
                    }
                });
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
        self.handle_input(&ctx);

        let elapsed = fixed_step::frame_time(self.idle, ctx.input(|i| i.unstable_dt));
        for _ in 0..self.clock.advance(elapsed) {
            self.lane.step(fixed_step::DT);
        }
        let snapshot = self.lane.snapshot();
        self.follow.update(&snapshot, elapsed);
        self.idle =
            matches!(snapshot.phase, Phase::Aiming | Phase::GameOver) && self.follow.is_home();
        if !self.idle {
            ctx.request_repaint();
        }

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| {
            let status = debug_view::status(&snapshot, self.lane.score_sheet());
            ui.label(format!("{status} | Theme {}", theme(self.theme).label));
        });
        egui::CentralPanel::default().show(ui, |ui| {
            debug_view::scoreboard(ui, self.lane.score_sheet());
            debug_view::power_bar(ui, &snapshot);
            self.canvas = Some(if self.debug_view {
                debug_view::lane(ui, &snapshot)
            } else {
                let mut camera = Camera::default();
                self.follow.apply_to(&mut camera);
                view3d::show(ui, camera, &scene_adapter::to_scene(&snapshot), self.theme)
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

/// The app, repainted every frame even while it waits for input, so `BOWLINGBOX_RENDER_STATS`
/// has frames to time.
pub struct RedrawAlways(pub App);

impl eframe::App for RedrawAlways {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.0.ui(ui, frame);
        ui.ctx().request_repaint();
    }
}
