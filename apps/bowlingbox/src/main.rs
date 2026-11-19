mod app;
mod cli;
mod command;
mod debug_view;
mod fixed_step;
mod follow_camera;
mod input;
mod lane_commands;
mod scene_adapter;
mod view3d;

use clap::Parser;
use eframe::egui;

fn main() -> eframe::Result {
    let cli = cli::Cli::parse();
    let startup = cli.startup(std::env::var("BOWLINGBOX_RENDER").ok().as_deref());
    for warning in &startup.warnings {
        eprintln!("{warning}");
    }
    let title = "Bowlingbox";
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_app_id("Bowlingbox")
            .with_inner_size([960.0, 640.0])
            .with_min_inner_size([800.0, 600.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        title,
        options,
        Box::new(move |cc| {
            let render_state = cc
                .wgpu_render_state
                .as_ref()
                .ok_or("the window has no wgpu render state")?;
            view3d::install(render_state, startup.theme);
            Ok(Box::new(app::App::new(startup.theme, startup.debug_view)))
        }),
    )
}
