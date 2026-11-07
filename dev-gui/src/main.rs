#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod config;
mod format;
mod jobs;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 680.0])
            .with_min_inner_size([820.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "App Updater Dev",
        options,
        Box::new(|cc| {
            gui_common::apply_style(&cc.egui_ctx);
            Ok(Box::new(app::DevApp::new()))
        }),
    )
}
