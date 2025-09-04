// A mockup of the probable sendme interface

use crate::comms::MessageOut;
use anyhow::Result;
use anyhow::anyhow;
use iroh_blobs::ticket::BlobTicket;
use std::path::PathBuf;
use walkdir::WalkDir;

pub async fn send(path_string: String, mess: MessageOut) -> Result<()> {
    let path = PathBuf::from(path_string);
    let files = WalkDir::new(path.clone()).into_iter();
    for file in files { 
        mess.info(format!("{:?}",file).as_str()).await?;
    }
    mess.error("Error in send").await?;
    Err(anyhow!("Borked"))
}

pub async fn reciver(ticket: BlobTicket) -> Result<()> {
    Err(anyhow!("Bad Ticket"))
}
