// Comms between the gui and  the worker in it's own module.
// Some of this lives on both sides ( be careful )

use anyhow::Result;
use async_channel::Sender;
use eframe::egui;
use egui::{Color32, Ui};

// Incoming events
#[derive(Clone)]
pub enum Event {
    Message(MessageDisplay),
    Progress((String, usize, usize)),
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
