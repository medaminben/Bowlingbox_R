mod app;
mod command;

use eframe::egui;

fn main() -> eframe::Result {
    if std::env::args().any(|arg| arg == "--version") {
        println!("bowlingbox {}", env!("BK_VERSION_FULL"));
        return Ok(());
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
        Box::new(|_cc| Ok(Box::new(app::App::default()))),
    )
}
