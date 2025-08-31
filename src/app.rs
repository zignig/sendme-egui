// The application egui front end

use core::f32;
use std::f64::consts::E;
use std::mem;

use crate::comms::{Command, Event};
use crate::worker::{Worker, WorkerHandle};
use anyhow::Result;
use eframe::NativeOptions;
use eframe::egui;
use egui::{Color32, RichText, Ui};
use rfd;
// use tracing::{info, warn};

pub struct App {
    is_first_update: bool,
    state: AppState,
}

#[derive(PartialEq)]
enum AppMode {
    Idle,
    Send,
    SendProgress,
    Fetch,
    FetchProgess,
    Finished
}

struct AppState {
    picked_path: Option<String>,
    worker: WorkerHandle,
    mode: AppMode,
    receiver_ticket: String,
    progress: f32,
    progress_text: String,
    messages: Vec<MessageDisplay>,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        if self.is_first_update {
            self.is_first_update = false;
            ctx.set_zoom_factor(1.);
            let ctx = ctx.clone();
            ctx.request_repaint();
        }
        self.state.update(ctx);
    }
}

impl App {
    pub fn run(options: NativeOptions) -> Result<(), eframe::Error> {
        let handle = Worker::spawn();
        let mut state = AppState {
            picked_path: None,
            worker: handle,
            mode: AppMode::Idle,
            receiver_ticket: String::new(),
            progress: 0.,
            progress_text: String::new(),
            messages: Vec::new(),
        };
        // TODO remove testing
        state.messages.push(MessageDisplay {
            text: "info".to_string(),
            mtype: MessageType::Info,
        });
        state.messages.push(MessageDisplay {
            text: "error".to_string(),
            mtype: MessageType::Error,
        });
        let app = App {
            is_first_update: true,
            state,
        };
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
                    if self.messages.len() > 10 {
                        let _ = self.messages.remove(0);
                    }
                    self.messages.push(MessageDisplay {
                        text: m,
                        mtype: MessageType::Info,
                    });
                }
                Event::Progress((name, value)) => {
                    self.progress = value;
                    self.progress_text = name;
                }
                Event::Finished => {
                    // Reset state
                    self.reset();
                }
            }
        }

        // active flags
        let mut send_enabled: bool = true;
        let mut receive_enabled: bool = true;
        // Use the mode
        match self.mode {
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

            }
        }

        // The actual gui
        egui::CentralPanel::default().show(ctx, |ui| {
            // Main buttons
            ui.vertical_centered(|ui| ui.heading("Sendme"));
            ui.separator();
            ui.add_space(5.);
            ui.horizontal(|ui| {
                ui.add_enabled_ui(send_enabled, |ui| {
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
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_enabled_ui(receive_enabled, |ui| {
                        if ui.button("Receive...").clicked() {
                            self.mode = AppMode::Fetch;
                        }
                    });
                });
            });
            ui.separator();
            // Show mode based widgets
            match self.mode {
                AppMode::Idle => {}
                AppMode::Send => {
                    if let Some(path) = &self.picked_path {
                        self.cmd(Command::Send(path.to_owned()));
                        self.mode = AppMode::SendProgress;
                    }
                }
                AppMode::Fetch => {
                    ui.label("Paste blob ticket.");
                    ui.add_space(8.);
                    let ticket_edit = egui::TextEdit::multiline(&mut self.receiver_ticket)
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
                    ctx.request_repaint();
                }
                AppMode::FetchProgess => {
                    let progress_bar = egui::ProgressBar::new(self.progress)
                        .text(&self.progress_text)
                        .show_percentage();
                    ui.add(progress_bar);
                    // Add a list of the messages.
                    if self.progress == 1.0 {
                        self.progress = 0.0;
                        self.mode = AppMode::Idle;
                    }
                    ctx.request_repaint();
                },
                AppMode::Finished => { 
                    self.reset();
                }
            }
            // Show the current messages
            self.show_messages(ui);
            // TODO ebug interface
            ui.separator();
            if ui.button("Reset").clicked() {
                self.reset();
            }
            if let Some(path) = &self.picked_path {
                let _ = ui.label(format!("{}", path));
            }
        });
    }

    fn reset(&mut self) {
        self.mode = AppMode::Idle;
        self.receiver_ticket = "".to_string();
        self.messages = Vec::new();
    }

    // Show the list of
    fn show_messages(&mut self, ui: &mut Ui) {
        ui.add_space(4.);
        for message in self.messages.iter() {
            message.show(ui);
            ui.add_space(4.);
        }
    }

    fn cmd(&self, command: Command) {
        self.worker
            .command_tx
            .send_blocking(command)
            .expect("Worker is not responding");
    }
}

// Some formatting for messages
enum MessageType {
    Info,
    Error,
}

struct MessageDisplay {
    text: String,
    mtype: MessageType,
}

impl MessageDisplay {
    fn show(&self, ui: &mut Ui) {
        match self.mtype {
            MessageType::Info => {
                let m = egui::RichText::new(&self.text).family(egui::FontFamily::Monospace);
                ui.label(m);
            }
            MessageType::Error => {
                let m = egui::RichText::new(&self.text)
                    .color(Color32::LIGHT_RED)
                    .family(egui::FontFamily::Monospace);
                ui.label(m);
            }
        }
    }
}
