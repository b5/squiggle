use anyhow::{anyhow, Result};
use ed25519_dalek::Signature;
use iroh::blobs::Hash;
use iroh::docs::Author;
use iroh::net::key::PublicKey;
use rusqlite::types::{FromSql, ToSqlOutput};
use rusqlite::{params, ToSql};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

use crate::router::RouterClient;

use super::db::DB;

const NOSTR_EVENT_VERSION_NUMBER: u32 = 0;
pub(crate) const NOSTR_SCHEMA_TAG: &str = "sch";
pub(crate) const NOSTR_ID_TAG: &str = "id";

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum EventKind {
    MutateAuthor,
    DeleteAuthor,
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
            EventKind::MutateProgram => 100002,
            EventKind::DeleteProgram => 100003,
            EventKind::MutateSchema => 100004,
            EventKind::DeleteSchema => 100005,
            EventKind::MutateRow => 100006,
            EventKind::DeleteRow => 100007,
        }
    }
}

impl ToSql for EventKind {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.kind()))
    }
}

impl FromSql for EventKind {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let kind = u32::column_result(value)?;
        match kind {
            100000 => Ok(EventKind::MutateAuthor),
            100001 => Ok(EventKind::DeleteAuthor),
            100002 => Ok(EventKind::MutateProgram),
            100003 => Ok(EventKind::DeleteProgram),
            100004 => Ok(EventKind::MutateSchema),
            100005 => Ok(EventKind::DeleteSchema),
            100006 => Ok(EventKind::MutateRow),
            100007 => Ok(EventKind::DeleteRow),
            _ => Err(rusqlite::types::FromSqlError::OutOfRange(kind)),
        }
    }
}

impl Serialize for EventKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.kind().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for EventKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let kind = u32::deserialize(deserializer)?;
        match kind {
            100000 => Ok(EventKind::MutateAuthor),
            100001 => Ok(EventKind::DeleteAuthor),
            100002 => Ok(EventKind::MutateProgram),
            100003 => Ok(EventKind::DeleteProgram),
            100004 => Ok(EventKind::MutateSchema),
            100005 => Ok(EventKind::DeleteSchema),
            100006 => Ok(EventKind::MutateRow),
            100007 => Ok(EventKind::DeleteRow),
            _ => Err(serde::de::Error::custom(format!("Unknown event kind: {}", kind))),
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
pub(crate) struct Tag(String, String, Option<String>);

impl Tag {
    pub fn new(name: &str, value: &str) -> Self {
        Tag(name.to_string(), value.to_string(), None)
    }
    pub fn new_with_hint(name: &str, value: &str, hint: &str) -> Self {
        Tag(name.to_string(), value.to_string(), Some(hint.to_string()))
    }
}

// {
// "id": "4376c65d2f232afbe9b882a35baa4f6fe8667c4e684749af565f981833ed6a65",
// "pubkey": "6e468422dfb74a5738702a8823b9b28168abab8655faacb6853cd0ee15deee93",
// "created_at": 1673347337,
// "kind": 1,
// "content": "Walled gardens became prisons, and nostr is the first step towards tearing down the prison walls.",
// "tags": [
//     ["e", "3da979448d9ba263864c4d6f14984c423a3838364ec255f03c7904b1ae77f206"],
//     ["p", "bf2376e17ba4ec269d10fcc996a4746b451152be9031fa48e74553dde5526bce"]
// ],
// "sig": "908a15e46fb4d8675bab026fc230a0e3542bfade63da02d542fb78b2a8513fcd0092619a2c8c1221e581946e0191f2af505dfdf8657a414dbca329186f009262"
// }

#[derive(Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: Sha256Digest,
    pub pubkey: PublicKey,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    pub kind: EventKind,
    pub tags: Vec<Tag>,
    pub sig: Signature,
    pub content: HashOrContent,
}

impl Event {
    pub(crate) fn create(author: Author, created_at: i64, kind: EventKind, tags: Vec<Tag>, content: Hash) -> Result<Event> {
        // TODO(b5) - wat. why? you're doing something wrong with types.
        let pubkey = PublicKey::from_bytes(author.public_key().as_bytes())?;
        
        let id = Self::nostr_id(pubkey, created_at, kind, &tags, &content)?;
        let sig = author.sign(id.as_bytes());
        Ok(Event {
            id,
            pubkey,
            created_at,
            kind,
            tags,
            sig,
            content: content.into(),
        })
    }

    pub(crate) fn nostr_id(
        pubkey: PublicKey,
        created_at: i64,
        kind: EventKind,
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

    pub(crate) fn schema(&self) -> Result<Option<Hash>> {
        let schema_tag = self
            .tags
            .iter()
            .find(|tag| tag.0 == NOSTR_SCHEMA_TAG);

        match schema_tag {
            Some(tag) => {
                let hash = Hash::from_str(&tag.1)?;
                Ok(Some(hash))
            }
            None => Ok(None),
        }
    }

    pub(crate) fn data_id(&self) -> Result<Option<Uuid>> {
        let id_tag = self
            .tags
            .iter()
            .find(|tag| tag.0 == NOSTR_ID_TAG);

        match id_tag  {
            Some(tag) => {
                let id = Uuid::parse_str(&tag.1).map_err(|e| anyhow::anyhow!(e))?;
                Ok(Some(id))
            }
            None => Ok(None),
        }
    }


    pub(crate) async fn write(&self, db: &DB) -> Result<()> {
        let schema = match self.schema()? {
            Some(s) => Some(s.to_string()),
            None => None,
        };
        let data_id = self.data_id()?;
        let content = match self.content {
            HashOrContent::Hash(ref hash) => hash.to_string(),
            HashOrContent::Content(_) => anyhow::bail!("content must be a hash"),
        };

        let conn = db.lock().await;
        conn.execute(
            "INSERT INTO events (id, pubkey, created_at, kind, schema, data_id, content, sig) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                self.id.to_string(),
                self.pubkey.to_string(),
                self.created_at,
                self.kind,
                schema,
                data_id, 
                content,
                self.sig.to_string(),
            ],
        )?;
        Ok(())
    }

    pub(crate) fn from_sql_row(row: &rusqlite::Row) -> Result<Self> {
        // (0   1       2           3     4       5        6       7)
        // (id, pubkey, created_at, kind, schema, data_id, content, sig)
        let pubkey = String::from(row.get(1)?);
        let pubkey = PublicKey::from_str(&pubkey)?;
        let id: String = row.get(0)?;
        let content: String = row.get(6)?;
        let data_id: Uuid = row.get(5)?;
        let data: [&u8] = row.get(7)?;
        let sig = Signature::from_bytes(data).map_err(|e| anyhow!(e))?;
        Ok(Self {
            id: Sha256Digest::from_str(&id).map_err(|e| anyhow!(e))?,
            pubkey,
            created_at: row.get(2)?,
            kind: row.get(3)?,
            tags: vec![
                Tag(NOSTR_SCHEMA_TAG.to_string(), row.get(4)?, None),
                Tag(NOSTR_ID_TAG.to_string(), data_id.to_string(), None),
            ],
            content: Hash::from_str(&content)?.into(),
            sig,
        })
    }
}

// Define the EventObject trait
pub trait EventObject {
    async fn from_event(event: Event, client: &RouterClient) -> Result<Self>
    where Self: Sized;
    fn into_mutate_event(&self, author: Author) -> Result<Event>;
}