// The application egui front end

use core::f32;
use std::fmt::Display;
use std::path::PathBuf;

use crate::about::ABOUT;
use crate::comms::{Command, Config, Event, MessageDisplay, MessageType, ProgressList};
use crate::worker::{Worker, WorkerHandle};
use anyhow::Result;
use directories::{BaseDirs, UserDirs};
use eframe::NativeOptions;
use eframe::egui::{self, FontId, RichText, Visuals};
use egui::Ui;
use rfd;

use tracing::info;

const APP_NAME: &str = "sendme-egui";

impl Default for Config {
    fn default() -> Self {
        let download_path = match UserDirs::new() {
            Some(user_dirs) => user_dirs.download_dir().unwrap().to_owned().join("sendme"),
            None => std::process::exit(1),
        };
        let store_path = match BaseDirs::new() {
            Some(base_dirs) => base_dirs
                .data_dir()
                .to_owned()
                .join("sendme-egui")
                .join("blob_data"),
            None => std::process::exit(1),
        };
        Self {
            dark_mode: true,
            download_path,
            store_path,
        }
    }
}

// Message list max
const MESSAGE_MAX: usize = 50;

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
    FetchProgess,
    Finished,
    Config,
    About,
}

impl Display for AppMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let val = match self {
            AppMode::Init => "Init",
            AppMode::Idle => "Idle",
            AppMode::Send => "Send",
            AppMode::SendProgress => "Send Running...",
            AppMode::FetchProgess => "Fetch Running...",
            AppMode::Finished => "Finished",
            AppMode::Config => "Config",
            AppMode::About => "About...",
        };
        write!(f, "{}", val)
    }
}

// Internal state for the application
struct AppState {
    picked_path: Option<PathBuf>,
    worker: WorkerHandle,
    mode: AppMode,
    receiver_ticket: String,
    send_ticket: Option<String>,
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
            ctx.set_zoom_factor(1.);
            if self.state.config.dark_mode {
                ctx.set_visuals(Visuals::dark());
            } else {
                ctx.set_visuals(Visuals::light());
            };
            // Push the redraw function into the worker.
            // This is janky and has a mutex for borrowing reasons
            let ctx = ctx.clone();
            let callback = Box::new(move || ctx.request_repaint());
            self.state.cmd(Command::Setup { callback });
        }
        self.state.update(ctx);
    }
}

// The application runner start,draw, etc...
// Spawns the worker as a subthread
impl App {
    pub fn run(options: NativeOptions) -> Result<(), eframe::Error> {
        // Load the config
        let config: Config = confy::load(APP_NAME, None).unwrap_or_default();

        // Start up the worker , separate thread , async runner
        let handle = Worker::spawn(config.clone());

        // Create a fresh application
        let state = AppState {
            picked_path: None,
            worker: handle,
            mode: AppMode::Init,
            receiver_ticket: String::new(),
            send_ticket: None,
            progress: ProgressList::new(),
            messages: Vec::new(),
            config: config,
            elapsed: None,
        };

        // New App
        let app = App {
            is_first_update: true,
            state,
        };

        // Run the egui in the foreground, worker as  a subthread (async)
        eframe::run_native("sendme-egui", options, Box::new(|_cc| Ok(Box::new(app))))
    }
}

// Actual gui code (the interface)
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
                    self.cmd(Command::ResetTimer);
                    self.mode = AppMode::Finished;
                }
                Event::ProgressFinished(name) => self.progress.finish(name),
                Event::ProgressComplete(name) => self.progress.complete(name),
                Event::Tick(seconds) => {
                    self.elapsed = Some(seconds);
                }
                Event::StopTick => {
                    self.elapsed = None;
                }
                Event::SendTicket(ticket) => self.send_ticket = Some(ticket),
            }
        }

        // active flags
        let mut send_enabled: bool = true;

        // Use the mode to enable and disable
        match self.mode {
            AppMode::Init => {
                self.mode = AppMode::Idle;
            }
            AppMode::SendProgress | AppMode::FetchProgess => {
                send_enabled = false;
            }
            AppMode::Finished => {
                self.mode = AppMode::Idle;
            }
            AppMode::Config => {
                send_enabled = false;
            }
            _ => {}
        }

        // The actual gui

        self.footer(ctx);

        // Main panel
        egui::CentralPanel::default().show(ctx, |ui| {
            // Main buttons
            ui.vertical_centered(|ui| ui.heading("Sendme"));
            ui.separator();
            ui.add_space(5.);
            self.button_header(send_enabled, ui);
            // gap
            ui.separator();
            // Modal Display
            self.modal_display_above(ui);
            // Show the current progress bars
            self.show_progress(ui);
            // Show the current messages
            self.show_messages(ui);
            // Lower modal display
            self.modal_display_below(ui);
        });
    }

    fn footer(&mut self, ctx: &egui::Context) {
        // Status bar at the bottom
        // egui needs outer things done first
        // the status bar at the bottom.
        egui::TopBottomPanel::bottom("status bar").show(ctx, |ui| {
            ui.add_space(5.);
            ui.horizontal(|ui| {
                if ui.button("Clear").clicked() {
                    // Clear and reset the interface
                    // if we are in send mode drop the connection
                    if self.mode == AppMode::SendProgress {
                        self.cmd(Command::CancelSend);
                    }
                    // Reset the timer for good measure
                    self.cmd(Command::ResetTimer);
                    self.reset();
                }
                ui.add_space(5.);

                // mode and timer
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(elapsed_seconds) = self.elapsed {
                        ui.label(RichText::new(format_seconds_as_hms(elapsed_seconds)).strong());
                    }
                    ui.label(format!(" {} ", self.mode));
                });
            });
            ui.add_space(5.);
        });
    }

    // The buttons at the top
    fn button_header(&mut self, send_enabled: bool, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.add_space(2.);
            ui.add_enabled_ui(send_enabled, |ui| {
                if ui.button("Send Folder…").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.picked_path = Some(path);
                        self.mode = AppMode::Send;
                    }
                };
                if ui.button("Send File…").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        self.picked_path = Some(path);
                        self.mode = AppMode::Send;
                    }
                };
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("About").clicked() {
                        self.mode = AppMode::About;
                    }
                    ui.add_space(10.);
                    if ui.button("Config").clicked() {
                        self.mode = AppMode::Config;
                    }
                });
            });
            ui.add_space(5.);
        });
    }

    // modal display above progress and messages
    fn modal_display_above(&mut self, ui: &mut Ui) {
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
            AppMode::SendProgress => {
                if let Some(path) = &self.picked_path {
                    ui.label(format!("{}", path.display()));
                }
                if let Some(ticket) = &self.send_ticket {
                    ui.add_space(10.);
                    ui.label("Blob Ticket...");
                    ui.add_space(5.);
                    ui.separator();
                    ui.add_space(10.);
                    ui.label(RichText::new(ticket).strong().font(FontId::monospace(15.)));
                    ui.add_space(10.);
                    ui.separator();
                }
            }
            AppMode::FetchProgess => {
                ui.label("Fetching ...");
            }
            AppMode::Finished => {}
            AppMode::Config => {
                self.show_config(ui);
            }
            AppMode::About => self.about(ui),
        }
    }

    // Modal display below progress and messages
    fn modal_display_below(&mut self, ui: &mut Ui) {
        match self.mode {
            AppMode::SendProgress => {
                ui.add_space(10.);
                ui.separator();
                if ui.button("Finish").clicked() {
                    // This activates a notify within the send system
                    // to finish the send endpoint
                    self.cmd(Command::CancelSend);
                    self.send_ticket = None;
                    self.mode = AppMode::Idle;
                }
            }
            _ => {}
        }
    }

    // Show the config editor ,  needs a restart to work
    fn show_config(&mut self, ui: &mut Ui) {
        // config editor
        // LATER need a fall back config on cancel
        ui.label("Configuration");
        ui.add_space(5.);
        ui.separator();
        ui.small("Display Mode");
        ui.checkbox(&mut self.config.dark_mode, "Darkmode");
        ui.add_space(5.);
        ui.small("Download Path");
        ui.horizontal(|ui| {
            ui.label(self.config.download_path.display().to_string());
            if ui.button("Change").clicked() {
                let mut new_path = rfd::FileDialog::new();
                new_path = new_path.set_directory(self.config.download_path.as_path());
                if let Some(path) = new_path.pick_folder() {
                    info!("new export path {}", path.display().to_string());
                    self.config.download_path = path;
                }
            }
        });
        ui.separator();

        if ui.button("Save Config").clicked() {
            let message = MessageDisplay {
                text: "Config updated".to_string(),
                mtype: MessageType::Good,
            };
            self.messages.push(message);
            // Save the config to file
            let _ = confy::store(APP_NAME, None, &self.config);
            // Push the config down to the worker
            self.cmd(Command::SendConfig(self.config.clone()));
            // Set idle
            self.mode = AppMode::Idle;
        }
    }

    // About panel
    fn about(&mut self, ui: &mut Ui) {
        ui.label(ABOUT);
        ui.add_space(10.);
        let _ = ui.hyperlink("https://github.com/zignig/sendme-egui");
        ui.add_space(10.);
        ui.separator();
        if ui.button("Awesome!").clicked() {
            self.mode = AppMode::Idle;
        }
    }

    // Show the blob ticket fetch box
    fn fetch_box(&mut self, ui: &mut Ui) {
        ui.label("Fetch blob with ticket...");
        ui.add_space(8.);
        let _ticket_edit = egui::TextEdit::multiline(&mut self.receiver_ticket)
            .desired_width(f32::INFINITY)
            .show(ui);
        ui.add_space(5.);
        ui.horizontal(|ui| {
            if ui.button("Fetch").clicked() {
                // Fetch to the default path
                self.cmd(Command::Fetch((
                    self.receiver_ticket.clone(),
                    self.config.download_path.clone(),
                )));
                self.mode = AppMode::FetchProgess;
            };
            if ui.button("Fetch Into...").clicked() {
                // Fetch to a custom path
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
        self.send_ticket = None;
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
            .stick_to_bottom(true)
            .max_width(f32::INFINITY)
            .show(ui, |ui| {
                let ui_builder = egui::UiBuilder::new();
                ui.scope_builder(ui_builder, |ui| {
                    egui::Grid::new("message_grid")
                        .num_columns(1)
                        .spacing([40.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            for message in self.messages.iter() {
                                message.show(ui);
                                ui.end_row();
                            }
                        });
                });
            });
    }

    // Send command to the worker.
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
