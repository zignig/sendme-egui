// Egui interface for sendme.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
mod app;
mod comms;
mod worker;
mod transport;

// mod sendme;

use app::App;
use eframe::NativeOptions;

fn main() -> eframe::Result {
    tracing_subscriber::fmt::init();
    let mut options = NativeOptions::default();
    options.viewport = options
        .viewport
        .with_title("Sendme Egui")
        .with_resizable(true)
        .with_inner_size([320., 400.])
        .with_drag_and_drop(true); // So cool !!
    App::run(options)
}
