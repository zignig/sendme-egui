

// Egui interface for sendme.

use eframe::NativeOptions;
use eframe::egui;
use rfd;
mod app;

#[derive(Default)]
struct MyApp {
    picked_path: Option<String>,
}

fn main() -> eframe::Result {
    let mut options = NativeOptions::default();
    options.viewport = options
        .viewport
        .with_title("Sendme Egui")
        .with_resizable(true);
        // .with_inner_size([500., 600.]);
    eframe::run_native(
        "Sendme Egui",
        options,
        Box::new(|_cc| Ok(Box::<MyApp>::default())),
    )
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if ui.button("Open file…").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.picked_path = Some(path.display().to_string());
                }
            }
        });
    }
}
