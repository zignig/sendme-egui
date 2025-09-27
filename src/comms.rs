// Comms between the gui and  the worker in it's own module.
// Some of this lives on both sides ( be careful )

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use anyhow::Result;
use async_channel::Sender;
use eframe::egui::{self};

use egui::{Color32, Ui};
use tokio::sync::Mutex;

// Update Callback
type UpdateCallback = Box<dyn Fn() + Send + 'static>;

// Incoming events
pub enum Event {
    Message(MessageDisplay),
    Progress((String, usize, usize)),
    ProgressFinished(String),
    ProgressComplete(String),
    SendTicket(String),
    Tick(u64),
    StopTick,
    Finished,
}

// Outgoing Commands
pub enum Command {
    Setup { callback: UpdateCallback },
    Send(PathBuf),
    Fetch((String, PathBuf)),
    CancelTest
}

// Message types
#[derive(Clone)]
enum MessageType {
    Good,
    Info,
    Error,
}

// egui display struct
#[derive(Clone)]
pub struct MessageDisplay {
    text: String,
    mtype: MessageType,
}

// Messaging
#[derive(Clone)]
pub struct MessageOut(Arc<Mutex<MessageInner>>);

pub struct MessageInner {
    event_tx: Sender<Event>,
    callback: Option<UpdateCallback>,
}

impl MessageOut {
    pub fn new(event_tx: Sender<Event>) -> Self {
        Self(Arc::new(Mutex::new(MessageInner {
            event_tx,
            callback: None,
        })))
    }

    pub async fn set_callback(&self, callback: UpdateCallback) -> Result<()> {
        let mut value = self.0.lock().await;
        value.callback = Some(callback);
        Ok(())
    }

    async fn emit(&self, event: Event) -> Result<()> {
        let binding = self.0.lock().await;
        if let Some(callback) = &binding.callback {
            callback();
        }
        binding.event_tx.send(event).await.unwrap();
        Ok(())
    }

    pub async fn info(&self, message: &str) -> Result<()> {
        self.emit(Event::Message(MessageDisplay {
            text: message.to_string(),
            mtype: MessageType::Info,
        }))
        .await?;
        Ok(())
    }

    pub async fn correct(&self, message: &str) -> Result<()> {
        self.emit(Event::Message(MessageDisplay {
            text: message.to_string(),
            mtype: MessageType::Good,
        }))
        .await?;
        Ok(())
    }

    pub async fn error(&self, message: &str) -> Result<()> {
        self.emit(Event::Message(MessageDisplay {
            text: message.to_string(),
            mtype: MessageType::Error,
        }))
        .await?;
        Ok(())
    }

    pub async fn finished(&self) -> Result<()> {
        self.emit(Event::Message(MessageDisplay {
            text: "Finished...".to_string(),
            mtype: MessageType::Good,
        }))
        .await?;
        self.emit(Event::Finished).await?;
        Ok(())
    }

    pub async fn progress(&self, name: &str, current: usize, total: usize) -> Result<()> {
        self.emit(Event::Progress((name.to_string(), current, total)))
            .await?;
        Ok(())
    }

    pub async fn complete(&self, name: &str) -> Result<()> {
        self.emit(Event::ProgressFinished(name.to_string())).await?;
        Ok(())
    }

    pub async fn progress_finish(&self, name: &str) -> Result<()> {
        self.emit(Event::ProgressComplete(name.to_string())).await?;
        Ok(())
    }

    pub async fn tick(&self, since: u64) -> Result<()> {
        self.emit(Event::Tick(since)).await?;
        Ok(())
    }

    pub async fn reset_timer(&self) -> Result<()> {
        self.emit(Event::StopTick).await?;
        Ok(())
    }

    pub async fn send_ticket(&self, ticket: String) -> Result<()> { 
        self.emit(Event::SendTicket(ticket)).await?;
        Ok(())
    }
}

// Message formatting
impl MessageDisplay {
    pub fn show(&self, ui: &mut Ui) {
        match self.mtype {
            MessageType::Good => {
                let m = egui::RichText::new(&self.text)
                    .color(Color32::LIGHT_GREEN)
                    .family(egui::FontFamily::Monospace);
                ui.label(m);
            }
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


// --------
// Progress Bars
// --------

struct ProgressBar {
    name: String,
    current: usize,
    total: usize,
    complete: bool,
    item: Option<String>,
}

impl ProgressBar {
    pub fn show(&self, ui: &mut Ui) {
        ui.add_space(2.);
        ui.small(self.name.to_string());
        ui.add_space(2.);
        let prog_val = if self.current == self.total {
            1.
        } else {
            (self.current as f32) / (self.total as f32)
        };
        let mut progress_bar = egui::ProgressBar::new(prog_val)
            .show_percentage()
            .desired_height(12.);
        if self.complete {
            progress_bar = progress_bar.fill(Color32::DARK_GREEN);
        }
        ui.add(progress_bar);
        if let Some(item) = &self.item {
            ui.small(item);
        }
    }
}

pub struct ProgressList {
    bars: BTreeMap<String, ProgressBar>,
}

impl ProgressList {
    pub fn new() -> Self {
        Self {
            bars: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, name: String, current: usize, total: usize) {
        if let Some(item) = self.bars.get_mut(&name) {
            item.name = name;
            item.current = current;
            item.total = total;
        } else {
            self.bars.insert(
                name.to_owned(),
                ProgressBar {
                    name: name,
                    current,
                    total,
                    complete: false,
                    item: None,
                },
            );
        }
    }

    pub fn complete(&mut self, name: String) {
        if let Some(item) = self.bars.get_mut(&name) {
            item.complete = true;
        }
    }

    pub fn finish(&mut self, name: String) {
        self.bars.remove(&name);
    }

    pub fn show(&self, ui: &mut Ui) {
        for (_, item) in self.bars.iter() {
            item.show(ui);
        }
    }

    pub fn clear(&mut self) {
        self.bars = BTreeMap::new();
    }
}
