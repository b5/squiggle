use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use bytes::Bytes;
use console::style;
use futures_buffered::BufferedStreamExt;
use futures_lite::{future::Boxed, StreamExt};
use indicatif::{
    HumanBytes, HumanDuration, MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle,
};
use iroh::blobs::Hash;
use iroh_base::{node_addr::AddrInfoOptions, ticket::BlobTicket};
use iroh_blobs::{
    format::collection::Collection,
    get::{
        db::DownloadProgress,
        fsm::{AtBlobHeaderNextError, DecodeError},
        request::get_hash_seq_and_sizes,
    },
    provider::{self, handle_connection, CustomEventSender, EventSender},
    store::{ExportMode, ImportMode, ImportProgress, ReadableStore, Store},
    util::local_pool::LocalPool,
    BlobFormat, Hash, HashAndFormat, TempTag,
};
use iroh_net::{
    key::SecretKey,
    relay::{RelayMap, RelayMode, RelayUrl},
    Endpoint,
};
use serde::{Deserialize, Serialize};
use tokio::io::BufReader;
use walkdir::WalkDir;

use super::Repo;
use crate::repo::schemas::PROGRAMS_SCHAMA_NAME;
use crate::router::RouterClient;

const MANIFEST_FILENAME: &str = "package.json";
const TASK_FILENAME: &str = "program.wasm";
const UI_FILENAME: &str = "index.html";

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    name: String,
    version: String,
    description: Option<String>,
    homepage: Option<String>,
    repository: Option<String>,
    license: Option<String>,
    main: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Program {
    pub name: String,
    pub hash: Hash,
    pub data: Option<serde_json::Value>,
}

impl Program {
    pub async fn load(router: &RouterClient, hash: Hash) -> Result<Self> {
        let bytes = router.blobs().read_to_bytes(hash).await?;
        let meta: Manifest = serde_json::from_slice(&bytes)?;
        let data = serde_json::from_slice(&bytes)?;

        Ok(Program {
            name: meta.name,
            hash,
            data: Some(data),
        })
    }

    pub fn validator(&self) -> Result<jsonschema::Validator> {
        match &self.data {
            Some(data) => jsonschema::validator_for(data).context("failed to create validator"),
            None => Err(anyhow!("no validator found")),
        }
    }

    pub fn id(&self) -> Result<Hash> {
        let res = serde_json::to_string(self).map(|data| Hash::from_str(data.as_str()))??;
        Ok(res)
    }

    // async fn write_event(&self, author: Author, db: &DB) -> Result<()> {
    //     let created_at = chrono::Utc::now().timestamp();
    //     let content = self.name.clone().into();
    //     let event = Event {
    //         id: nostr_id(
    //             PublicKey::from_bytes(author.public_key().as_bytes())?,
    //             self.created_at,
    //             EventKind::MutateProgram,
    //             &vec![],
    //             &content,
    //         )?,
    //         pubkey: self.pubkey.clone(),
    //         created_at: self.created_at,
    //         kind: EventKind::MutateProgram,
    //         tags: vec![Tag(NOSTR_Program_TAG.to_string(), Program.to_string(), None)],
    //         sig: "".to_string(),
    //         content: HashOrContent::Content(content),
    //     };
    //     event.write(db).await
    // }
}

#[derive(Clone)]
pub struct Programs(Repo);

impl Programs {
    pub fn new(repo: Repo) -> Self {
        Programs(repo)
    }

    // pub async fn create(&self, data: Bytes) -> Result<Program> {
    //     // extract the title from the Program
    //     let meta: ProgramMetadata = serde_json::from_slice(&data)?;

    //     // confirm our data is a valid JSON Program
    //     let Program = serde_json::from_slice(&data)?;
    //     jsonschema::validator_for(&Program)?;

    //     // serialize data & add locally
    //     // TODO - test that this enforces field ordering
    //     let serialized = serde_json::to_vec(&Program)?;

    //     // TODO - should construct a HashSeq, place the new Program as the 1th element
    //     // and update the metadata in 0th element
    //     let res = self.0.router.blobs().add_bytes(serialized).await?;

    //     Ok(Program {
    //         title: meta.title,
    //         hash: res.hash,
    //         data: Some(Program),
    //     })
    // }

    pub async fn bundle(&self, author: Author, path: impl Into<PathBuf>) -> Result<Program> {
        let path: PathBuf = path.into().canonicalize()?;
        anyhow::ensure!(path.exists(), "path {} does not exist", path.display());
        anyhow::ensure!(path.is_dir(), "path {} is not a directory", path.display());

        let manifest_path = path.join(MANIFEST_FILENAME);
        let file = tokio::fs::File::open(manifest_path)
            .await
            .context("opening manifest file")?;
        let mut manifest_reader = BufReader::new(file);
        let manifest: Manifest =
            serde_json::from_reader(manifest_reader).context("invalid manifest")?;

        let result = import(self.0.router().blobs(), path).await?;

        let programs_schema = self.0.schemas().get_by_name(PROGRAMS_SCHAMA_NAME).await?;
        self.0.events().create(author, schema, data)

        let program = Program {
            name: manifest.name,
            hash: result.0.hash,
            data: Some(serde_json::to_value(manifest)?),
        };
        Ok(program)
    }

    pub async fn load(&self, hash: Hash) -> Result<Program> {
        Program::load(self.0.router(), hash).await
    }

    // pub async fn mutate(&self, _id: Hash, data: &str) -> Result<Hash> {
    //     let Program = Program::new(data.to_string());
    //     // TODO - should construct a HashSeq, place the new Program as the 1th element
    //     // and update the metadata in 0th element
    //     // Program.write(&self.db).await
    //     Program.id()
    // }

    pub async fn get_by_name(&self, name: &str) -> Result<Program> {
        // TODO - SLOW
        self.list(0, -1)
            .await?
            .into_iter()
            .find(|program| program.name == name)
            .ok_or_else(|| anyhow!("Program not found"))
    }

    pub async fn get_by_hash(&self, hash: Hash) -> Result<Program> {
        // TODO - SLOW
        self.list(0, -1)
            .await?
            .into_iter()
            .find(|program| program.hash.eq(&hash))
            .ok_or_else(|| anyhow!("Program not found"))
    }

    pub async fn list(&self, offset: i64, limit: i64) -> Result<Vec<Program>> {
        let conn = self.0.db.lock().await;
        let mut stmt = conn
            .prepare("SELECT DISTINCT Program FROM events LIMIT ?1 OFFSET ?2")
            .context("selecting Programs from events table")?;
        let mut rows = stmt.query([limit, offset])?;

        let mut programs = Vec::new();
        while let Some(row) = rows.next()? {
            let hash: String = row.get(0)?;
            let hash = Hash::from_str(&hash)?;
            match Program::load(self.0.router(), hash).await {
                Ok(program) => {
                    programs.push(Program);
                }
                Err(e) => {
                    // TODO - what to do when we can't load Program data?
                    tracing::error!(
                        "failed to load Program for hash: {:?} {:?}",
                        hash.to_string(),
                        e
                    );
                }
            }
        }
        Ok(Programs)
    }
}

fn validate_path_component(component: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !component.contains('/'),
        "path components must not contain the only correct path separator, /"
    );
    Ok(())
}

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

pub async fn show_ingest_progress(
    recv: async_channel::Receiver<ImportProgress>,
) -> anyhow::Result<()> {
    let mp = MultiProgress::new();
    mp.set_draw_target(ProgressDrawTarget::stderr());
    let op = mp.add(ProgressBar::hidden());
    op.set_style(
        ProgressStyle::default_spinner().template("{spinner:.green} [{elapsed_precise}] {msg}")?,
    );
    // op.set_message(format!("{} Ingesting ...\n", style("[1/2]").bold().dim()));
    // op.set_length(total_files);
    let mut names = BTreeMap::new();
    let mut sizes = BTreeMap::new();
    let mut pbs = BTreeMap::new();
    loop {
        let event = recv.recv().await;
        match event {
            Ok(ImportProgress::Found { id, name }) => {
                names.insert(id, name);
            }
            Ok(ImportProgress::Size { id, size }) => {
                sizes.insert(id, size);
                let total_size = sizes.values().sum::<u64>();
                op.set_message(format!(
                    "{} Ingesting {} files, {}\n",
                    style("[1/2]").bold().dim(),
                    sizes.len(),
                    HumanBytes(total_size)
                ));
                let name = names.get(&id).cloned().unwrap_or_default();
                let pb = mp.add(ProgressBar::hidden());
                pb.set_style(ProgressStyle::with_template(
                  "{msg}{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes}",
              )?.progress_chars("#>-"));
                pb.set_message(format!("{} {}", style("[2/2]").bold().dim(), name));
                pb.set_length(size);
                pbs.insert(id, pb);
            }
            Ok(ImportProgress::OutboardProgress { id, offset }) => {
                if let Some(pb) = pbs.get(&id) {
                    pb.set_position(offset);
                }
            }
            Ok(ImportProgress::OutboardDone { id, .. }) => {
                // you are not guaranteed to get any OutboardProgress
                if let Some(pb) = pbs.remove(&id) {
                    pb.finish_and_clear();
                }
            }
            Ok(ImportProgress::CopyProgress { .. }) => {
                // we are not copying anything
            }
            Err(e) => {
                op.set_message(format!("Error receiving progress: {e}"));
                break;
            }
        }
    }
    op.finish_and_clear();
    Ok(())
}

/// Import from a file or directory into the database.
///
/// The returned tag always refers to a collection. If the input is a file, this
/// is a collection with a single blob, named like the file.
///
/// If the input is a directory, the collection contains all the files in the
/// directory.
async fn import(
    db: &iroh::client::blobs::Client,
    path: PathBuf,
) -> anyhow::Result<(TempTag, u64, Collection)> {
    let root = path.clone();
    // walkdir also works for files, so we don't need to special case them
    let files = WalkDir::new(path.clone()).into_iter();
    // flatten the directory structure into a list of (name, path) pairs.
    // ignore symlinks.
    let data_sources: Vec<(String, PathBuf)> = files
        .map(|entry| {
            let entry = entry?;
            if !entry.file_type().is_file() {
                // Skip symlinks. Directories are handled by WalkDir.
                return Ok(None);
            }
            let path = entry.into_path();
            let relative = path.strip_prefix(&root)?;
            let name = canonicalized_path_to_string(relative, true)?;
            anyhow::Ok(Some((name, path)))
        })
        .filter_map(Result::transpose)
        .collect::<anyhow::Result<Vec<_>>>()?;
    let (send, recv) = async_channel::bounded(32);
    let progress = iroh_blobs::util::progress::AsyncChannelProgressSender::new(send);
    let show_progress = tokio::spawn(show_ingest_progress(recv));
    // import all the files, using num_cpus workers, return names and temp tags
    let mut names_and_tags = futures_lite::stream::iter(data_sources)
        .map(|(name, path)| {
            let db = db.clone();
            let progress = progress.clone();
            async move {
                let (temp_tag, file_size) = db
                    .import_file(path, ImportMode::Copy, BlobFormat::Raw, progress)
                    .await?;
                anyhow::Ok((name, temp_tag, file_size))
            }
        })
        .buffered_unordered(num_cpus::get())
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<anyhow::Result<Vec<_>>>()?;
    drop(progress);
    names_and_tags.sort_by(|(a, _, _), (b, _, _)| a.cmp(b));
    // total size of all files
    let size = names_and_tags.iter().map(|(_, _, size)| *size).sum::<u64>();
    // collect the (name, hash) tuples into a collection
    // we must also keep the tags around so the data does not get gced.
    let (collection, tags) = names_and_tags
        .into_iter()
        .map(|(name, tag, _)| ((name, *tag.hash()), tag))
        .unzip::<_, _, Collection, Vec<_>>();
    let temp_tag = collection.clone().store(&db).await?;
    // now that the collection is stored, we can drop the tags
    // data is protected by the collection
    drop(tags);
    show_progress.await??;
    Ok((temp_tag, size, collection))
}
