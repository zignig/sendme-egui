// Comms between the gui and  the worker in it's own module.
// Some of this lives on both sides ( be careful )

use std::collections::BTreeMap;

use anyhow::Result;
use async_channel::Sender;
use eframe::egui;
use egui::{Color32, Ui};

// Incoming events
#[derive(Clone)]
pub enum Event {
    Message(MessageDisplay),
    Progress((String, usize, usize)),
    ProgressFinished(String),
    Finished,
}

// Outgoing Commands
#[derive(Debug)]
pub enum Command {
    Setup,
    Send(String),
    Fetch(String),
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

pub struct MessageOut {
    event_tx: Sender<Event>,
}

impl MessageOut {
    pub fn new(event_tx: Sender<Event>) -> Self {
        Self { event_tx }
    }

    pub async fn info(&self, message: &str) -> Result<()> {
        self.event_tx
            .send(Event::Message(MessageDisplay {
                text: message.to_string(),
                mtype: MessageType::Info,
            }))
            .await?;
        Ok(())
    }

    pub async fn correct(&self, message: &str) -> Result<()> {
        self.event_tx
            .send(Event::Message(MessageDisplay {
                text: message.to_string(),
                mtype: MessageType::Good,
            }))
            .await?;
        Ok(())
    }

    pub async fn error(&self, message: &str) -> Result<()> {
        self.event_tx
            .send(Event::Message(MessageDisplay {
                text: message.to_string(),
                mtype: MessageType::Error,
            }))
            .await?;
        Ok(())
    }

    pub async fn finished(&self) -> Result<()> {
        self.event_tx
            .send(Event::Message(MessageDisplay {
                text: "finished...".to_string(),
                mtype: MessageType::Good,
            }))
            .await?;
        self.event_tx.send(Event::Finished).await?;
        Ok(())
    }

    pub async fn progress(&self, name: &str, current: usize, total: usize) -> Result<()> {
        // info!("progress {} / {} ",current,total);
        self.event_tx
            .send(Event::Progress((name.to_string(), current, total)))
            .await?;
        Ok(())
    }

    pub async fn complete(&self, name: &str) -> Result<()> {
        // info!("progress {} / {} ",current,total);
        self.event_tx
            .send(Event::ProgressFinished(name.to_string()))
            .await?;
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

// Progress Bars
//

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
        let prog_val = (self.current as f32) / (self.total as f32);
        let mut progress_bar = egui::ProgressBar::new(prog_val).show_percentage(); 
        if self.complete { 
            progress_bar = progress_bar.fill(Color32::DARK_GREEN);
        }
        ui.add(progress_bar);
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

    pub fn show(&self, ui: &mut Ui) {
        for (_, item) in self.bars.iter() {
            item.show(ui);
        }
    }

    pub fn clear(&mut self) {
        self.bars = BTreeMap::new();
    }
}
