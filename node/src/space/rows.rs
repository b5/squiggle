use anyhow::{anyhow, Context, Result};
use iroh::blobs::Hash;
use iroh::docs::Author;
use iroh::net::key::PublicKey;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::router::RouterClient;
use crate::space::events::Tag;

use super::events::{Event, EventKind, EventObject, HashLink, NOSTR_ID_TAG, NOSTR_SCHEMA_TAG};
use super::Space;

#[derive(Debug, Serialize, Deserialize)]
pub struct Row {
    pub id: Uuid,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    pub author: PublicKey,
    pub content: HashLink,
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
        let content = match event.content.value {
            Some(_) => event.content,
            None => {
                let content = client.blobs().read_to_bytes(event.content.hash).await?;
                let content = serde_json::from_slice::<Value>(&content).map_err(|e| anyhow!(e))?;
                HashLink {
                    hash: event.content.hash,
                    value: Some(content),
                }
            }
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
        Event::create(
            author,
            self.created_at,
            EventKind::MutateRow,
            tags,
            self.content.clone(),
        )
    }
}

impl Row {
    async fn from_sql_row(row: &rusqlite::Row<'_>, client: &RouterClient) -> Result<Row> {
        let event = Event::from_sql_row(row)?;
        Self::from_event(event, client).await
    }
}

#[derive(Clone)]
pub struct Rows(Space);

impl Rows {
    pub fn new(repo: Space) -> Self {
        Rows(repo)
    }

    pub async fn create(
        &self,
        router: &RouterClient,
        author: Author,
        schema: Hash,
        data: serde_json::Value,
    ) -> Result<Row> {
        let data_id = Uuid::new_v4();
        self.mutate(router, author, schema, data_id, data).await
    }

    pub async fn mutate(
        &self,
        router: &RouterClient,
        author: Author,
        schema_hash: Hash,
        id: Uuid,
        data: serde_json::Value,
    ) -> Result<Row> {
        self.0
            .schemas()
            .get_by_hash(router, schema_hash)
            .await
            .context("loading schema")?
            .mutate_row(router, &self.0, author, id, data)
            .await
    }

    pub async fn query(
        &self,
        router: &RouterClient,
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
            let event = Row::from_sql_row(row, router).await?;
            events.push(event);
        }
        Ok(events)
    }
}
