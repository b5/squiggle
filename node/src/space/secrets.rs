use std::collections::HashMap;

use anyhow::{anyhow, Result};
use iroh::docs::Author;
use iroh::net::key::PublicKey;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::router::RouterClient;

use super::events::{Event, EventKind, EventObject, HashLink, Tag, NOSTR_ID_TAG};
use super::{Space, EVENT_SQL_READ_FIELDS};

pub type SecretsConfig = HashMap<String, String>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Secret {
    pub program_id: Uuid, // always maps to the program ID
    pub created_at: i64,
    pub pubkey: PublicKey,
    pub content: HashLink,
    pub config: SecretsConfig,
}

impl EventObject for Secret {
    async fn from_event(event: Event, _router: &RouterClient) -> Result<Self> {
        if event.kind != EventKind::MutateSecret {
            return Err(anyhow!("event is not a user mutation"));
        }

        // normalize tags
        let id = event.data_id()?.ok_or_else(|| anyhow!("missing data id"))?;

        // fetch content if necessary
        let content = event.content.clone();
        let config = match content.data {
            Some(content) => {
                let env: SecretsConfig = serde_json::from_value(content)?;
                env
            }
            // TODO (b5): this is almost definitely not what we want, but we shouldn't storing secrets
            // in the blob store, which means we shouldn't be reading here.
            None => HashMap::new(),
        };

        Ok(Secret {
            program_id: id,
            pubkey: event.pubkey,
            created_at: event.created_at,
            content: event.content,
            config,
        })
    }

    fn into_mutate_event(&self, author: Author) -> Result<Event> {
        // assert!(author.public_key() == self.author);
        let tags = vec![Tag::new(NOSTR_ID_TAG, self.program_id.to_string().as_str())];
        Event::create(
            author,
            self.created_at,
            EventKind::MutateSecret,
            tags,
            self.content.clone(),
        )
    }
}

impl Secret {
    async fn from_sql_row(row: &rusqlite::Row<'_>, client: &RouterClient) -> Result<Secret> {
        let event = Event::from_sql_row(row)?;
        Self::from_event(event, client).await
    }
}

pub struct Secrets(Space);

impl Secrets {
    pub fn new(repo: Space) -> Self {
        Secrets(repo)
    }

    pub async fn set_for_program(
        &self,
        author: Author,
        program_id: Uuid,
        config: SecretsConfig,
    ) -> Result<Secret> {
        let data = serde_json::to_vec(&config)?;
        let value = serde_json::to_value(&config)?;
        let outcome = self.0.router.blobs().add_bytes(data).await?;

        // TODO(b5): wat. why? you're doing something wrong with types.
        let pubkey = PublicKey::from_bytes(author.public_key().as_bytes())?;

        let secret = Secret {
            program_id,
            pubkey,
            created_at: chrono::Utc::now().timestamp(),
            content: HashLink {
                hash: outcome.hash,
                data: Some(value),
            },
            config,
        };
        let event = secret.into_mutate_event(author)?;
        event.write(&self.0.db).await?;
        Ok(secret)
    }

    pub async fn for_program_id(
        &self,
        _author: Author,
        program_id: Uuid,
    ) -> Result<Option<Secret>> {
        let conn = self.0.db.lock().await;
        let mut stmt = conn.prepare(
            format!("SELECT {EVENT_SQL_READ_FIELDS} FROM events WHERE kind = ?1 AND data_id = ?2 ORDER BY created_at DESC LIMIT 1")
                .as_str(),
        )?;
        let mut rows = stmt.query(params![EventKind::MutateSecret, program_id])?;

        if let Some(row) = rows.next()? {
            let secret = Secret::from_sql_row(row, &self.0.router).await?;
            return Ok(Some(secret));
        }
        Ok(None)
    }

    pub async fn list(&self, offset: i64, limit: i64) -> Result<Vec<Secret>> {
        let conn = self.0.db.lock().await;
        let mut stmt = conn.prepare(
            format!(
                "SELECT {EVENT_SQL_READ_FIELDS} FROM events WHERE kind = ?1 LIMIT ?2 OFFSET ?3"
            )
            .as_str(),
        )?;
        let mut rows = stmt.query(params![EventKind::MutateSecret, limit, offset])?;

        let mut users = Vec::new();
        while let Some(row) = rows.next()? {
            let user = Secret::from_sql_row(row, &self.0.router).await?;
            users.push(user);
        }

        Ok(users)
    }
}
