use anyhow::{anyhow, Context, Result};
use bytes::Bytes;
use iroh::blobs::Hash;
use iroh::docs::Author;
use iroh::net::key::PublicKey;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::events::{
    Event, EventKind, EventObject, HashLink, Tag, EVENT_SQL_READ_FIELDS, NOSTR_ID_TAG,
};
use super::rows::Row;
use super::Space;
use crate::router::RouterClient;

#[derive(Debug, Serialize, Deserialize)]
struct TableMetadata {
    title: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Table {
    pub id: Uuid,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    pub author: PublicKey,
    pub content: HashLink,
    pub title: String,
}

impl EventObject for Table {
    async fn from_event(event: Event, client: &RouterClient) -> Result<Self> {
        if event.kind != EventKind::MutateTable {
            return Err(anyhow!("event is not a schema mutation"));
        }

        // normalize tags
        let id = event.data_id()?.ok_or_else(|| anyhow!("missing data id"))?;

        // fetch content if necessary
        // TODO(b5): I know the double serializing is terrible
        let (content, title) = match event.content.data {
            None => {
                let content = client.blobs().read_to_bytes(event.content.hash).await?;
                let size = content.len();
                let meta =
                    serde_json::from_slice::<TableMetadata>(&content).map_err(|e| anyhow!(e))?;
                let content = serde_json::from_slice::<Value>(&content).map_err(|e| anyhow!(e))?;
                (
                    HashLink {
                        hash: event.content.hash,
                        size: Some(size as u64),
                        data: Some(content),
                    },
                    meta.title,
                )
            }
            Some(ref v) => {
                let data = serde_json::to_vec(v)?;
                let meta =
                    serde_json::from_slice::<TableMetadata>(&data).map_err(|e| anyhow!(e))?;
                (event.content, meta.title)
            }
        };

        Ok(Table {
            author: event.pubkey,
            id,
            created_at: event.created_at,
            content,
            title,
        })
    }

    fn into_mutate_event(&self, author: Author) -> Result<Event> {
        // assert!(author.public_key() == self.author);
        let tags = vec![Tag::new(NOSTR_ID_TAG, self.id.to_string().as_str())];
        Event::create(
            author,
            self.created_at,
            EventKind::MutateTable,
            tags,
            self.content.clone(),
        )
    }
}

impl Table {
    async fn from_sql_row(row: &rusqlite::Row<'_>, router: &RouterClient) -> Result<Table> {
        let event = Event::from_sql_row(row)?;
        Self::from_event(event, router).await
    }

    // pub async fn load(router: &RouterClient, hash: Hash) -> Result<Self> {
    //     let bytes = router.blobs().read_to_bytes(hash).await?;
    //     let meta: SchemaMetadata = serde_json::from_slice(&bytes)?;
    //     let data = serde_json::from_slice(&bytes)?;

    //     Ok(Schema {
    //         title: meta.title,
    //         hash,
    //         data: Some(data),
    //     })
    // }

    // pub fn id(&self) -> Result<Hash> {
    //     let res = serde_json::to_string(self).map(|data| Hash::from_str(data.as_str()))??;
    //     Ok(res)
    // }

    pub async fn validator(&mut self, router: &RouterClient) -> Result<jsonschema::Validator> {
        let value = self.content.resolve(router).await?;
        jsonschema::validator_for(&value).context("failed to create validator")
    }

    pub async fn create_row(
        &mut self,
        space: &Space,
        author: Author,
        data: serde_json::Value,
    ) -> Result<Row> {
        let id = Uuid::new_v4();
        self.mutate_row(space, author, id, data).await
    }

    pub async fn mutate_row(
        &mut self,
        space: &Space,
        author: Author,
        id: Uuid,
        data: serde_json::Value,
    ) -> Result<Row> {
        let router = space.router();
        // validate data matches schema
        let validator = self.validator(router).await.context("getting validator")?;
        if let Err(e) = validator.validate(&data) {
            return Err(anyhow!("validation error: {}", e.to_string()));
        };

        // add to iroh
        let data2 = serde_json::to_vec(&data)?;
        let size = data2.len();
        let outcome = router.blobs().add_bytes(data2).await?;
        let created_at = chrono::Utc::now().timestamp();
        let hash = outcome.hash;

        // construct row
        let row = Row {
            // TODO(b5) - wat. why? you're doing something wrong with types.
            author: PublicKey::from_bytes(author.public_key().as_bytes())?,
            id,
            schema: self.content.hash,
            created_at,
            content: HashLink {
                hash,
                size: Some(size as u64),
                data: Some(data),
            },
        };

        // write event
        let event = row.into_mutate_event(author)?;
        event.write(&space.db).await?;

        Ok(row)
    }
}

#[derive(Clone)]
pub struct Tables(Space);

impl Tables {
    pub fn new(repo: Space) -> Self {
        Tables(repo)
    }

    pub async fn load_or_create(&self, author: Author, data: Bytes) -> Result<Table> {
        let meta: TableMetadata = serde_json::from_slice(&data)?;

        let schema = self.get_by_title(&meta.title).await;
        match schema {
            Ok(schema) => Ok(schema),
            Err(_) => self.create(author, data).await,
        }
    }

    pub async fn create(&self, author: Author, data: Bytes) -> Result<Table> {
        let id = Uuid::new_v4();
        self.mutate(author, id, data).await
    }

    pub async fn mutate(&self, author: Author, id: Uuid, data: Bytes) -> Result<Table> {
        // let schema = Schema::new(data.to_string());
        // TODO - should construct a HashSeq, place the new schema as the 1th element
        // and update the metadata in 0th element
        // schema.write(&self.db).await
        // schema.id()

        // extract the title from the schema
        let meta: TableMetadata = serde_json::from_slice(&data)?;

        // confirm our data is a valid JSON schema
        let schema = serde_json::from_slice(&data)?;
        jsonschema::validator_for(&schema)?;

        // serialize data & add locally
        // TODO - test that this enforces field ordering
        let serialized = serde_json::to_vec(&schema)?;
        let size = serialized.len();
        let res = self.0.router.blobs().add_bytes(serialized).await?;

        let schema = Table {
            id,
            created_at: chrono::Utc::now().timestamp(),
            title: meta.title,
            // TODO(b5) - wat. why? you're doing something wrong with types.
            author: PublicKey::from_bytes(author.public_key().as_bytes())?,
            content: HashLink {
                hash: res.hash,
                size: Some(size as u64),
                data: Some(schema),
            },
        };

        let event = schema.into_mutate_event(author)?;
        event.write(&self.0.db).await?;

        Ok(schema)
    }

    pub async fn get_by_title(&self, name: &str) -> Result<Table> {
        // TODO - SLOW
        self.list(0, -1)
            .await?
            .into_iter()
            .find(|schema| schema.title == name)
            .ok_or_else(|| anyhow!("schema not found"))
    }

    pub async fn get_by_hash(&self, hash: Hash) -> Result<Table> {
        // TODO - SLOW
        let conn = self.0.db.lock().await;
        let mut stmt = conn
            .prepare(
                format!(
                    "SELECT {EVENT_SQL_READ_FIELDS} FROM events WHERE kind = ?1 AND content_hash = ?2"
                )
                .as_str(),
            )
            .context("selecting schemas from events table")?;

        let mut rows = stmt.query(params![EventKind::MutateTable, hash.to_string()])?;
        if let Some(row) = rows.next()? {
            return Table::from_sql_row(row, &self.0.router).await;
        }

        Err(anyhow!("schema not found"))
    }

    pub async fn list(&self, offset: i64, limit: i64) -> Result<Vec<Table>> {
        let conn = self.0.db.lock().await;
        let mut stmt = conn
            .prepare(
                format!(
                    "SELECT {EVENT_SQL_READ_FIELDS} FROM events WHERE kind = ?1 LIMIT ?2 OFFSET ?3"
                )
                .as_str(),
            )
            .context("selecting schemas from events table")?;
        let mut rows = stmt.query(rusqlite::params![EventKind::MutateTable, limit, offset])?;

        let mut schemas = Vec::new();
        while let Some(row) = rows.next()? {
            let schema = Table::from_sql_row(row, &self.0.router)
                .await
                .context("parsing schema row")?;
            schemas.push(schema);
        }

        Ok(schemas)
    }
}
