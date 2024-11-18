use anyhow::{anyhow, Result};
use bytes::Bytes;
use iroh::docs::Author;
use iroh::net::key::PublicKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::events::{Event, EventKind, EventObject, HashLink, Tag, NOSTR_ID_TAG};
use super::Space;
use crate::router::RouterClient;

#[derive(Debug, Serialize, Deserialize)]
struct SpaceDetails {
    title: String,
    description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpaceEvent {
    pub id: Uuid,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    pub author: PublicKey,
    pub content: HashLink,
    pub title: String,
}

impl EventObject for SpaceEvent {
    async fn from_event(event: Event, client: &RouterClient) -> Result<Self> {
        if event.kind != EventKind::MutateSpace {
            return Err(anyhow!("event is not a schema mutation"));
        }

        // normalize tags
        let id = event.data_id()?.ok_or_else(|| anyhow!("missing data id"))?;

        // fetch content if necessary
        // TODO(b5): I know the double serializing is terrible
        let (content, title) = match event.content.value {
            None => {
                let content = client.blobs().read_to_bytes(event.content.hash).await?;
                let meta =
                    serde_json::from_slice::<SpaceDetails>(&content).map_err(|e| anyhow!(e))?;
                let content = serde_json::from_slice::<Value>(&content).map_err(|e| anyhow!(e))?;
                (
                    HashLink {
                        hash: event.content.hash,
                        value: Some(content),
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
            title,
        })
    }

    fn into_mutate_event(&self, author: Author) -> Result<Event> {
        // assert!(author.public_key() == self.author);
        let tags = vec![Tag::new(NOSTR_ID_TAG, self.id.to_string().as_str())];
        Event::create(
            author,
            self.created_at,
            EventKind::MutateSchema,
            tags,
            self.content.clone(),
        )
    }
}

impl SpaceEvent {
    async fn from_sql_row(row: &rusqlite::Row<'_>, client: &RouterClient) -> Result<SpaceEvent> {
        let event = Event::from_sql_row(row)?;
        Self::from_event(event, client).await
    }
}

#[derive(Clone)]
pub struct SpaceEvents(Space);

impl SpaceEvents {
    pub fn new(space: Space) -> Self {
        SpaceEvents(space)
    }

    pub async fn create(
        &self,
        router: &RouterClient,
        author: Author,
        data: Bytes,
    ) -> Result<SpaceEvent> {
        let id = Uuid::new_v4();
        self.mutate(router, author, id, data).await
    }

    pub async fn mutate(
        &self,
        router: &RouterClient,
        author: Author,
        id: Uuid,
        data: Bytes,
    ) -> Result<SpaceEvent> {
        // let schema = Schema::new(data.to_string());
        // TODO - should construct a HashSeq, place the new schema as the 1th element
        // and update the metadata in 0th element
        // schema.write(&self.db).await
        // schema.id()

        // extract the title from the schema
        let meta: SpaceDetails = serde_json::from_slice(&data)?;

        // confirm our data is a valid JSON schema
        let schema = serde_json::from_slice(&data)?;
        jsonschema::validator_for(&schema)?;

        // serialize data & add locally
        // TODO - test that this enforces field ordering
        let serialized = serde_json::to_vec(&schema)?;

        let res = router.blobs().add_bytes(serialized).await?;

        let schema = SpaceEvent {
            id,
            created_at: chrono::Utc::now().timestamp(),
            title: meta.title,
            // TODO(b5) - wat. why? you're doing something wrong with types.
            author: PublicKey::from_bytes(author.public_key().as_bytes())?,
            content: HashLink {
                hash: res.hash,
                value: None,
            },
        };

        let event = schema.into_mutate_event(author)?;
        event.write(&self.0.db).await?;

        Ok(schema)
    }
}
