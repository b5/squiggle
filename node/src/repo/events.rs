use anyhow::Result;
use rusqlite::params;
use rusqlite::types::FromSqlError;
use serde::{Deserialize, Serialize};

use super::db::DB;

#[derive(Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub pubkey: String,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    pub kind: u32,
    pub content: String,
    pub tags: Vec<(String, String)>,
    pub sig: String,
}

impl Event {
    pub(crate) async fn create(db: &DB, event: Event) -> Result<()> {
        let conn = db.lock().await;
        conn.execute(
            "INSERT INTO events (id, pubkey, created_at, kind, content, tags, sig) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event.id,
                event.pubkey,
                event.created_at,
                event.kind,
                event.content,
                serde_json::to_string(&event.tags)?,
                event.sig,
            ],
        )?;
        Ok(())
    }
}

pub(crate) async fn list(db: &DB) -> Result<Vec<Event>> {
    let conn = db.lock().await;
    let mut stmt =
        conn.prepare("SELECT id, pubkey, created_at, kind, content, tags, sig FROM events")?;
    let rows = stmt.query_map([], |row| {
        Ok(Event {
            id: row.get(0)?,
            pubkey: row.get(1)?,
            created_at: row.get(2)?,
            kind: row.get(3)?,
            content: row.get(4)?,
            tags: serde_json::from_str(row.get::<_, String>(5)?.as_str())
                .map_err(|e| FromSqlError::Other(Box::new(e)))?,
            sig: row.get(6)?,
        })
    })?;

    let mut events = Vec::new();
    for event in rows {
        events.push(event?);
    }

    Ok(events)
}
