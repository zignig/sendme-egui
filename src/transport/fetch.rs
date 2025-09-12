use crate::comms::MessageOut;
use anyhow::Result;
use anyhow::anyhow;
use directories::BaseDirs;
use iroh_blobs::api::Store;
use iroh_blobs::api::remote::GetProgressItem;
use iroh_blobs::format::collection::Collection;
use iroh_blobs::get::GetError;
use iroh_blobs::get::Stats;
use iroh_blobs::get::request::get_hash_seq_and_sizes;
use iroh_blobs::ticket::BlobTicket;
use n0_future::{StreamExt, task::AbortOnDropHandle};
use std::str::FromStr;
use tokio::{select, sync::mpsc};
use tracing::{info, warn};

use iroh::{
    Endpoint, NodeAddr, RelayMode, RelayUrl, SecretKey, Watcher,
    discovery::{dns::DnsDiscovery, pkarr::PkarrPublisher},
};

// fetch a blob from the iroh network
pub async fn receive(ticket: String, mess: MessageOut) -> Result<()> {
    if ticket == "".to_string() {
        return Err(anyhow!("Empty Blob"));
    }
    let ticket = BlobTicket::from_str(ticket.as_str())?;
    // mess.correct(format!("nodeid : {:?}", ticket.node_addr().node_id).as_str())        .await?;
    // mess.correct(format!("hash : {:?}", ticket.hash()).as_str())        .await?;
    let addr = ticket.node_addr().clone();
    let secret_key = super::get_or_create_secret(true)?;
    let mut builder = Endpoint::builder().alpns(vec![]).secret_key(secret_key);
    // .relay_mode(args.common.relay.into());

    if ticket.node_addr().relay_url.is_none() && ticket.node_addr().direct_addresses.is_empty() {
        builder = builder.add_discovery(DnsDiscovery::n0_dns());
    }
    let endpoint = builder.bind().await?;

    // Use a local user data filder
    let iroh_data_dir = match BaseDirs::new() {
        Some(base_dirs) => base_dirs
            .data_dir()
            .to_owned()
            .join("sendme-egui")
            .join("blob_data"),
        None => return Err(anyhow!("Can't create data directory")),
    };
    println!("{:#?}", iroh_data_dir);
    let db = iroh_blobs::store::fs::FsStore::load(&iroh_data_dir).await?;
    let db2 = db.clone();
    warn!("Node built");

    // Now run the fetch
    let fut = async move {
        let hash_and_format = ticket.hash_and_format();
        info!("computing local");
        let local = db.remote().local(hash_and_format).await?;
        let (stats, total_files, payload_size) = if !local.is_complete() {
            let connection = endpoint.connect(addr, iroh_blobs::protocol::ALPN).await?;
            let (_hash_seq, sizes) =
                get_hash_seq_and_sizes(&connection, &hash_and_format.hash, 1024 * 1024 * 32, None)
                    .await?;
            // .map_err(show_get_error)?;
            let total_size = sizes.iter().copied().sum::<u64>();
            let payload_size = sizes.iter().skip(2).copied().sum::<u64>();
            let total_files = (sizes.len().saturating_sub(1)) as u64;
            eprintln!(
                "getting collection {} {} files, {}",
                &ticket.hash().to_hex().to_string(),
                total_files,
                payload_size
            );
            // Fetch the file
            let get = db.remote().execute_get(connection, local.missing());
            let mut stats = Stats::default();
            let mut stream = get.stream();
            while let Some(item) = stream.next().await {
                match item {
                    GetProgressItem::Progress(offset) => {
                        // info!("{:#?}", offset);
                        mess.progress("Download", offset as usize, payload_size as usize)
                            .await?;
                    }
                    GetProgressItem::Done(value) => {
                        // info!("Done {:#?}", value);
                        mess.correct("Done").await?;
                        mess.info(format!("bytes read {}", value.payload_bytes_read).as_str())
                            .await?;
                    }
                    GetProgressItem::Error(value) => {
                        anyhow::bail!(anyhow!("stream"));
                    }
                }
            }
            (stats, total_files, payload_size)
        } else {
            mess.correct("Already Complete!").await?;
            let total_files = local.children().unwrap() - 1;
            let payload_bytes = 0;
            (Stats::default(), total_files, payload_bytes)
        };
        let collection = Collection::load(hash_and_format.hash, db.as_ref()).await?;
        export(&db,collection,mess.clone()).await?;
        anyhow::Ok((total_files, payload_size, stats))
    };
    // Follow the files and wait for event
    let (total_files, payload_size, stats) = select! {
        x = fut => match x {
            Ok(x) => x,
            Err(e) => {
                // make sure we shutdown the db before exiting
                db2.shutdown().await?;
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        },
        _ = tokio::signal::ctrl_c() => {
            db2.shutdown().await?;
            std::process::exit(130);
        }
    };
    Ok(())
}

pub async fn export(db: &Store, collection: Collection, mess: MessageOut) -> Result<()> {
    let len = collection.len();
    for (i,(name,hash)) in collection.iter().enumerate() {
        mess.progress("export",i,len).await?;
        mess.info(format!("{}",name).as_str()).await?;
    };
    Ok(())
}

// const MAX: usize = 100;
// let mut ticker = time::interval(Duration::from_millis(20));
// let mut counter = 0;
// info!("{}", ticket);
// loop {
//     counter += 1;
//     ticker.tick().await;
//     mess.progress("Downloading", counter, MAX).await?;
//     if counter == MAX {
//         // mess.finished().await?;
//         return Ok(());
//     }
// }

// fn show_get_error(e: GetError) -> GetError {
//     match &e {
//         GetError::NotFound { .. } => {
//             eprintln!("{}", style("send side no longer has a file").yellow())
//         }
//         GetError::RemoteReset { .. } => eprintln!("{}", style("remote reset").yellow()),
//         GetError::NoncompliantNode { .. } => {
//             eprintln!("{}", style("non-compliant remote").yellow())
//         }
//         GetError::Io { source, .. } => eprintln!(
//             "{}",
//             style(format!("generic network error: {source}")).yellow()
//         ),
//         GetError::BadRequest { .. } => eprintln!("{}", style("bad request").yellow()),
//         GetError::LocalFailure { source, .. } => {
//             eprintln!("{} {source:?}", style("local failure").yellow())
//         }
//     }
//     e
// }
