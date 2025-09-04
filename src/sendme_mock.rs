// A mockup of the probable sendme interface

use crate::comms::MessageOut;
use anyhow::Result;
use anyhow::anyhow;
use iroh_blobs::ticket::BlobTicket;
use std::path::PathBuf;
use std::str::FromStr;
use walkdir::WalkDir;

pub async fn send(path_string: String, mess: MessageOut) -> Result<()> {
    let path = PathBuf::from(path_string);
    let files = WalkDir::new(path.clone()).into_iter();
    for file in files {
        mess.info(format!("{:?}", file).as_str()).await?;
    }
    mess.error("Error in send").await?;
    Err(anyhow!("Borked"))
}

pub async fn receive(ticket: String, mess: MessageOut) -> Result<()> {
    let blob_ticket = BlobTicket::from_str(ticket.as_str())?;
    mess.correct(format!("{:?}", blob_ticket).as_str()).await?;
    Err(anyhow!("Bad Ticket"))
}
