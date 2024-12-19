use anyhow::{anyhow, Result};
use iroh::PublicKey;
use iroh_docs::Author;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::events::{Event, EventKind, EventObject, HashLink, Tag, NOSTR_ID_TAG};
use super::{Space, EVENT_SQL_READ_FIELDS};
use crate::iroh::Protocols;

#[derive(Debug, Serialize, Deserialize)]
pub struct SpaceDetails {
    pub title: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpaceEvent {
    pub id: Uuid,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    pub author: PublicKey,
    pub content: HashLink,
}

impl EventObject for SpaceEvent {
    async fn from_event(event: Event, client: &Protocols) -> Result<Self> {
        if event.kind != EventKind::MutateSpace {
            return Err(anyhow!("event is not a schema mutation"));
        }

        // normalize tags
        let id = event.data_id()?.ok_or_else(|| anyhow!("missing data id"))?;

        // fetch content if necessary
        // TODO(b5): I know the double serializing is terrible
        let (content, _title) = match event.content.data {
            None => {
                let content = client.blobs().read_to_bytes(event.content.hash).await?;
                let size = content.len();
                let meta =
                    serde_json::from_slice::<SpaceDetails>(&content).map_err(|e| anyhow!(e))?;
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
                let meta = serde_json::from_slice::<SpaceDetails>(&data).map_err(|e| anyhow!(e))?;
                (event.content, meta.title)
            }
        };

        Ok(SpaceEvent {
            author: event.pubkey,
            id,
            created_at: event.created_at,
            content,
        })
    }

    fn into_mutate_event(&self, author: Author) -> Result<Event> {
        // assert!(author.public_key() == self.author);
        let tags = vec![Tag::new(NOSTR_ID_TAG, self.id.to_string().as_str())];
        Event::create(
            author,
            self.created_at,
            EventKind::MutateSpace,
            tags,
            self.content.clone(),
        )
    }
}

#[derive(Clone)]
pub struct SpaceEvents(Space);

impl SpaceEvents {
    pub fn new(space: Space) -> Self {
        SpaceEvents(space)
    }

    pub async fn mutate(
        &self,
        author: Author,
        id: Uuid,
        details: SpaceDetails,
    ) -> Result<SpaceEvent> {
        // serialize data & add locally
        // TODO - test that this enforces field ordering
        let serialized = serde_json::to_vec(&details)?;
        let size = serialized.len();
        let v = serde_json::from_slice::<Value>(&serialized)?;
        let res = self.0.router.blobs().add_bytes(serialized).await?;

        let schema = SpaceEvent {
            id,
            created_at: chrono::Utc::now().timestamp(),
            // TODO(b5) - wat. why? you're doing something wrong with types.
            author: PublicKey::from_bytes(author.public_key().as_bytes())?,
            content: HashLink {
                hash: res.hash,
                size: Some(size as u64),
                data: Some(v),
            },
        };

        let event = schema.into_mutate_event(author)?;
        event.write(&self.0.db).await?;

        Ok(schema)
    }

    pub async fn read(&self) -> Result<SpaceEvent> {
        let conn = self.0.db.lock().await;
        let mut stmt = conn.prepare(
            format!("SELECT {EVENT_SQL_READ_FIELDS} FROM events WHERE kind = ?1 ORDER BY created_at DESC LIMIT 1 OFFSET 0").as_str()
        )?;
        let mut rows = stmt.query(params![EventKind::MutateSpace])?;
        if let Some(row) = rows.next()? {
            let event = Event::from_sql_row(row)?;
            let event = SpaceEvent::from_event(event, &self.0.router).await?;
            return Ok(event);
        }
        Err(anyhow!("no event found"))
    }
}
