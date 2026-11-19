mod app;
mod cli;
mod command;
mod debug_view;
mod fixed_step;
mod follow_camera;
mod input;
mod lane_commands;
mod scene_adapter;
mod sound_control;
mod sound_director;
mod sound_settings;
mod theme_sounds;
mod view3d;

#[cfg(test)]
mod recording_output;

use bowlingbox_render::render_stats_from_env;
use clap::Parser;
use eframe::egui;
use eframe::egui_wgpu::WgpuSetup;
use std::sync::Arc;

fn main() -> eframe::Result {
    let cli = cli::Cli::parse();
    let startup = cli.startup(std::env::var("BOWLINGBOX_RENDER").ok().as_deref());
    let stats = render_stats_from_env(std::env::var("BOWLINGBOX_RENDER_STATS").ok().as_deref());
    for warning in &startup.warnings {
        eprintln!("{warning}");
    }
    let (out, sound_warning) = bowlingbox_audio::open_output();
    if let Some(warning) = sound_warning {
        eprintln!("{warning}");
    }
    let title = "Bowlingbox";
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_app_id("Bowlingbox")
            .with_inner_size([960.0, 640.0])
            .with_min_inner_size([800.0, 600.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    if let WgpuSetup::CreateNew(create_new) = &mut options.wgpu_options.wgpu_setup {
        create_new.device_descriptor =
            Arc::new(move |adapter| view3d::device_descriptor(adapter, stats));
    }
    eframe::run_native(
        title,
        options,
        Box::new(move |cc| {
            let render_state = cc
                .wgpu_render_state
                .as_ref()
                .ok_or("the window has no wgpu render state")?;
            view3d::install(render_state, startup.theme, startup.render_mode, stats);
            let app = app::App::new(startup.theme, startup.debug_view, startup.mute, out);
            Ok(if stats {
                Box::new(app::RedrawAlways(app))
            } else {
                Box::new(app)
            })
        }),
    )
}
