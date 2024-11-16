use anyhow::{anyhow, Context, Result};
use iroh::blobs::Hash;
use iroh::docs::Author;
use iroh::net::key::PublicKey;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::repo::events::Tag;
use crate::router::RouterClient;

use super::events::{Event, EventKind, EventObject, HashOrContent, NOSTR_ID_TAG, NOSTR_SCHEMA_TAG};
use super::Repo;

#[derive(Debug, Serialize, Deserialize)]
pub struct Row {
    pub id: Uuid,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    pub author: PublicKey,
    pub content: HashOrContent,
    pub schema: Hash,
}

impl EventObject for Row {
    async fn from_event(event: Event, client: &RouterClient) -> Result<Self> {
        if event.kind != EventKind::MutateRow {
            return Err(anyhow!("event is not a row mutation"));
        }

        // normalize tags
        let schema = event.schema()?.ok_or_else(|| anyhow!("no schema found"))?;
        let id = event.data_id()?.ok_or_else(|| anyhow!("missing data id"))?;

        // fetch content if necessary
        let content = match event.content {
            HashOrContent::Hash(hash) => {
                let content = client.blobs().read_to_bytes(hash).await?;
                let content = serde_json::from_slice::<Value>(&content).map_err(|e| anyhow!(e))?;
                HashOrContent::Content(content)
            }
            HashOrContent::Content(v) => HashOrContent::Content(v),
        };

        Ok(Row {
            author: event.pubkey,
            id,
            schema,
            created_at: event.created_at,
            content,
        })
    }

    fn into_mutate_event(&self, author: Author) -> Result<Event> {
        // assert!(author.public_key() == self.author);
        let tags = vec![
            Tag::new(NOSTR_SCHEMA_TAG, self.schema.to_string().as_str()),
            Tag::new(NOSTR_ID_TAG, self.id.to_string().as_str()),
        ];
        let content = match self.content {
            HashOrContent::Hash(hash) => hash,
            HashOrContent::Content(_) => anyhow::bail!("content must be a hash"),
        };
        Event::create(author, self.created_at, EventKind::MutateRow, tags, content)
    }
}

impl Row {
    async fn from_sql_row(row: &rusqlite::Row<'_>, client: &RouterClient) -> Result<Row> {
        let event = Event::from_sql_row(row)?;
        Self::from_event(event, client).await
    }
}

#[derive(Clone)]
pub struct Rows(Repo);

impl Rows {
    pub fn new(repo: Repo) -> Self {
        Rows(repo)
    }

    pub async fn create(
        &self,
        author: Author,
        schema: Hash,
        data: serde_json::Value,
    ) -> Result<Row> {
        let data_id = Uuid::new_v4();
        self.mutate(author, schema, data_id, data).await
    }

    pub async fn mutate(
        &self,
        author: Author,
        schema_hash: Hash,
        id: Uuid,
        data: serde_json::Value,
    ) -> Result<Row> {
        self.0
            .schemas()
            .get_by_hash(schema_hash)
            .await
            .context("loading schema")?
            .mutate_row(&self.0, author, id, data)
            .await
    }

    pub async fn query(
        &self,
        schema: Hash,
        _query: String,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Row>> {
        let conn = self.0.db.lock().await;
        let mut stmt = conn.prepare("SELECT id, pubkey, created_at, kind, schema, data_id, content, sig FROM events WHERE schema = ?1 LIMIT ?2 OFFSET ?3")?;
        let mut rows = stmt.query(params![schema.to_string(), limit, offset])?;
        let mut events = Vec::new();

        while let Some(row) = rows.next()? {
            let event = Row::from_sql_row(row, &self.0.router).await?;
            events.push(event);
        }
        Ok(events)
    }
}
