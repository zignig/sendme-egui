// --------------------------
// Worker
// --------------------------

use crate::comms::{Command, Event};
use anyhow::Result;
use tokio::time::{self, Duration};
use async_channel::{Receiver, Sender};
use tracing::{info, warn};


pub struct Worker {
    pub command_rx: Receiver<Command>,
    pub event_tx: Sender<Event>,
    // TODO add worker state
}

pub struct WorkerHandle {
    pub command_tx: Sender<Command>,
    pub event_rx: Receiver<Event>,
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
                    let command = command?;
                    info!("command {:?}",command);
                    if let Err(err ) = self.handle_command(command).await{
                        warn!("command failed {err}");
                    }
                }
            }
        }
    }

    async fn handle_command(&mut self, command: Command) -> Result<()> {
        match command {
            Command::Message => self.emit(Event::Message("hello".to_string())).await,
            Command::Send(_) => {
                return Ok(());
            }
            Command::Receive(mess) => {
                const MAX: i32 = 100;
                let mut ticker = time::interval(Duration::from_millis(50));
                let mut counter = 0;
                info!("{}",mess);
                loop {
                    counter += 1;
                    ticker.tick().await;
                    let value = (counter as f32) / (MAX as f32);
                    self.emit(Event::Progress(("Fetching...".to_string(), value))).await;
                    self.emit(Event::Message(format!("counter {}",&value))).await;
                    // info!("progress {}",value);
                    if counter == MAX {
                        return Ok(());
                    }
                }
            }
        }
    }
}
