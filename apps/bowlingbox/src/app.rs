//! The window: menu, key and pointer input, the fixed-step clock, the 3D view and the debug view.
//! The rules live in the `lane` and `score` crates.

use bowlingbox_audio::AudioOutput;
use bowlingbox_lane::{Lane, Phase, RapierWorld};
use bowlingbox_render::{Camera, ThemeId, next_theme, theme};
use bowlingbox_score::ScoreCalculator;
use eframe::egui;

use crate::command::{Command, CommandTarget, Route, menu_click_allowed, route};
use crate::dialogs::{self, Dialog};
use crate::fixed_step::{self, FixedStep};
use crate::follow_camera::FollowCamera;
use crate::hud::{self, HudGeometry};
use crate::input::{self, KeyLatch};
use crate::menus::{self, MenuAction};
use crate::sound_control::SoundControl;
use crate::{debug_view, hud_text, lane_commands, scene_adapter, view3d};

const VERSION: &str = env!("BK_VERSION_FULL");

pub struct App {
    lane: Lane<RapierWorld, ScoreCalculator>,
    clock: FixedStep,
    follow: FollowCamera,
    theme: ThemeId,
    /// Draw the top-down physics view instead of the 3D view (`--debug-view`).
    debug_view: bool,
    latch: KeyLatch,
    out: Box<dyn AudioOutput>,
    sound: SoundControl,
    /// The lane drawing of the last frame, where pointer presses count.
    canvas: Option<egui::Rect>,
    /// The open dialog: the lane runs on, but its keys do not reach it.
    dialog: Option<Dialog>,
    /// The last frame asked for no repaint: the window slept until the next input.
    idle: bool,
}

impl App {
    pub fn new(
        theme: ThemeId,
        debug_view: bool,
        muted: bool,
        mut out: Box<dyn AudioOutput>,
    ) -> Self {
        let sound = SoundControl::new(out.as_mut(), muted, theme);
        Self {
            lane: Lane::default(),
            clock: FixedStep::default(),
            follow: FollowCamera::default(),
            theme,
            debug_view,
            latch: KeyLatch::default(),
            out,
            sound,
            canvas: None,
            dialog: None,
            idle: false,
        }
    }

    fn run(&mut self, ctx: &egui::Context, command: Command) {
        match command.target() {
            // A command the phase does not allow changes nothing.
            CommandTarget::Lane => {
                let done = lane_commands::execute(command, &mut self.lane).is_ok();
                if done && command == Command::NewGame {
                    self.sound.new_game(self.out.as_mut());
                }
            }
            CommandTarget::App => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            CommandTarget::View => self.set_theme(next_theme(self.theme)),
            CommandTarget::Sound => self.sound.apply(self.out.as_mut(), command),
        }
    }

    fn set_theme(&mut self, theme: ThemeId) {
        if theme != self.theme {
            self.theme = theme;
            self.sound.theme_changed(self.out.as_mut(), theme);
        }
    }

    fn dispatch(&mut self, ctx: &egui::Context, command: Command) {
        match route(command, self.dialog.is_some(), ctx.any_popup_open()) {
            Route::Run(command) => self.run(ctx, command),
            Route::CloseDialog => self.dialog = None,
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
                        let blocked = self.dialog.is_some() || ctx.any_popup_open();
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
        let free = over_canvas && self.dialog.is_none() && !ctx.any_popup_open();
        let moved = input::pointer_move(dx, primary, secondary, width, self.phase());
        if let Some(command) = moved.filter(|_| free) {
            self.dispatch(ctx, command);
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        match menus::show(ui, self.theme, self.sound.settings()) {
            Some(MenuAction::Command(command)) => {
                if menu_click_allowed(command, self.dialog.is_some()) {
                    self.run(ui.ctx(), command);
                }
            }
            Some(MenuAction::Theme(id)) => self.dispatch_theme(id),
            Some(MenuAction::Controls) => self.dialog = Some(Dialog::Controls),
            Some(MenuAction::About) => self.dialog = Some(Dialog::About),
            None => {}
        }
    }

    /// A theme picked in the menu; like the keys, it does nothing behind a dialog.
    fn dispatch_theme(&mut self, id: ThemeId) {
        if self.dialog.is_none() {
            self.set_theme(id);
        }
    }

    /// The 3D view in the middle band with the HUD around it, as the C++ canvas does.
    fn hud_view(&mut self, ui: &mut egui::Ui, snapshot: &bowlingbox_lane::Snapshot) {
        let (area, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
        self.canvas = Some(area);
        let geometry = HudGeometry::new(area);
        let painter = ui.painter_at(area);
        painter.rect_filled(area, 0.0, hud::CLEAR);
        let mut camera = Camera::default();
        self.follow.apply_to(&mut camera);
        view3d::show(
            ui,
            geometry.middle,
            camera,
            &scene_adapter::to_scene(snapshot),
            self.theme,
        );
        hud::draw(&painter, &geometry, snapshot, self.lane.score_sheet());
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_input(&ctx);

        let elapsed = fixed_step::frame_time(self.idle, ctx.input(|i| i.unstable_dt));
        let steps = self.clock.advance(elapsed);
        let snapshot = self
            .sound
            .step_frame(self.out.as_mut(), &mut self.lane, steps);
        self.follow.update(&snapshot, elapsed);
        self.idle =
            matches!(snapshot.phase, Phase::Aiming | Phase::GameOver) && self.follow.is_home();
        if !self.idle {
            ctx.request_repaint();
        }

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| {
            let status = hud_text::status(&snapshot, self.lane.score_sheet());
            ui.label(format!(
                "{status} | Theme {} | {}",
                theme(self.theme).label,
                self.sound.status()
            ));
        });
        if self.debug_view {
            egui::CentralPanel::default().show(ui, |ui| {
                debug_view::scoreboard(ui, self.lane.score_sheet());
                debug_view::power_bar(ui, &snapshot);
                self.canvas = Some(debug_view::lane(ui, &snapshot));
            });
        } else {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| self.hud_view(ui, &snapshot));
        }
        dialogs::show(&ctx, &mut self.dialog, VERSION);
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
