// The application egui front end

use core::f32;

use crate::comms::{Command, Event, MessageDisplay, ProgressList};
use crate::worker::{Worker, WorkerHandle};
use anyhow::Result;
use eframe::NativeOptions;
use eframe::egui::{self, Visuals};
use egui::Ui;
use rfd;
use serde_derive::{Deserialize, Serialize};
use tracing::info;

// Application saved config
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    dark_mode: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { dark_mode: true }
    }
}

// Message list max
const MESSAGE_MAX: usize = 300;

// The application
pub struct App {
    is_first_update: bool,
    state: AppState,
}

// The application mode
#[derive(PartialEq)]
enum AppMode {
    Init,
    Idle,
    Send,
    SendProgress,
    Fetch,
    FetchProgess,
    Finished,
}

// Internal state for the application
struct AppState {
    picked_path: Option<String>,
    worker: WorkerHandle,
    mode: AppMode,
    receiver_ticket: String,
    progress: ProgressList,
    messages: Vec<MessageDisplay>,
    config: Config,
}

// Make the egui impl for display
impl eframe::App for App {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        if self.is_first_update {
            self.is_first_update = false;
            ctx.set_zoom_factor(1.2);
            if !self.state.config.dark_mode {
                ctx.set_visuals(Visuals::light());
            };
            let ctx = ctx.clone();
            ctx.request_repaint();
        }
        self.state.update(ctx);
    }
}

// The application runner start,draw, etc...
// Spawns the worker as a subthread
impl App {
    pub fn run(options: NativeOptions) -> Result<(), eframe::Error> {
        let handle = Worker::spawn();
        // Load the config
        let config = confy::load("sendme-egui", None).unwrap_or_default();
        let path = confy::get_configuration_file_path("sendme-egui", None);
        info!("config path {:?}", path);
        let state = AppState {
            picked_path: None,
            worker: handle,
            mode: AppMode::Init,
            receiver_ticket: String::new(),
            progress: ProgressList::new(),
            messages: Vec::new(),
            config: config,
        };
        let app = App {
            is_first_update: true,
            state,
        };
        // Rus the egui in the foreground
        eframe::run_native("sendme-egui", options, Box::new(|_cc| Ok(Box::new(app))))
    }
}

// Actual gui code
impl AppState {
    fn update(&mut self, ctx: &egui::Context) {
        // Events from the worker
        while let Ok(event) = self.worker.event_rx.try_recv() {
            // Event probably needs a repaint
            ctx.request_repaint();
            match event {
                Event::Message(m) => {
                    if self.messages.len() > MESSAGE_MAX {
                        let _ = self.messages.remove(0);
                    }
                    self.messages.push(m);
                }
                Event::Progress((name, current, total)) => {
                    self.progress.insert(name, current, total);
                }
                Event::Finished => {
                    // Reset state
                    self.reset();
                }
                Event::ProgressFinished(name) => self.progress.complete(name),
            }
        }

        // active flags
        let mut send_enabled: bool = true;
        let mut receive_enabled: bool = true;

        // Use the mode
        match self.mode {
            AppMode::Init => {
                self.cmd(Command::Setup);
                self.mode = AppMode::Idle;
            }
            AppMode::Idle => {}
            AppMode::Send => {
                receive_enabled = false;
            }
            AppMode::Fetch => {
                send_enabled = false;
            }
            AppMode::SendProgress | AppMode::FetchProgess => {
                receive_enabled = false;
                send_enabled = false;
            }
            AppMode::Finished => {}
        }
        // The actual gui
        egui::CentralPanel::default().show(ctx, |ui| {
            // Main buttons
            ui.vertical_centered(|ui| ui.heading("Sendme"));
            ui.separator();
            ui.add_space(5.);
            ui.horizontal(|ui| {
                ui.add_enabled_ui(send_enabled, |ui| {
                    ui.add_enabled_ui(receive_enabled, |ui| {
                        if ui.button("Fetch...").clicked() {
                            self.mode = AppMode::Fetch;
                        }
                    });
                    ui.add_space(2.);
                    if ui.button("Send Folder…").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.picked_path = Some(path.display().to_string());
                        }
                        self.mode = AppMode::Send;
                    };
                    if ui.button("Send File…").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            self.picked_path = Some(path.display().to_string());
                        }
                        self.mode = AppMode::Send;
                    };
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {});
            });
            ui.separator();
            // Show mode based widgets
            match self.mode {
                AppMode::Init => {}
                AppMode::Idle => {}
                AppMode::Send => {
                    if let Some(path) = &self.picked_path {
                        self.cmd(Command::Send(path.to_owned()));
                        self.mode = AppMode::SendProgress;
                    }
                }
                AppMode::Fetch => {
                    ui.small("Blob ticket.");
                    ui.add_space(8.);
                    let _ticket_edit = egui::TextEdit::multiline(&mut self.receiver_ticket)
                        .desired_width(f32::INFINITY)
                        .show(ui);
                    ui.horizontal(|ui| {
                        if ui.button("Fetch").clicked() {
                            self.cmd(Command::Fetch(self.receiver_ticket.clone()));
                            self.mode = AppMode::FetchProgess;
                        };
                        if ui.button("Fetch Into...").clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                self.picked_path = Some(path.display().to_string());
                            }
                            self.cmd(Command::Fetch(self.receiver_ticket.clone()));
                            self.mode = AppMode::FetchProgess;
                        };
                    });
                }
                AppMode::SendProgress => {
                    ui.label("Sending");
                }
                AppMode::FetchProgess => {
                    // let prog_val = (self.progress_current as f32) / (self.progress_total as f32);
                    // let progress_bar = egui::ProgressBar::new(prog_val)
                    //     .text(&self.progress_text)
                    //     .show_percentage();
                    // ui.add(progress_bar);
                    // // Add a list of the messages.
                    // if prog_val == 1.0 {
                    //     self.progress_current = 0;
                    //     self.progress_total = 0;
                    //     self.mode = AppMode::Idle;
                    // }
                }
                AppMode::Finished => {
                    self.reset();
                }
            }

            // Show the current messages
            self.show_messages(ui);
            // Show the current progress bars
            self.show_progress(ui);
            ui.separator();
            // Temp reset button
            if ui.button("Reset").clicked() {
                self.reset();
            }
        });
    }

    // Reset the application
    fn reset(&mut self) {
        self.mode = AppMode::Idle;
        self.receiver_ticket = "".to_string();
        self.messages = Vec::new();
        self.progress.clear();
    }

    // Show the list of progress bars
    fn show_progress(&mut self, ui: &mut Ui) {
        ui.add_space(4.);
        self.progress.show(ui);
    }

    // Show the list of messages
    fn show_messages(&mut self, ui: &mut Ui) {
        ui.add_space(4.);
        egui::ScrollArea::vertical()
            .max_height(100.)
            .show(ui, |ui| {
                for message in self.messages.iter() {
                    message.show(ui);
                    ui.add_space(4.);
                }
            });
    }

    fn cmd(&self, command: Command) {
        self.worker
            .command_tx
            .send_blocking(command)
            .expect("Worker is not responding");
    }
}
