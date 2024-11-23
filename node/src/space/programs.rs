use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use futures_buffered::BufferedStreamExt;
use futures_lite::StreamExt;
use iroh::blobs::format::collection::Collection;
use iroh::blobs::util::SetTagOption;
use iroh::blobs::Hash;
use iroh::client::blobs::WrapOption;
use iroh::docs::Author;
use iroh::net::key::PublicKey;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::events::{
    Event, EventKind, EventObject, HashLink, Tag, EVENT_SQL_READ_FIELDS, NOSTR_ID_TAG,
};
use super::tickets::ProgramTicket;
use super::Space;
use crate::router::RouterClient;

const MANIFEST_FILENAME: &str = "program.json";
const DEFAULT_PROGRAM_ENTRY_FILENAME: &str = "index.wasm";
const HTML_INDEX_FILENAME: &str = "index.html";

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub license: Option<String>,
    pub main: Option<String>,
    pub config: Option<ProgramConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProgramConfig {
    environment: Option<Vec<ProgramEnvVar>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProgramEnvVar {
    pub key: String,
    pub description: String,
    pub required: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Program {
    pub id: Uuid,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    pub author: PublicKey,
    pub content: HashLink,
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
        let collection = client.blobs().get_collection(event.content.hash).await?;

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
        Event::create(
            author,
            self.created_at,
            EventKind::MutateProgram,
            tags,
            self.content.clone(),
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
            } else if *name == entry_filename {
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
pub struct Programs(Space);

impl Programs {
    pub fn new(repo: Space) -> Self {
        Programs(repo)
    }

    pub async fn create(&self, author: Author, path: impl Into<PathBuf>) -> Result<Program> {
        let id = Uuid::new_v4();
        self.mutate(author, id, path).await
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
        let (hash, size, collection) = import(self.0.router.blobs(), path).await?;

        // build program
        let (html_index, program_entry) = Program::hash_pointers(&manifest, &collection)?;
        let program = Program {
            id,
            // TODO(b5) - wat. why? you're doing something wrong with types.
            author: PublicKey::from_bytes(author.public_key().as_bytes())?,
            created_at: chrono::Utc::now().timestamp(),
            manifest,
            content: HashLink {
                hash,
                size: Some(size),
                data: None,
            },
            html_index,
            program_entry,
        };

        // write event
        let event = program.into_mutate_event(author)?;
        event.write(&self.0.db).await?;

        Ok(program)
    }

    pub async fn share(&self, router: &RouterClient, id: Uuid) -> Result<ProgramTicket> {
        // get the raw event, write it to the store
        let program_event = Event::read_raw(&self.0.db, id).await?;
        let (program_event_hash, program_event_tag) =
            program_event.write_raw_to_blob(router).await?;

        // TODO - get profile information for the user, add to collection

        let head = vec![(String::from("event"), program_event_hash)];

        // get collection contents
        let program_collection = router
            .blobs()
            .get_collection(program_event.content.hash)
            .await?;

        // create our collection contents
        let collection =
            Collection::from_iter(head.into_iter().chain(program_collection.into_iter()));

        let (hash, _) = router
            .blobs()
            .create_collection(collection, SetTagOption::Auto, vec![program_event_tag])
            .await?;

        // get our dialing information
        let mut addr = router.net().node_addr().await?;
        addr.apply_options(iroh::base::node_addr::AddrInfoOptions::Id);

        // create ticket
        ProgramTicket::new(addr, hash, iroh::blobs::BlobFormat::HashSeq)
    }

    pub async fn download(&self, router: &RouterClient, ticket: ProgramTicket) -> Result<Program> {
        let addr = ticket.node_addr().clone();
        // fetch the blob
        router
            .blobs()
            .download(ticket.hash(), addr)
            .await?
            .finish()
            .await?;

        let mut collection = router
            .blobs()
            .get_collection(ticket.hash())
            .await?
            .into_iter();

        // ingest the program event
        let (_, hash) = collection
            .next()
            .ok_or_else(|| anyhow!("empty collection"))?;
        let event = Event::ingest_from_blob(&self.0.db, router, hash).await?;

        // consume the rest of the collection, adding as a new collection to re-surface the progra
        // pacakge root hash in our local repo
        let collection = Collection::from_iter(collection);
        router
            .blobs()
            .create_collection(collection, SetTagOption::Auto, vec![])
            .await?;

        Program::from_event(event, router).await
    }

    pub async fn get_by_name(&self, name: String) -> Result<Program> {
        // TODO (b5) - I know. this is terrible
        self.list(0, -1)
            .await?
            .into_iter()
            .find(|program| program.manifest.name == name)
            .ok_or_else(|| anyhow!("Program not found"))
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Program> {
        let conn = self.0.db.lock().await;
        let mut stmt = conn
            .prepare(
                format!(
                    "SELECT {EVENT_SQL_READ_FIELDS} FROM events WHERE kind = ?1 AND data_id = ?2"
                )
                .as_str(),
            )
            .context("selecting Program by id from events table")?;
        let mut rows = stmt.query(params![EventKind::MutateProgram, id])?;

        if let Some(row) = rows.next()? {
            Program::from_sql_row(row, &self.0.router).await
        } else {
            Err(anyhow!("Program not found"))
        }
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
            .prepare(
                format!(
                    "SELECT {EVENT_SQL_READ_FIELDS} FROM events WHERE kind = ?1 LIMIT ?2 OFFSET ?3"
                )
                .as_str(),
            )
            .context("selecting Programs from events table")?;
        let mut rows = stmt.query(params![EventKind::MutateProgram, limit, offset])?;

        let mut programs = Vec::new();
        while let Some(row) = rows.next()? {
            let program = Program::from_sql_row(row, &self.0.router).await?;
            programs.push(program);
        }
        Ok(programs)
    }
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

// based on https://docs.npmjs.com/cli/v10/configuring-npm/package-json#files
// exanded for rust things
// const IGNORE_PATTERNS: &[&str] = &[
//     "*.orig",
//     ".*.swp",
//     ".DS_Store",
//     "._*",
//     ".git",
//     ".hg",
//     ".lock-wscript",
//     ".npmrc",
//     ".svn",
//     ".wafpickle-N",
//     "CVS",
//     "config.gypi",
//     "node_modules",
//     "target",
//     "npm-debug.log",
//     "package-lock.json",
//     "pnpm-lock.yaml",
//     "yarn.lock",
// ];

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
) -> anyhow::Result<(Hash, u64, Collection)> {
    let root = path.clone();
    // walkdir also works for files, so we don't need to special case them
    let files = ignore::WalkBuilder::new(path.clone())
        .standard_filters(true)
        .follow_links(false)
        .build();
    // TODO(b5): finish
    // for pattern in IGNORE_PATTERNS {
    //     builder = builder.add_custom_ignore_filename(pattern);
    // }

    // flatten the directory structure into a list of (name, path) pairs.
    // ignore symlinks.
    let data_sources: Vec<(String, PathBuf)> = files
        .map(|entry| {
            let entry = entry?;
            let path = entry.into_path();
            let relative = path.strip_prefix(&root)?;
            let name = canonicalized_path_to_string(relative, true)?;
            anyhow::Ok(Some((name, path)))
        })
        .filter_map(Result::transpose)
        .collect::<anyhow::Result<Vec<_>>>()?;

    // import all the files, using num_cpus workers, return names and temp tags
    let mut names_and_tags = futures_lite::stream::iter(data_sources)
        .map(|(name, path)| {
            let db = db.clone();
            async move {
                let result = db
                    .add_from_path(path, false, SetTagOption::Auto, WrapOption::NoWrap)
                    .await?
                    .finish()
                    .await?;
                anyhow::Ok((name, result))
            }
        })
        .buffered_unordered(num_cpus::get())
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<anyhow::Result<Vec<_>>>()?;

    names_and_tags.sort_by(|(a, _), (b, _)| a.cmp(b));
    // total size of all files
    let size = names_and_tags
        .iter()
        .map(|(_, result)| result.size)
        .sum::<u64>();
    // collect the (name, hash) tuples into a collection
    // we must also keep the tags around so the data does not get gced.
    let (collection, tags_to_delete) = names_and_tags
        .into_iter()
        .map(|(name, result)| ((name, result.hash), result.tag))
        .unzip::<_, _, Collection, Vec<_>>();
    let (hash, _tag) = db
        .create_collection(collection.clone(), SetTagOption::Auto, tags_to_delete)
        .await?;

    Ok((hash, size, collection))
}
