use anyhow::{anyhow, Result};
use iroh::blobs::Hash;
use iroh::docs::Author;
use iroh::net::key::PublicKey;
use rusqlite::params;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

use super::db::DB;
use super::Repo;

const NOSTR_EVENT_VERSION_NUMBER: u32 = 0;
const NOSTR_SCHEMA_TAG: &str = "sch";
const NOSTR_ID_TAG: &str = "id";

pub enum EventKind {
    MutateAuthor,
    DeleteAuthor,
    MutateCapability,
    DeleteCapability,
    MutateProgram,
    DeleteProgram,
    MutateSchema,
    DeleteSchema,
    MutateRow,
    DeleteRow,
}

impl EventKind {
    /// the nostr event kind
    // TODO(b5): random number placeholders for now
    pub fn kind(&self) -> u32 {
        match self {
            EventKind::MutateAuthor => 100000,
            EventKind::DeleteAuthor => 100001,
            EventKind::MutateCapability => 100002,
            EventKind::DeleteCapability => 100003,
            EventKind::MutateProgram => 100004,
            EventKind::DeleteProgram => 100005,
            EventKind::MutateSchema => 100006,
            EventKind::DeleteSchema => 100007,
            EventKind::MutateRow => 100008,
            EventKind::DeleteRow => 100009,
        }
    }
}

/// A struct that wraps Sha256 digest and can be serialized/deserialized
/// from a 32-byte lowercase-encoded hex string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sha256Digest([u8; 32]);

impl Sha256Digest {
    /// Create a new Sha256Digest from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Sha256Digest(bytes)
    }

    /// Returns the underlying byte array
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Create a Sha256Digest from hashing some input data
    pub fn from_data(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        Sha256Digest(result.into())
    }
}

/// Helper function to convert a byte array to a lowercase hex string
fn to_hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{:02x}", byte)).collect()
}

/// Helper function to parse a hex string into a byte array
fn from_hex_string(hex: &str) -> Result<[u8; 32], String> {
    let bytes = hex::decode(hex).map_err(|e| e.to_string())?;
    if bytes.len() != 32 {
        return Err(format!("Expected 32 bytes, got {}", bytes.len()));
    }
    let mut array = [0u8; 32];
    array.copy_from_slice(&bytes);
    Ok(array)
}

impl Serialize for Sha256Digest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hex_string = to_hex_string(&self.0);
        serializer.serialize_str(&hex_string)
    }
}

impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex_string = String::deserialize(deserializer)?;
        let bytes = from_hex_string(&hex_string).map_err(serde::de::Error::custom)?;
        Ok(Sha256Digest(bytes))
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", to_hex_string(&self.0))
    }
}

impl FromStr for Sha256Digest {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = from_hex_string(s)?;
        Ok(Sha256Digest(bytes))
    }
}

pub(crate) fn nostr_id(
    pubkey: PublicKey,
    created_at: i64,
    kind: u32,
    tags: &Vec<Tag>,
    content: &Hash,
) -> Result<Sha256Digest> {
    let data = serde_json::to_string(&(
        NOSTR_EVENT_VERSION_NUMBER,
        pubkey,
        created_at,
        kind,
        tags,
        content,
    ))?;
    Ok(Sha256Digest::from_data(data.as_bytes()))
}

#[derive(Debug, Serialize, Deserialize)]
pub enum HashOrContent {
    Hash(Hash),
    Content(Value),
}

impl From<Hash> for HashOrContent {
    fn from(hash: Hash) -> Self {
        HashOrContent::Hash(hash)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tag(String, String, Option<String>);

#[derive(Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: Sha256Digest,
    pub pubkey: String,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    pub kind: u32,
    pub tags: Vec<Tag>,
    pub sig: String,
    pub content: HashOrContent,
}

impl Event {
    async fn mutate(
        db: &DB,
        author: Author,
        schema: Hash,
        id: Uuid,
        content: Hash,
    ) -> Result<Event> {
        let created_at = chrono::Utc::now().timestamp();
        let tags = vec![
            Tag(NOSTR_SCHEMA_TAG.to_string(), schema.to_string(), None),
            Tag(NOSTR_ID_TAG.to_string(), id.to_string(), None),
        ];
        let id = nostr_id(
            // TODO - remove this fuckery
            PublicKey::from_bytes(author.public_key().as_bytes())?,
            created_at,
            EventKind::MutateRow.kind(),
            &tags,
            &content,
        )?;

        let sig = author.sign(id.as_bytes());
        let event = Event {
            id,
            pubkey: author.id().to_string(),
            created_at,
            kind: EventKind::MutateRow.kind(),
            tags,
            sig: hex::encode(sig.to_bytes()),
            content: content.into(),
        };

        event.write(db).await?;
        Ok(event)
    }

    fn schema_hash(&self) -> Result<Hash> {
        let schema_tag = self
            .tags
            .iter()
            .find(|tag| tag.0 == NOSTR_SCHEMA_TAG)
            .ok_or_else(|| anyhow::anyhow!("No schema tag found"))?;
        let hash = Hash::from_str(&schema_tag.1)?;
        Ok(hash)
    }

    fn data_id(&self) -> Result<Uuid> {
        let id_tag = self
            .tags
            .iter()
            .find(|tag| tag.0 == NOSTR_ID_TAG)
            .ok_or_else(|| anyhow::anyhow!("No id tag found"))?;
        Uuid::parse_str(&id_tag.1).map_err(|e| anyhow::anyhow!(e))
    }

    pub(crate) async fn write(&self, db: &DB) -> Result<()> {
        let conn = db.lock().await;
        let schema = self.schema_hash()?.to_string();
        let data_id = self.data_id()?;
        let content = match self.content {
            HashOrContent::Hash(ref hash) => hash.to_string(),
            HashOrContent::Content(ref data) => {
                let data = serde_json::to_vec(data)?;
                String::from_utf8(data)?
            },
        };
        conn.execute(
            "INSERT INTO events (id, pubkey, created_at, kind, schema, data_id, content, sig) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                self.id.to_string(),
                self.pubkey,
                self.created_at,
                self.kind,
                schema,
                data_id, 
                content,
                self.sig
            ],
        )?;
        Ok(())
    }

    fn from_sql_row(row: &rusqlite::Row) -> Result<Self> {
        // (0   1       2           3     4       5        6       7)
        // (id, pubkey, created_at, kind, schema, data_id, content, sig)
        let id: String = row.get(0)?;
        let content: String = row.get(6)?;
        let data_id: Uuid = row.get(5)?;
        Ok(Self {
            id: Sha256Digest::from_str(&id).map_err(|e| anyhow!(e))?,
            pubkey: row.get(1)?,
            created_at: row.get(2)?,
            kind: row.get(3)?,
            tags: vec![
                Tag(NOSTR_SCHEMA_TAG.to_string(), row.get(4)?, None),
                Tag(NOSTR_ID_TAG.to_string(), data_id.to_string(), None),
            ],
            content: Hash::from_str(&content)?.into(),
            sig: row.get(6)?,
        })
    }
}

#[derive(Clone)]
pub struct Events(Repo);

impl Events {
    pub fn new(repo: Repo) -> Self {
        Events(repo)
    }

    pub async fn create(
        &self,
        author: Author,
        schema: Hash,
        data: impl Serialize,
    ) -> Result<Event> {
        let data_id = Uuid::new_v4();
        self.mutate(author, schema, data_id, data).await
    }

    pub async fn mutate(
        &self,
        author: Author,
        schema: Hash,
        id: Uuid,
        data: impl Serialize,
    ) -> Result<Event> {
        // TODO - validate data against schema
        let data = serde_json::to_vec(&data)?;
        let outcome = self.0.router.blobs().add_bytes(data).await?;
        let content = outcome.hash;
        Event::mutate(&self.0.db, author, schema, id, content).await
        // TODO - write to data table
    }

    pub async fn query(
        &self,
        schema: Hash,
        _query: String,
        offset: i64,
        limit: i64
    ) -> Result<Vec<Event>> {
        let conn = self.0.db.lock().await;
        let mut stmt = conn.prepare("SELECT id, pubkey, created_at, kind, schema, data_id, content, sig FROM events WHERE schema = ?1 LIMIT ?2 OFFSET ?3")?;
        let mut rows = stmt.query(params![schema.to_string(), limit, offset])?;
        let mut events = Vec::new();
        

        while let Some(row) = rows.next()? {
            let mut event = Event::from_sql_row(row)?;
            let content_hash = match event.content {
                HashOrContent::Hash(hash) => hash,
                HashOrContent::Content(_) => panic!("must be a hash")
            };
            let content = self.0.router.blobs().read_to_bytes(content_hash).await?;
            let content = serde_json::from_slice::<Value>(&content).map_err(|e| anyhow!(e))?;
            event.content = HashOrContent::Content(content);
            events.push(event);
        }
        Ok(events)
    }
}