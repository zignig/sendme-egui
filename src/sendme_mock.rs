// A mockup of the probable sendme interface
// with a view to convert original sendme to this egui

use crate::comms::MessageOut;
use anyhow::Result;
use anyhow::anyhow;
use iroh_blobs::ticket::BlobTicket;
use std::path::PathBuf;
use std::str::FromStr;
use tokio::time::{self, Duration};
use tracing::info;
use walkdir::WalkDir;

// Mock of offser folder in iroh-blobs
pub async fn send(path_string: String, mess: MessageOut) -> Result<()> {
    let path = PathBuf::from(path_string);
    let files = WalkDir::new(path.clone()).into_iter();
    for file in files {
        if let Ok(file) = file {
            mess.info(format!("{:?}", file.path()).as_str()).await?;
        };
    }
    Err(anyhow!("Send Fail"))
}

// fetch a blob from the iroh network
pub async fn receive(ticket: String, mess: MessageOut) -> Result<()> {
    if ticket == "".to_string() { 
        return Err(anyhow!("Empty Blob"));
    }
    let blob_ticket = BlobTicket::from_str(ticket.as_str())?;
    mess.correct(format!("nodeid : {:?}", blob_ticket.node_addr().node_id).as_str())
        .await?;
    mess.correct(format!("hash : {:?}", blob_ticket.hash()).as_str())
        .await?;
    const MAX: usize = 100;
    let mut ticker = time::interval(Duration::from_millis(20));
    let mut counter = 0;
    info!("{}", ticket);
    loop {
        counter += 1;
        ticker.tick().await;
        mess.progress("Downloading", counter, MAX).await?;
        if counter == MAX {
            // mess.finished().await?;
            return Ok(());
        }
    }
}
