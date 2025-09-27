use crate::comms::MessageOut;
use anyhow::Result;
use anyhow::anyhow;
use chrono::Local;
use humansize::{DECIMAL, format_size};
use iroh_blobs::api::Store;
use iroh_blobs::api::blobs::ExportMode;
use iroh_blobs::api::blobs::ExportOptions;
use iroh_blobs::api::blobs::ExportProgressItem;
use iroh_blobs::api::remote::GetProgressItem;
use iroh_blobs::format::collection::Collection;
use iroh_blobs::get::Stats;
use iroh_blobs::get::request::get_hash_seq_and_sizes;
use iroh_blobs::store::fs::FsStore;
use iroh_blobs::ticket::BlobTicket;
use n0_future::StreamExt;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use tracing::{info, warn};

use iroh::{Endpoint, RelayMode, discovery::dns::DnsDiscovery};

// fetch a blob from the iroh network
pub async fn receive(ticket: String, target: PathBuf, mess: MessageOut, db: FsStore) -> Result<()> {
    if ticket == "".to_string() {
        return Err(anyhow!("Empty Blob"));
    }
    let ticket = BlobTicket::from_str(ticket.as_str())?;

    // TODO move these up into the worker.
    let addr = ticket.node_addr().clone();
    let secret_key = super::get_or_create_secret()?;
    let mut builder = Endpoint::builder()
        .alpns(vec![])
        .secret_key(secret_key)
        .relay_mode(RelayMode::Default);

    // if ticket.node_addr().relay_url.is_none() && ticket.node_addr().direct_addresses.is_empty() {
    builder = builder.add_discovery(DnsDiscovery::n0_dns());
    // }
    let endpoint = builder.bind().await?;
    mess.info("Local endpoint created...").await?;

    // let db2 = db.clone();
    warn!("Node built");

    // Now run the fetch
    let (stats, total_files, payload_size) = {
        let hash_and_format = ticket.hash_and_format();
        info!("computing local");
        let local = db.remote().local(hash_and_format).await?;
        info!("got local");
        let (stats, total_files, payload_size) = if !local.is_complete() {
            mess.info("Unfinished Download...").await?;
            let connection = endpoint.connect(addr, iroh_blobs::protocol::ALPN).await?;
            mess.correct("Connection Established").await?;
            let (_hash_seq, sizes) =
                get_hash_seq_and_sizes(&connection, &hash_and_format.hash, 1024 * 1024 * 32, None)
                    .await?;
            // .map_err(show_get_error)?;
            let total_size = sizes.iter().copied().sum::<u64>();
            let payload_size = sizes.iter().skip(2).copied().sum::<u64>();
            let total_files = (sizes.len().saturating_sub(1)) as u64;
            mess.info(format!("total size: {}", format_size(total_size, DECIMAL)).as_str())
                .await?;
            eprintln!(
                "getting collection {} {} files, {}",
                &ticket.hash().to_hex().to_string(),
                total_files,
                payload_size
            );
            // Fetch the file
            let get = db.remote().execute_get(connection, local.missing());
            let stats = Stats::default();
            let mut stream = get.stream();
            while let Some(item) = stream.next().await {
                match item {
                    GetProgressItem::Progress(offset) => {
                        mess.progress("Download", offset as usize, payload_size as usize)
                            .await?;
                    }
                    GetProgressItem::Done(value) => {
                        mess.correct("Done").await?;
                        mess.complete("Download").await?;
                        mess.info(format!("bytes read {}", value.payload_bytes_read).as_str())
                            .await?;
                    }
                    GetProgressItem::Error(_value) => {
                        anyhow::bail!(anyhow!("stream"));
                    }
                }
            }
            // Set a tag for later work
            let dt = Local::now().to_rfc3339().to_owned();
            db.tags().set(format!("incoming-{}", dt), ticket.hash()).await?;
            (stats, total_files, payload_size)
        } else {
            mess.correct("Blob is complete and local!").await?;
            let total_files = local.children().unwrap() - 1;
            let payload_bytes = 0;
            (Stats::default(), total_files, payload_bytes)
        };
        let collection = Collection::load(hash_and_format.hash, db.as_ref()).await?;
        export(&db, collection, target, mess.clone()).await?;
        (stats, total_files, payload_size)
    };
    mess.correct(format!("{:#?}", stats).as_str()).await?;
    mess.correct(format!("{}", total_files).as_str()).await?;
    mess.correct(format!("{}", payload_size).as_str()).await?;
    Ok(())
}

pub async fn export(
    db: &Store,
    collection: Collection,
    target_dir: PathBuf,
    mess: MessageOut,
) -> Result<()> {
    let len = collection.len();
    for (i, (name, hash)) in collection.iter().enumerate() {
        // info!("file name {}", name);
        let target = get_export_path(&target_dir, name)?;
        info!("target {:#?}", target.display());
        if target.exists() {
            info!(
                "target {} already exists. Export stopped.",
                target.display()
            );
            anyhow::bail!("{} already exists", target.display());
        }
        mess.progress("Export", i + 1, len).await?;
        let mut stream = db
            .export_with_opts(ExportOptions {
                hash: *hash,
                target,
                mode: ExportMode::Copy,
            })
            .stream()
            .await;
        while let Some(item) = stream.next().await {
            match item {
                ExportProgressItem::Size(_size) => {
                    // pb.set_length(size);
                }
                ExportProgressItem::CopyProgress(_offset) => {
                    // pb.set_position(offset);
                }
                ExportProgressItem::Done => {
                    // pb.finish_and_clear();
                }
                ExportProgressItem::Error(cause) => {
                    // pb.finish_and_clear();
                    anyhow::bail!("error exporting {}: {}", name, cause);
                }
            }
        }
    }
    mess.complete("Export").await?;
    Ok(())
}

fn get_export_path(root: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let parts = name.split('/');
    let mut path = root.to_path_buf();
    for part in parts {
        validate_path_component(part)?;
        path.push(part);
    }
    Ok(path)
}

fn validate_path_component(component: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !component.contains('/'),
        "path components must not contain the only correct path separator, /"
    );
    Ok(())
}