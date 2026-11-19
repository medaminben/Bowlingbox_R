//! The window: menu, key and pointer input, the fixed-step clock, the 3D view and the debug view.
//! The rules live in the `lane` and `score` crates.

use bowlingbox_audio::AudioOutput;
use bowlingbox_lane::{Lane, Phase, RapierWorld};
use bowlingbox_render::{Camera, ThemeId, next_theme, theme};
use bowlingbox_score::ScoreCalculator;
use eframe::egui;

use crate::capture;
use crate::command::{Command, CommandTarget, Route, menu_click_allowed, route};
use crate::dialogs::{self, Dialog};
use crate::fixed_step::{self, FixedStep};
use crate::follow_camera::FollowCamera;
use crate::hud::{self, HudGeometry};
use crate::input::{self, KeyLatch};
use crate::menus::{self, MenuAction};
use crate::screen::ScreenState;
use crate::sound_control::SoundControl;
use crate::sound_settings::SoundSettings;
use crate::{debug_view, hud_text, lane_commands, scene_adapter, settings_store, view3d};

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
    /// The Sound menu is drawn open (`--screen=sound-menu`).
    sound_menu_pinned: bool,
    /// The last frame asked for no repaint: the window slept until the next input.
    idle: bool,
    save: SavePolicy,
    /// `--capture`: takes the screenshot and closes the window.
    capture: Option<capture::Run>,
}

/// Whether `save` writes the sound settings.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SavePolicy {
    Settings,
    /// A `--screen` run writes nothing.
    Nothing,
}

impl App {
    /// `stored` are the saved sound settings; `mute` mutes this session only. `screen` is the
    /// `--screen` state to start in.
    pub fn new(
        theme: ThemeId,
        debug_view: bool,
        (stored, mute): (SoundSettings, bool),
        screen: ScreenState,
        mut out: Box<dyn AudioOutput>,
    ) -> Self {
        let sound = SoundControl::new(out.as_mut(), stored, mute, theme);
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
            dialog: screen.dialog,
            sound_menu_pinned: screen.sound_menu_open,
            idle: false,
            save: if screen.read_only {
                SavePolicy::Nothing
            } else {
                SavePolicy::Settings
            },
            capture: None,
        }
    }

    /// Makes this a capture run: it also saves nothing, whatever the screen was.
    pub fn with_capture(mut self, run: capture::Run) -> Self {
        self.capture = Some(run);
        self.save = SavePolicy::Nothing;
        self
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

    /// Writes the sound settings to eframe's storage (see `settings_store`), unless this is a
    /// `--screen` run.
    fn save_settings(&self, storage: &mut dyn eframe::Storage) {
        if self.save == SavePolicy::Nothing {
            return;
        }
        settings_store::save(storage, self.sound.persisted());
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
        match menus::show(
            ui,
            self.theme,
            self.sound.settings(),
            self.sound_menu_pinned,
        ) {
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

/// Ctrl+'+' and Ctrl+'-' change the volume only; egui's own UI zoom is off.
pub fn configure(ctx: &egui::Context) {
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
    // eframe loads stored egui memory, with a zoom an earlier build saved, before the creator runs.
    ctx.set_zoom_factor(1.0);
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.save_settings(storage);
    }

    /// Only the sound settings persist, not egui's memory (which holds the UI zoom).
    fn persist_egui_memory(&self) -> bool {
        false
    }

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
        let home = self.follow.is_home();
        if let Some(run) = &mut self.capture {
            run.frame(&ctx, home);
        }
    }
}

/// The app, repainted every frame even while it waits for input, so `BOWLINGBOX_RENDER_STATS`
/// has frames to time. It forwards every `eframe::App` method the app overrides.
pub struct RedrawAlways<A>(pub A);

impl<A: eframe::App> eframe::App for RedrawAlways<A> {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.0.ui(ui, frame);
        ui.ctx().request_repaint();
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.0.save(storage);
    }

    fn persist_egui_memory(&self) -> bool {
        self.0.persist_egui_memory()
    }
}

#[cfg(test)]
mod acceptance;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings_store::test_storage::Fake;
    use bowlingbox_audio::SilentOutput;
    use eframe::App as _;
    use std::sync::Arc;

    fn app(stored: SoundSettings, mute: bool, screen: ScreenState) -> App {
        App::new(
            ThemeId::NeonHall,
            false,
            (stored, mute),
            screen,
            Box::new(SilentOutput::default()),
        )
    }

    #[test]
    fn the_screen_state_is_set_at_start_and_nothing_else() {
        let plain = app(SoundSettings::default(), false, ScreenState::default());
        assert!(plain.dialog.is_none() && !plain.sound_menu_pinned);
        let controls = app(
            SoundSettings::default(),
            false,
            crate::screen::Screen::Controls.state(),
        );
        assert_eq!(controls.dialog, Some(Dialog::Controls));
        assert!(!controls.sound_menu_pinned);
        let menu = app(
            SoundSettings::default(),
            false,
            crate::screen::Screen::SoundMenu.state(),
        );
        assert!(menu.sound_menu_pinned && menu.dialog.is_none());
    }

    #[test]
    fn save_writes_the_sound_settings() {
        let stored = SoundSettings {
            muted: false,
            volume: 20,
            ambience: false,
        };
        let mut storage = Fake::default();
        app(stored, false, ScreenState::default()).save(&mut storage);
        assert_eq!(settings_store::load(Some(&storage)), stored);
    }

    #[test]
    fn a_mute_session_saves_the_stored_muted_value() {
        let mut storage = Fake::default();
        app(SoundSettings::default(), true, ScreenState::default()).save(&mut storage);
        assert!(!settings_store::load(Some(&storage)).muted);
    }

    #[test]
    fn redraw_always_forwards_save() {
        struct Recorder(Vec<&'static str>);
        impl eframe::App for Recorder {
            fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}
            fn save(&mut self, _storage: &mut dyn eframe::Storage) {
                self.0.push("save");
            }
            fn persist_egui_memory(&self) -> bool {
                false
            }
        }
        let mut wrapped = RedrawAlways(Recorder(Vec::new()));
        wrapped.save(&mut Fake::default());
        assert_eq!(wrapped.0.0, ["save"]);
    }

    #[test]
    fn redraw_always_forwards_persist_egui_memory() {
        struct Recorder(bool);
        impl eframe::App for Recorder {
            fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}
            fn persist_egui_memory(&self) -> bool {
                self.0
            }
        }
        // The trait default is true, so a wrapper that forgets to forward would say true.
        assert!(!RedrawAlways(Recorder(false)).persist_egui_memory());
        assert!(RedrawAlways(Recorder(true)).persist_egui_memory());
    }

    #[test]
    fn the_app_keeps_egui_memory_out_of_storage() {
        let plain = app(SoundSettings::default(), false, ScreenState::default());
        assert!(!plain.persist_egui_memory());
    }

    #[test]
    fn keyboard_zoom_is_off_after_configure() {
        let ctx = egui::Context::default();
        assert!(ctx.options(|o| o.zoom_with_keyboard));
        configure(&ctx);
        assert!(!ctx.options(|o| o.zoom_with_keyboard));
    }

    #[test]
    fn configure_resets_a_zoom_loaded_from_storage() {
        let ctx = egui::Context::default();
        ctx.options_mut(|o| o.zoom_factor = 1.75);
        configure(&ctx);
        // The new zoom takes effect at the end of the next pass.
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .textures_delta
            .clear();
        assert!((ctx.zoom_factor() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_capture_run_saves_nothing_even_without_a_screen() {
        let mut storage = Fake::default();
        let run = capture::Run::new("x.png".into(), Arc::default());
        app(SoundSettings::default(), false, ScreenState::default())
            .with_capture(run)
            .save(&mut storage);
        assert!(storage.0.is_empty());
    }

    #[test]
    fn a_screen_run_saves_nothing() {
        let mut storage = Fake::default();
        let screen = ScreenState {
            read_only: true,
            ..ScreenState::default()
        };
        app(SoundSettings::default(), false, screen).save(&mut storage);
        assert!(storage.0.is_empty());
    }
}
