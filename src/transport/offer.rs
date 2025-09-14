// Share a file onto the blob network
// TODO
// rip send out of the sendme

use crate::comms::MessageOut;
use anyhow::Result;
use anyhow::anyhow;
use std::path::PathBuf;
use walkdir::WalkDir;

// Mock of offser folder in iroh-blobs
pub async fn send(path: PathBuf,mess: &mut MessageOut) -> Result<()> {
    let files = WalkDir::new(path.clone()).into_iter();
    for file in files {
        if let Ok(file) = file {
            mess.info(format!("{:?}", file.path().display()).as_str())
                .await?;
        };
    }
    Err(anyhow!("Send Fail"))
}
 