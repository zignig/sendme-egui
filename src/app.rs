// The application egui front end

use core::f32;
use std::path::PathBuf;

use crate::comms::{Command, Event, MessageDisplay, ProgressList};
use crate::worker::{Worker, WorkerHandle};
use anyhow::Result;
// use anyhow::anyhow;
use directories::UserDirs;
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
    download_path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        let download_path = match UserDirs::new() {
            Some(user_dirs) => user_dirs.download_dir().unwrap().to_owned().join("sendme"),
            None => std::process::exit(1),
        };
        Self {
            dark_mode: true,
            download_path,
        }
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
    picked_path: Option<PathBuf>,
    worker: WorkerHandle,
    mode: AppMode,
    receiver_ticket: String,
    progress: ProgressList,
    messages: Vec<MessageDisplay>,
    config: Config,
    elapsed: Option<u64>,
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
            let callback = Box::new(move || ctx.request_repaint());
            self.state.cmd(Command::SetUpdateCallBack { callback });
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
            elapsed: None,
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
                    self.mode = AppMode::Finished;
                    // Reset state
                    // self.reset();
                }
                Event::ProgressFinished(name) => self.progress.complete(name),
                Event::Tick(seconds) => {
                    self.elapsed = Some(seconds);
                }
                Event::StopTick => {
                    self.elapsed = None;
                }
            }
        }

        // active flags
        let mut send_enabled: bool = true;
        let mut receive_enabled: bool = true;

        // Use the mode to enable and disable
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
            AppMode::Finished => {
                self.mode = AppMode::Idle;
            }
        }
        // The actual gui

        // Status bar at the bottom
        egui::TopBottomPanel::bottom("status bar").show(ctx, |ui| {
            ui.add_space(5.);
            ui.horizontal(|ui| {
                if ui.button("Reset").clicked() {
                    self.reset();
                }
                ui.add_space(6.);
                if let Some(elapsed_seconds) = self.elapsed {
                    ui.label(format_seconds_as_hms(elapsed_seconds));
                }
            });
            ui.add_space(5.);
        });

        // Main panel
        egui::CentralPanel::default().show(ctx, |ui| {
            // Main buttons
            ui.vertical_centered(|ui| ui.heading("Sendme"));
            ui.separator();
            ui.add_space(5.);
            ui.horizontal(|ui| {
                ui.add_enabled_ui(receive_enabled, |ui| {
                    if ui.button("Fetch...").clicked() {
                        self.mode = AppMode::Fetch;
                    }
                });
                ui.add_space(2.);
                ui.add_enabled_ui(send_enabled, |ui| {
                    if ui.button("Send Folder…").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.picked_path = Some(path);
                        }
                        self.mode = AppMode::Send;
                    };
                    if ui.button("Send File…").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            self.picked_path = Some(path);
                        }
                        self.mode = AppMode::Send;
                    };
                });
                // ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {});
            });
            ui.separator();
            // Show mode based widgets
            match self.mode {
                AppMode::Init => {}
                AppMode::Idle => {
                    self.fetch_box(ui);
                }
                AppMode::Send => {
                    if let Some(path) = &self.picked_path {
                        self.cmd(Command::Send(path.to_owned()));
                        self.mode = AppMode::SendProgress;
                    }
                }
                AppMode::Fetch => {
                    self.fetch_box(ui);
                }
                AppMode::SendProgress => {
                    ui.label("Sending");
                }
                AppMode::FetchProgess => {}
                AppMode::Finished => {
                    // self.reset();
                }
            }
            // Show the current messages
            self.show_messages(ui);
            // Show the current progress bars
            self.show_progress(ui);
            ui.separator();
        });
    }

    fn fetch_box(&mut self, ui: &mut Ui) {
        ui.label("Blob ticket...");
        ui.add_space(8.);
        let _ticket_edit = egui::TextEdit::multiline(&mut self.receiver_ticket)
            .desired_width(f32::INFINITY)
            .show(ui);
        ui.horizontal(|ui| {
            if ui.button("Fetch").clicked() {
                self.cmd(Command::Fetch((
                    self.receiver_ticket.clone(),
                    self.config.download_path.clone(),
                )));
                self.mode = AppMode::FetchProgess;
            };
            if ui.button("Fetch Into...").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.picked_path = Some(path.clone());
                    self.cmd(Command::Fetch((self.receiver_ticket.clone(), path.clone())));
                    self.mode = AppMode::FetchProgess;
                }
            };
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
        // let text_style = egui::TextStyle::Body;
        // let row_height = ui.text_style_height(&text_style);
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
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

fn format_seconds_as_hms(total_seconds: u64) -> String {
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("[{:02}:{:02}:{:02}]", hours, minutes, seconds)
}
