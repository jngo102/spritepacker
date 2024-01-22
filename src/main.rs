#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use app::app::App;
use eframe::egui;

pub mod app;
pub mod components;
pub mod tk2d;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800., 640.]),
        ..Default::default()
    };
    eframe::run_native(
        "Sprite Packer",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Box::new(App::new(cc))
        }),
    )
}
