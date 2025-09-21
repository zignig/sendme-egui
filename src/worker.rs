// --------------------------
// Worker
// --------------------------

use std::{path::PathBuf, time::Duration};

use crate::comms::{Command, Event, MessageOut};
use anyhow::Result;
use async_channel::{Receiver, Sender};
use tokio::time::{Instant, interval};
use tracing::{info, warn};

use crate::transport::{receive, send};
pub struct Worker {
    pub command_rx: Receiver<Command>,
    // pub event_tx: Sender<Event>,
    // TODO add worker state
    pub mess: MessageOut,
    pub start_time: Instant,
    pub running: bool,
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
                .expect("failed to start tokio runtime");
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

    async fn start(
        command_rx: async_channel::Receiver<Command>,
        event_tx: async_channel::Sender<Event>,
    ) -> Result<Self> {
        let ev_tx_clone = event_tx.clone();
        Ok(Self {
            command_rx,
            mess: MessageOut::new(ev_tx_clone),
            start_time: Instant::now(),
            running: true,
        })
    }

    async fn run(&mut self) -> Result<()> {
        // the actual runner for the worker
        // TODO add elapsed timer ticker, send message to the gui.

        // TODO
        // I think this needs to be a join rather than a select
        // get both going at the same time.

        info!("Starting  the worker");

        // TODO strip this out into a separate function
        // with it's own async channel.
        
        let mut interval = interval(Duration::from_millis(1000));
        let m2 = self.mess.clone();
        let _ = tokio::spawn(async move {
            let start_time = Instant::now();
            loop {
                interval.tick().await;
                // info!("tick");
                    let since = start_time.elapsed().as_secs();
                    m2.tick(since).await;
            }
        });
        loop {
            tokio::select! {
                command = self.command_rx.recv() => {
                    let command = command?;
                    if let Err(err ) = self.handle_command(command).await{
                        self.mess.error(format!("{}",err).as_str()).await?;
                        warn!("command failed {err}");
                    }
                }
            }
        }
    }

    // handle the incoming commands from the egui
    async fn handle_command(&mut self, command: Command) -> Result<()> {
        match command {
            // TODO move the update callback here and get rid of
            // the update message
            Command::Setup { callback } => {
                let _ = self.mess.set_callback(callback).await?;
                self.mess.correct("Ready...").await?;
                // self.mess.info("info").await?;
                // self.mess.error("error").await?;
                return Ok(());
            }
            // This needs commands to finish
            Command::Send(path) => {
                send(path, self.mess.clone()).await?;
                return Ok(());
            }
            Command::Fetch((ticket, target)) => {
                let target_path = PathBuf::from(target);
                self.start_timer().await?;
                match receive(ticket, target_path, self.mess.clone()).await {
                    Ok(_) => {
                        self.reset_timer().await?;
                        self.mess.finished().await?;
                    }
                    Err(err) => {
                        self.reset_timer().await?;
                        return Err(err);
                    }
                };
                return Ok(());
            } // Command::SetUpdateCallBack { callback } => {
              //     //  Set up  the callback
              //     self.mess.set_callback(callback);
              //     return Ok(());
              // }
        }
    }

    async fn start_timer(&mut self) -> Result<()> {
        warn!("Start Time");
        self.start_time = Instant::now();
        self.running = true;
        Ok(())
    }

    async fn reset_timer(&mut self) -> Result<()> {
        warn!("stop timer");
        self.running = false;
        self.mess.reset_timer().await?;
        Ok(())
    }
}
