// The application egui front end

use anyhow::Result;
use async_channel::{Receiver, Sender};
use eframe::NativeOptions;
use eframe::egui;
use rfd;
use tracing::{info, warn};

pub struct App {
    is_first_update: bool,
    state: AppState,
}

struct AppState {
    picked_path: Option<String>,
    worker: WorkerHandle,
    message: Option<String>
}

impl eframe::App for App {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        if self.is_first_update {
            self.is_first_update = false;
            ctx.set_zoom_factor(1.5);
            let ctx = ctx.clone();
            ctx.request_repaint();
        }
        self.state.update(ctx);
    }
}

impl App {
    pub fn run(options: NativeOptions) -> Result<(), eframe::Error> {
        let handle = Worker::spawn();
        let state = AppState {
            picked_path: None,
            worker: handle,
            message: None
        };
        let app = App {
            is_first_update: true,
            state,
        };
        eframe::run_native("sendme-egui", options, Box::new(|_cc| Ok(Box::new(app))))
    }
}

impl AppState {
    fn update(&mut self, ctx: &egui::Context) {
        // Events from the worker
        while let Ok(event) = self.worker.event_rx.try_recv(){ 
            match event {
                Event::Message(m) => self.message = Some(m),
            }
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            if ui.button("Open file…").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.picked_path = Some(path.display().to_string());
                }
            }
            if ui.button("Click").clicked(){
                self.cmd(Command::Message("Hello".to_string()));
            }
            ui.separator();
            if let Some(path) = &self.picked_path {
                let _ = ui.label(format!("{}", path));
            }
            ui.separator();
            if let Some(mes) = &self.message { 
                ui.label(format!("{}",mes));
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

// --------------------------
// Worker
// --------------------------

// Incoming events
enum Event {
    Message(String),
}

// Outgoing Commands
#[derive(Debug)]
enum Command {
    Message(String),
}

struct Worker {
    command_rx: Receiver<Command>,
    event_tx: Sender<Event>,
    // TODO add worker state
}

struct WorkerHandle {
    command_tx: Sender<Command>,
    event_rx: Receiver<Event>,
}

impl Worker {
    pub fn spawn() -> WorkerHandle {
        let (command_tx, command_rx) = async_channel::bounded(16);
        let (event_tx, event_rx) = async_channel::bounded(16);
        let handle = WorkerHandle {
            command_tx,
            event_rx,
        };
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failedd to start tokio runtime");
            rt.block_on(async move {
                let mut worker = Worker::start(command_rx, event_tx)
                    .await
                    .expect("Worker failed to start");
                if let Err(err) = worker.run().await {
                    warn!("worker stopped with error {err:?}");
                }
            })
        });
        handle
    }

    async fn emit(&self, event: Event) -> Result<()> {
        self.event_tx.send(event).await?;
        Ok(())
    }

    async fn start(
        command_rx: async_channel::Receiver<Command>,
        event_tx: async_channel::Sender<Event>,
    ) -> Result<Self> {
        Ok(Self {
            command_rx,
            event_tx,
        })
    }

    async fn run(&mut self) -> Result<()> {
        // the actual runner for the worker
        info!("Starting  the worker");
        loop {
            tokio::select! {
                command = self.command_rx.recv() => {
                    info!("command {:?}",command);
                    self.emit(Event::Message("from worker".to_string())).await?;
                }
            }
        }
        Ok(())
    }
}
