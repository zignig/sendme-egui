// --------------------------
// Worker
// --------------------------

use std::{path::PathBuf, time::Duration};

use crate::comms::{Command, Event, MessageOut};
use anyhow::Result;
use async_channel::{Receiver, Sender};
use iroh_blobs::store::fs::FsStore;
use tokio::time::{Instant, interval};
use tracing::{info, warn};

use crate::transport::{receive, send};

pub struct Worker {
    pub command_rx: Receiver<Command>,
    // pub event_tx: Sender<Event>,
    // TODO add worker state
    pub mess: MessageOut,
    pub timer_out: Sender<TimerCommands>,
    pub store_path: PathBuf,
    pub store: FsStore,
}

pub struct WorkerHandle {
    pub command_tx: Sender<Command>,
    pub event_rx: Receiver<Event>,
}

impl Worker {
    pub fn spawn(store_path: PathBuf) -> WorkerHandle {
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
                let mut worker = Worker::start(command_rx, event_tx, store_path)
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
        store_path: PathBuf,
    ) -> Result<Self> {
        let mess = MessageOut::new(event_tx.clone());
        // Timer
        let m2 = mess.clone();
        let (timer_out, timer_in) = async_channel::bounded(16);
        let timer = TimerTask::new(m2);
        timer.run(timer_in);
        let store = iroh_blobs::store::fs::FsStore::load(&store_path).await?;
        Ok(Self {
            command_rx,
            mess,
            timer_out: timer_out,
            store_path,
            store,
        })
    }

    async fn run(&mut self) -> Result<()> {
        // the actual runner for the worker
        info!("Starting  the worker");
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
                self.mess
                    .info(format!("{}", self.store_path.display()).as_str())
                    .await?;
                return Ok(());
            }
            // This needs commands to finish 
            // TODO add a cancellation ticket in here.
            Command::Send(path) => {
                self.start_timer().await?;
                match send(path, self.mess.clone(), self.store.clone()).await {
                    Ok(_) => {
                        self.reset_timer().await?;
                        self.mess.finished().await?
                    }
                    Err(err) => {
                        self.reset_timer().await?;
                        return Err(err);
                    }
                }
                return Ok(());
            }
            // This is working. 
            // TODO needs a cancellation ticket too.
            Command::Fetch((ticket, target)) => {
                let target_path = PathBuf::from(target);
                self.start_timer().await?;
                match receive(ticket, target_path, self.mess.clone(), self.store.clone()).await {
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
            }
        }
    }

    // -----
    // Timer functions 
    //------
    
    async fn start_timer(&mut self) -> Result<()> {
        warn!("Start Timer");
        self.timer_out.send(TimerCommands::Start).await?;
        Ok(())
    }

    async fn reset_timer(&mut self) -> Result<()> {
        warn!("Stop timer");
        self.timer_out.send(TimerCommands::Reset).await?;
        Ok(())
    }
}

// ----------
// Timer runner
// ----------

#[derive(Debug)]
pub enum TimerCommands {
    Start,
    Reset,
}

pub struct TimerTask {
    mess: MessageOut,
}

// Runs as a seperate tokio task, boops every second
// Only sends time if its running
impl TimerTask {
    pub fn new(mess: MessageOut) -> Self {
        Self { mess }
    }

    pub fn run(self, incoming: Receiver<TimerCommands>) {
        let _ = tokio::spawn(async move {
            // every second
            let mut interval = interval(Duration::from_millis(1000));
            let mut running = true;
            let mess = self.mess.clone();
            let mut start_time = Instant::now();
            loop {
                tokio::select! {
                    command  = incoming.recv() => {
                       let command = command.unwrap() ;
                       info!("{:?}",command);
                       match command {
                        TimerCommands::Start => { start_time = Instant::now(); running = true;},
                        TimerCommands::Reset => { running = false ; let _ = mess.reset_timer().await; } ,
                      };
                    }
                    _ = interval.tick() => {
                    if running {
                        let since = start_time.elapsed().as_secs();
                        let _ = mess.tick(since).await;
                    }
                    }
                }
            }
        });
    }
}
