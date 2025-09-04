// --------------------------
// Worker
// --------------------------

use crate::comms::{Command, Event, MessageOut};
use anyhow::Result;
use async_channel::{Receiver, Sender};
use tokio::time::{self, Duration};
use tracing::{info, warn};

use crate::sendme_mock::{send};

pub struct Worker {
    pub command_rx: Receiver<Command>,
    pub event_tx: Sender<Event>,
    // TODO add worker state
    pub mess: MessageOut,
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

    async fn start(
        command_rx: async_channel::Receiver<Command>,
        event_tx: async_channel::Sender<Event>,
    ) -> Result<Self> {
        let ev_tx_clone = event_tx.clone();
        Ok(Self {
            command_rx,
            event_tx,
            mess: MessageOut::new(ev_tx_clone),
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
                        self.mess.error(format!("Error : {}",err).as_str()).await?;
                        warn!("command failed {err}");
                    }
                }
            }
        }
    }

    // This is currently mocked
    // TODO rework sendme.
    async fn handle_command(&mut self, command: Command) -> Result<()> {
        match command {
            Command::Setup => {
                self.mess.correct("correct").await?;
                self.mess.info("info").await?;
                self.mess.error("error").await?;
                return Ok(());
            }
            Command::Send(path) => {
                send(path,self.mess.clone()).await?;
                return Ok(());
            }
            Command::Fetch(ticket) => {
                self.mess.info("info test").await?;
                const MAX: i32 = 100;
                let mut ticker = time::interval(Duration::from_millis(20));
                let mut counter = 0;
                info!("{}", ticket);
                loop {
                    counter += 1;
                    ticker.tick().await;
                    let value = (counter as f32) / (MAX as f32);
                    self.mess
                        .info(format!("counter {}", value).as_str())
                        .await?;
                    if counter == MAX {
                        self.mess.finished().await?;
                        return Ok(());
                    }
                }
                
                self.mess.correct("finished").await?;
            }
        }
    }
}
