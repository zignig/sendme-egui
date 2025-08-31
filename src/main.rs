// Egui interface for sendme.

use eframe::NativeOptions;
mod app;
mod comms;
mod worker;

use app::App;


fn main() -> eframe::Result {
    tracing_subscriber::fmt::init();
    let mut options = NativeOptions::default();
    options.viewport = options
        .viewport
        .with_title("Sendme Egui")
        .with_resizable(true)
        .with_inner_size([500., 600.])
        .with_drag_and_drop(true); // So cool !!
    App::run(options)
}
