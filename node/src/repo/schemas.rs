use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use bytes::Bytes;
use iroh::blobs::Hash;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::router::{Router, RouterClient};

use super::events::{nostr_id, EventKind, HashOrContent};
use super::{Repo, DB};

#[derive(Debug, Serialize, Deserialize)]
struct SchemaMetadata {
    title: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Schema {
    pub title: String,
    pub hash: Hash,
    data: Option<Bytes>,
}

impl Schema {
    // pub async fn new(title: String, hash: Hash) -> Self {
    //     Schema {
    //         title,
    //         hash,
    //         data: None,
    //     }
    // }

    pub async fn load(router: &RouterClient, hash: Hash) -> Result<Self> {
        let bytes = router.blobs().read_to_bytes(hash).await?;
        let meta: SchemaMetadata = serde_json::from_slice(&bytes)?;
        let bytes = router.blobs().read_to_bytes(hash).await?;

        Ok(Schema {
            title: meta.title,
            hash,
            data: Some(bytes),
        })
    }

    pub fn validator(&self) -> Result<jsonschema::Validator> {
        match &self.data {
            Some(data) => {
                let schema = serde_json::from_slice(data)?;
                jsonschema::validator_for(&schema).context("failed to create validator")
            }
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
    //             EventKind::MutateSchema,
    //             &vec![],
    //             &content,
    //         )?,
    //         pubkey: self.pubkey.clone(),
    //         created_at: self.created_at,
    //         kind: EventKind::MutateSchema,
    //         tags: vec![Tag(NOSTR_SCHEMA_TAG.to_string(), schema.to_string(), None)],
    //         sig: "".to_string(),
    //         content: HashOrContent::Content(content),
    //     };
    //     event.write(db).await
    // }
}

#[derive(Clone)]
pub struct Schemas(Repo);

impl Schemas {
    pub fn new(repo: Repo) -> Self {
        Schemas(repo)
    }

    pub async fn create(&self, data: Bytes) -> Result<Schema> {
        // extract the title from the schema
        let meta: SchemaMetadata = serde_json::from_slice(&data)?;

        // confirm our data is a valid JSON schema
        let schema = serde_json::from_slice(&data)?;
        jsonschema::validator_for(&schema)?;

        // serialize data & add locally
        // TODO - test that this enforces field ordering
        let serialized = serde_json::to_vec(&schema)?;

        // TODO - should construct a HashSeq, place the new schema as the 1th element
        // and update the metadata in 0th element
        let res = self.0.router.blobs().add_bytes(serialized).await?;

        Ok(Schema {
            title: meta.title,
            hash: res.hash,
            data: Some(data),
        })
    }

    pub async fn load_or_create(&self, data: Bytes) -> Result<Schema> {
        let meta: SchemaMetadata = serde_json::from_slice(&data)?;

        let schema = self.get_by_name(&meta.title).await;
        match schema {
            Ok(schema) => Ok(schema),
            Err(_) => self.create(data).await,
        }
    }

    pub async fn load(&self, hash: Hash) -> Result<Schema> {
        Schema::load(self.0.router(), hash).await
    }

    // pub async fn mutate(&self, _id: Hash, data: &str) -> Result<Hash> {
    //     let schema = Schema::new(data.to_string());
    //     // TODO - should construct a HashSeq, place the new schema as the 1th element
    //     // and update the metadata in 0th element
    //     // schema.write(&self.db).await
    //     schema.id()
    // }

    pub async fn get_by_name(&self, name: &str) -> Result<Schema> {
        // TODO - SLOW
        self.list(0, -1)
            .await?
            .into_iter()
            .find(|schema| schema.title == name)
            .ok_or_else(|| anyhow!("schema not found"))
    }

    pub async fn list(&self, offset: i64, limit: i64) -> Result<Vec<Schema>> {
        let conn = self.0.db.lock().await;
        let mut stmt = conn.prepare("SELECT DISTINCT schema FROM events LIMIT ?1 OFFSET ?2")?;
        let mut rows = stmt.query([limit, offset])?;

        let mut schemas = Vec::new();
        while let Some(row) = rows.next()? {
            let hash: String = row.get(0)?;
            let hash = Hash::from_str(&hash)?;
            let schema = Schema::load(self.0.router(), hash).await?;
            schemas.push(schema);
        }
        Ok(schemas)
    }
}
