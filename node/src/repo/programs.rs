use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use futures_lite::StreamExt;
use indicatif::{HumanBytes, MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use iroh::blobs::Hash;
use iroh::docs::Author;
use iroh::net::key::PublicKey;
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
use uuid::Uuid;
use walkdir::WalkDir;

use super::events::{Event, EventKind, EventObject, HashOrContent, Tag, NOSTR_ID_TAG};
use super::Repo;
use crate::router::RouterClient;

const MANIFEST_FILENAME: &str = "package.json";
const DEFAULT_PROGRAM_ENTRY_FILENAME: &str = "index.wasm";
const HTML_INDEX_FILENAME: &str = "index.html";

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
    pub id: Uuid,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    pub author: PublicKey,
    pub content: HashOrContent,
    pub manifest: Manifest,
    pub html_index: Option<Hash>,
    pub program_entry: Option<Hash>,
}

impl EventObject for Program {
    async fn from_event(event: Event, client: &RouterClient) -> Result<Self> {
        if event.kind != EventKind::MutateProgram {
            anyhow::bail!("event is not a program mutation");
        }

        let id = event.data_id()?.ok_or_else(|| anyhow!("missing data id"))?;

        // fetch collection content
        let content_hash = match event.content {
            HashOrContent::Hash(hash) => hash,
            HashOrContent::Content(_) => anyhow::bail!("content must be a hash"),
        };
        let collection = client.blobs().get_collection(content_hash).await?;

        // extract the manifest
        let (_, manifest_hash) = collection
            .iter()
            .find(|item| item.0 == MANIFEST_FILENAME)
            .ok_or_else(|| anyhow!("missing manifest"))?;
        let data = client.blobs().read_to_bytes(*manifest_hash).await?;
        let manifest: Manifest = serde_json::from_slice(&data)?;
        let (html_index, program_entry) = Program::hash_pointers(&manifest, &collection)?;

        Ok(Program {
            id,
            created_at: event.created_at,
            author: event.pubkey,
            content: event.content,
            manifest,
            html_index,
            program_entry,
        })
    }

    fn into_mutate_event(&self, author: Author) -> Result<Event> {
        // assert!(author.public_key() == self.author);
        let tags = vec![Tag::new(NOSTR_ID_TAG, self.id.to_string().as_str())];
        let content = match self.content {
            HashOrContent::Hash(hash) => hash,
            HashOrContent::Content(_) => anyhow::bail!("content must be a hash"),
        };
        Event::create(
            author,
            self.created_at,
            EventKind::MutateProgram,
            tags,
            content,
        )
    }
}

impl Program {
    fn hash_pointers(
        manifest: &Manifest,
        collection: &Collection,
    ) -> Result<(Option<Hash>, Option<Hash>)> {
        let entry_filename = manifest
            .main
            .clone()
            .unwrap_or(String::from(DEFAULT_PROGRAM_ENTRY_FILENAME));

        let mut html_index = None;
        let mut program_entry = None;
        for (name, hash) in collection.iter() {
            if name == HTML_INDEX_FILENAME {
                html_index = Some(*hash);
            } else if name == entry_filename {
                program_entry = Some(*hash);
            }
        }
        Ok((html_index, program_entry))
    }

    async fn from_sql_row(row: &rusqlite::Row<'_>, client: &RouterClient) -> Result<Program> {
        let event = Event::from_sql_row(row)?;
        Self::from_event(event, client).await
    }
}

#[derive(Clone)]
pub struct Programs(Repo);

impl Programs {
    pub fn new(repo: Repo) -> Self {
        Programs(repo)
    }

    pub async fn mutate(
        &self,
        author: Author,
        id: Uuid,
        path: impl Into<PathBuf>,
    ) -> Result<Program> {
        // assert this is a valid program directory
        let path: PathBuf = path.into().canonicalize()?;
        anyhow::ensure!(path.is_dir(), "path {} is not a directory", path.display());
        let manifest_path = path.join(MANIFEST_FILENAME);
        anyhow::ensure!(
            manifest_path.exists(),
            "path {} does not exist",
            path.display()
        );

        // load manifest
        let data: Vec<u8> = tokio::fs::read(&manifest_path).await?;
        let manifest: Manifest = serde_json::from_slice(data.as_slice())?;

        // create collection
        let result = import(self.0.router().blobs(), path).await?;
        let (temp_tag, size, collection) = import(self.0.router.blobs(), path).await?;
        let hash = *temp_tag.hash();

        // build program
        let (html_index, program_entry) = Program::hash_pointers(&manifest, &collection)?;
        let program = Program {
            id,
            // TODO(b5) - wat. why? you're doing something wrong with types.
            author: PublicKey::from_bytes(author.public_key().as_bytes())?,
            created_at: chrono::Utc::now().timestamp(),
            manifest,
            content: HashOrContent::Hash(hash),
            html_index,
            program_entry,
        };

        // write event
        let event = program.into_mutate_event(author)?;
        event.write(&self.0.db).await?;

        Ok(program)
    }

    pub async fn get_by_hash(&self, _hash: Hash) -> Result<Program> {
        todo!("get_by_hash");
        // // TODO - SLOW
        // self.list(0, -1)
        //     .await?
        //     .into_iter()
        //     .find(|program| program.content.eq(&hash))
        //     .ok_or_else(|| anyhow!("Program not found"))
    }

    pub async fn list(&self, offset: i64, limit: i64) -> Result<Vec<Program>> {
        let conn = self.0.db.lock().await;
        let mut stmt = conn
            .prepare("SELECT DISTINCT Program FROM events LIMIT ?1 OFFSET ?2")
            .context("selecting Programs from events table")?;
        let mut rows = stmt.query([limit, offset])?;

        let mut programs = Vec::new();
        while let Some(row) = rows.next()? {
            let program = Program::from_sql_row(row, self.0.router()).await?;
            programs.push(program);
        }
        Ok(programs)
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
    let progress = iroh::blobs::util::progress::AsyncChannelProgressSender::new(send);
    let show_progress = tokio::spawn(show_ingest_progress(recv));
    // import all the files, using num_cpus workers, return names and temp tags
    let mut names_and_tags = futures_lite::stream::iter(data_sources)
        .map(|(name, path)| {
            let db = db.clone();
            let progress = progress.clone();
            async move {
                let (temp_tag, file_size) = db
                    .add_from_path(path, ImportMode::Copy, BlobFormat::Raw, progress)
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
