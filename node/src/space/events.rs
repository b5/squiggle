use std::fmt::Write;

use anyhow::{anyhow, Context, Result};
use ed25519_dalek::Signature;
use iroh::blobs::Hash;
use iroh::docs::Author;
use iroh::net::key::PublicKey;
use rusqlite::types::{FromSql, ToSqlOutput};
use rusqlite::{params, ToSql};
use serde::ser::SerializeStruct;
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

pub(crate) const EVENT_SQL_FIELDS: &str =
    "id, pubkey, created_at, kind, schema, data_id, content, sig";

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum EventKind {
    MutateUser,
    DeleteUser,
    MutateSpace,
    DeleteSpace,
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
            EventKind::MutateUser => 100000,
            EventKind::DeleteUser => 100001,
            EventKind::MutateSpace => 100002,
            EventKind::DeleteSpace => 100003,
            EventKind::MutateProgram => 100004,
            EventKind::DeleteProgram => 100005,
            EventKind::MutateSchema => 100006,
            EventKind::DeleteSchema => 100007,
            EventKind::MutateRow => 100008,
            EventKind::DeleteRow => 100009,
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
            100000 => Ok(EventKind::MutateUser),
            100001 => Ok(EventKind::DeleteUser),
            100002 => Ok(EventKind::MutateSpace),
            100003 => Ok(EventKind::DeleteSpace),
            100004 => Ok(EventKind::MutateProgram),
            100005 => Ok(EventKind::DeleteProgram),
            100006 => Ok(EventKind::MutateSchema),
            100007 => Ok(EventKind::DeleteSchema),
            100008 => Ok(EventKind::MutateRow),
            100009 => Ok(EventKind::DeleteRow),
            _ => Err(rusqlite::types::FromSqlError::OutOfRange(kind.into())),
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
            100000 => Ok(EventKind::MutateUser),
            100001 => Ok(EventKind::DeleteUser),
            100002 => Ok(EventKind::MutateSpace),
            100003 => Ok(EventKind::DeleteSpace),
            100004 => Ok(EventKind::MutateProgram),
            100005 => Ok(EventKind::DeleteProgram),
            100006 => Ok(EventKind::MutateSchema),
            100007 => Ok(EventKind::DeleteSchema),
            100008 => Ok(EventKind::MutateRow),
            100009 => Ok(EventKind::DeleteRow),
            _ => Err(serde::de::Error::custom(format!(
                "Unknown event kind: {}",
                kind
            ))),
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
    bytes.iter().fold(String::new(), |mut output, b| {
        let _ = write!(output, "{b:02x}");
        output
    })
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

#[derive(Debug, Clone)]
pub struct HashLink {
    pub hash: Hash,
    pub value: Option<Value>,
}

impl From<Hash> for HashLink {
    fn from(hash: Hash) -> Self {
        HashLink { hash, value: None }
    }
}

impl Serialize for HashLink {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match &self.value {
            Some(value) => {
                let mut state = serializer.serialize_struct("HashLink", 2)?;
                state.serialize_field("hash", &self.hash.to_string())?;
                state.serialize_field("value", value)?;
                state.end()
            }
            None => serializer.serialize_str(&self.hash.to_string()),
        }
    }
}

impl<'de> Deserialize<'de> for HashLink {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct HashLinkVisitor;

        impl<'de> serde::de::Visitor<'de> for HashLinkVisitor {
            type Value = HashLink;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string or a struct representing a HashLink")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let hash = Hash::from_str(value).map_err(E::custom)?;
                Ok(HashLink::from(hash))
            }

            fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                #[derive(Deserialize)]
                struct HashLinkStruct {
                    hash: Hash,
                    value: Option<Value>,
                }

                let hash_link_struct =
                    HashLinkStruct::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(HashLink {
                    hash: hash_link_struct.hash,
                    value: hash_link_struct.value,
                })
            }
        }

        deserializer.deserialize_any(HashLinkVisitor)
    }
}

impl HashLink {
    pub async fn resolve(&mut self, router: &RouterClient) -> Result<Value> {
        match self.value {
            Some(ref v) => Ok(v.clone()),
            None => {
                let data = router.blobs().read_to_bytes(self.hash).await?;
                let value: Value = serde_json::from_slice(&data)?;
                self.value = Some(value.clone());
                Ok(value)
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Tag(String, String, Option<String>);

impl Tag {
    pub fn new(name: &str, value: &str) -> Self {
        Tag(name.to_string(), value.to_string(), None)
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
pub(crate) struct Event {
    pub id: Sha256Digest,
    pub pubkey: PublicKey,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    pub kind: EventKind,
    pub tags: Vec<Tag>,
    pub sig: Signature,
    pub content: HashLink,
}

impl Event {
    pub(crate) fn create(
        author: Author,
        created_at: i64,
        kind: EventKind,
        tags: Vec<Tag>,
        content: HashLink,
    ) -> Result<Event> {
        // TODO(b5) - wat. why? you're doing something wrong with types.
        let pubkey = PublicKey::from_bytes(author.public_key().as_bytes())?;

        let id = Self::nostr_id(pubkey, created_at, kind, &tags, &content.hash)?;
        let sig = author.sign(id.as_bytes());
        Ok(Event {
            id,
            pubkey,
            created_at,
            kind,
            tags,
            sig,
            content,
        })
    }

    /// read a raw event, usually not what you want. Worth preferring higher-level reads like
    /// programs, schemas, etc.
    pub(crate) async fn read_raw(db: &DB, id: Uuid) -> Result<Self> {
        let conn = db.lock().await;
        let mut stmt = conn.prepare(
            format!("SELECT {EVENT_SQL_FIELDS} FROM events WHERE id = ?1 ORDER BY CREATED DESC")
                .as_str(),
        )?;
        // TODO(b5) - use query_row with mapping func
        let mut rows = stmt.query(params![id])?;
        let row = rows.next()?.ok_or_else(|| anyhow!("event not found"))?;
        Event::from_sql_row(row)
    }

    pub(crate) async fn ingest_from_blob(
        db: &DB,
        router: &RouterClient,
        hash: Hash,
    ) -> Result<Self> {
        let data = router.blobs().read_to_bytes(hash).await?;
        let event: Self = serde_json::from_slice(&data)?;
        event.write(db).await?;
        Ok(event)
    }

    /// write a raw event to a blob, again usually not what you want. Events are stored in the
    /// sqlite db. This is for when we want to share events with others.
    pub(crate) async fn write_raw_to_blob(
        &self,
        router: &RouterClient,
    ) -> Result<(Hash, iroh::blobs::Tag)> {
        let data = serde_json::to_vec(&self)?;
        let result = router.blobs().add_bytes(data).await?;
        Ok((result.hash, result.tag))
    }

    fn nostr_id(
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
        let schema_tag = self.tags.iter().find(|tag| tag.0 == NOSTR_SCHEMA_TAG);

        match schema_tag {
            Some(tag) => {
                let hash = Hash::from_str(&tag.1)?;
                Ok(Some(hash))
            }
            None => Ok(None),
        }
    }

    pub(crate) fn data_id(&self) -> Result<Option<Uuid>> {
        let id_tag = self.tags.iter().find(|tag| tag.0 == NOSTR_ID_TAG);

        match id_tag {
            Some(tag) => {
                let id = Uuid::parse_str(&tag.1).map_err(|e| anyhow::anyhow!(e))?;
                Ok(Some(id))
            }
            None => Ok(None),
        }
    }

    pub(crate) async fn write(&self, db: &DB) -> Result<()> {
        let schema = self.schema()?.map(|s| s.to_string());
        let data_id = self.data_id()?;

        let conn = db.lock().await;
        conn.execute(
            format!(
                "INSERT INTO events ({EVENT_SQL_FIELDS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
            )
            .as_str(),
            params![
                self.id.to_string(),
                self.pubkey.to_string(),
                self.created_at,
                self.kind,
                schema,
                data_id,
                self.content.hash.to_string(),
                self.sig.to_bytes(),
            ],
        )
        .context("inserting event")?;
        Ok(())
    }

    pub(crate) fn from_sql_row(row: &rusqlite::Row) -> Result<Self> {
        // (0   1       2           3     4       5        6       7)
        // (id, pubkey, created_at, kind, schema, data_id, content, sig)
        let id: String = row.get(0)?;
        let pubkey: String = row.get(1)?;
        let pubkey = PublicKey::from_str(&pubkey)?;
        let content: String = row.get(6)?;
        let data_id: Uuid = row.get(5)?;
        let sig_data: Vec<u8> = row.get(7)?;
        let sig_data: [u8; 64] = sig_data
            .try_into()
            .map_err(|_| anyhow!("invalid signature data"))?;
        let sig = Signature::from_bytes(&sig_data);

        let mut tags: Vec<Tag> = Vec::new();
        let schema: Option<String> = row.get(4)?;
        if let Some(schema) = schema {
            tags.push(Tag(NOSTR_SCHEMA_TAG.to_string(), schema, None));
        }
        tags.push(Tag(NOSTR_ID_TAG.to_string(), data_id.to_string(), None));

        Ok(Self {
            id: Sha256Digest::from_str(&id).map_err(|e| anyhow!(e))?,
            pubkey,
            created_at: row.get(2)?,
            kind: row.get(3)?,
            tags,
            content: Hash::from_str(&content)?.into(),
            sig,
        })
    }
}

// Define the EventObject trait
pub(crate) trait EventObject {
    async fn from_event(event: Event, client: &RouterClient) -> Result<Self>
    where
        Self: Sized;
    fn into_mutate_event(&self, author: Author) -> Result<Event>;
}
