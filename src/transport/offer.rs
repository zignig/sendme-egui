// Share a file onto the blob network
// rip send out of the sendme
// This is a cut and paste from sendme bits that have been updated
// to use message and progress bars

use crate::comms::MessageOut;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use chrono::Local;
use futures_buffered::BufferedStreamExt;
use iroh::Endpoint;
use iroh::RelayMode;
use iroh::Watcher;
use iroh::discovery::dns::DnsDiscovery;
use iroh_blobs::BlobFormat;
use iroh_blobs::BlobsProtocol;
use iroh_blobs::api::TempTag;
use iroh_blobs::api::blobs::AddPathOptions;
use iroh_blobs::api::blobs::AddProgressItem;
use iroh_blobs::api::blobs::ImportMode;
use iroh_blobs::format::collection::Collection;
use iroh_blobs::store::fs::FsStore;
use iroh_blobs::ticket::BlobTicket;
use n0_future::StreamExt;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
// use tracing::info;
use walkdir::WalkDir;

// Not a mock anymore , breakdown.
// TODO , cancellation feed to stop.

pub async fn send(path: PathBuf, mess: MessageOut, store: FsStore) -> Result<()> {
    // Import the files into the blob store
    let (tag, size, collection) = import(path, &store, mess.clone()).await?;
    // Set a tag for later work
    let dt = Local::now().to_rfc3339().to_owned();
    store
        .tags()
        .set(format!("outgoing-{}", dt), tag.hash().to_owned())
        .await?;

    mess.correct(format!("{:?}", tag).as_str()).await?;
    mess.correct(format!("{:?}", size).as_str()).await?;

    // Create the endpoint.
    let secret_key = super::get_or_create_secret()?;
    let builder = Endpoint::builder()
        .alpns(vec![iroh_blobs::protocol::ALPN.to_vec()])
        .secret_key(secret_key)
        .relay_mode(RelayMode::Default)
        .add_discovery(DnsDiscovery::n0_dns());
    let endpoint = builder.bind().await?;
    mess.info("Local endpoint created...").await?;

    // Attach the services
    let blobs = BlobsProtocol::new(&store, endpoint.clone(), None);
    let router = iroh::protocol::Router::builder(endpoint)
        .accept(iroh_blobs::ALPN, blobs.clone())
        .spawn();

    // Create the ticket
    let addr = router.endpoint().node_addr().initialized().await;
    let ticket = BlobTicket::new(addr, tag.hash().to_owned(), BlobFormat::HashSeq);
    mess.send_ticket(ticket.to_string()).await?;

    Err(anyhow!("Send Fail"))
}

/// Import from a file or directory into the database.
///
/// The returned tag always refers to a collection. If the input is a file, this
/// is a collection with a single blob, named like the file.
///
/// If the input is a directory, the collection contains all the files in the
/// directory.
async fn import(
    path: PathBuf,
    store: &FsStore,
    mess: MessageOut,
) -> anyhow::Result<(TempTag, u64, Collection)> {
    let parallelism = num_cpus::get();
    let path = path.canonicalize()?;
    anyhow::ensure!(path.exists(), "path {} does not exist", path.display());
    let root = path.parent().context("context get parent")?;
    // walkdir also works for files, so we don't need to special case them
    let files = WalkDir::new(path.clone()).into_iter();
    // flatten the directory structure into a list of (name, path) pairs.
    // ignore symlinks.
    let data_sources: Vec<(String, PathBuf)> = files
        .map(|entry| {
            let entry = entry?;
            if !entry.file_type().is_file() {
                return Ok(None);
            }
            let path = entry.into_path();
            let relative = path.strip_prefix(root)?;
            let name = canonicalized_path_to_string(relative, true)?;
            anyhow::Ok(Some((name, path)))
        })
        .filter_map(Result::transpose)
        .collect::<anyhow::Result<Vec<_>>>()?;
    // import all the files, using num_cpus workers, return names and temp tags
    // let op = mp.add(make_import_overall_progress());
    // op.set_message(format!("importing {} files", data_sources.len()));
    // op.set_length(data_sources.len() as u64);
    let mut names_and_tags = n0_future::stream::iter(data_sources)
        .map(|(name, path)| {
            let db = store.clone();
            // This clones a mutex for each file , seems to work.
            let m = mess.clone();
            async move {
                let import = db.add_path_with_opts(AddPathOptions {
                    path,
                    mode: ImportMode::TryReference,
                    format: BlobFormat::Raw,
                });
                let mut stream = import.stream().await;
                let mut item_size = 0;
                let temp_tag = loop {
                    let item = stream
                        .next()
                        .await
                        .context("import stream ended without a tag")?;
                    match item {
                        AddProgressItem::Size(size) => {
                            item_size = size;
                            m.progress(name.as_str(), 0, item_size as usize).await?;
                        }
                        AddProgressItem::CopyProgress(offset) => {
                            m.progress(name.as_str(), offset as usize, item_size as usize)
                                .await?;
                        }
                        AddProgressItem::CopyDone => {
                            m.complete(name.as_str()).await?;
                        }
                        AddProgressItem::OutboardProgress(offset) => {
                            m.progress(name.as_str(), offset as usize, item_size as usize)
                                .await?;
                        }
                        AddProgressItem::Error(cause) => {
                            anyhow::bail!("error importing {}: {}", name, cause);
                        }
                        AddProgressItem::Done(tt) => {
                            m.progress_finish(name.as_str()).await?;
                            break tt;
                        }
                    }
                };
                anyhow::Ok((name, temp_tag, item_size))
            }
        })
        .buffered_unordered(parallelism)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<anyhow::Result<Vec<_>>>()?;
    // op.finish_and_clear();
    names_and_tags.sort_by(|(a, _, _), (b, _, _)| a.cmp(b));
    // total size of all files
    let size = names_and_tags.iter().map(|(_, _, size)| *size).sum::<u64>();
    // collect the (name, hash) tuples into a collection
    // we must also keep the tags around so the data does not get gced.
    let (collection, tags) = names_and_tags
        .into_iter()
        .map(|(name, tag, _)| ((name, *tag.hash()), tag))
        .unzip::<_, _, Collection, Vec<_>>();
    let temp_tag = collection.clone().store(store).await?;
    // now that the collection is stored, we can drop the tags
    // data is protected by the collection
    drop(tags);
    Ok((temp_tag, size, collection))
}

/// From original sendme.
/// This function converts an already canonicalized path to a string.
///
/// If `must_be_relative` is true, the function will fail if any component of the path is
/// `Component::RootDir`
///
/// This function will also fail if the path is non canonical, i.e. contains
/// `..` or `.`, or if the path components contain any windows or unix path
/// separators.
pub fn canonicalized_path_to_string(
    path: impl AsRef<Path>,
    must_be_relative: bool,
) -> anyhow::Result<String> {
    let mut path_str = String::new();
    let parts = path
        .as_ref()
        .components()
        .filter_map(|c| match c {
            Component::Normal(x) => {
                let c = match x.to_str() {
                    Some(c) => c,
                    None => return Some(Err(anyhow::anyhow!("invalid character in path"))),
                };

                if !c.contains('/') && !c.contains('\\') {
                    Some(Ok(c))
                } else {
                    Some(Err(anyhow::anyhow!("invalid path component {:?}", c)))
                }
            }
            Component::RootDir => {
                if must_be_relative {
                    Some(Err(anyhow::anyhow!("invalid path component {:?}", c)))
                } else {
                    path_str.push('/');
                    None
                }
            }
            _ => Some(Err(anyhow::anyhow!("invalid path component {:?}", c))),
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let parts = parts.join("/");
    path_str.push_str(&parts);
    Ok(path_str)
}
