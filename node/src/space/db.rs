use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use rusqlite::Connection;
use tokio::sync::Mutex;

pub(crate) type DB = Arc<Mutex<Connection>>;

pub(crate) async fn open_db(path: impl Into<PathBuf>) -> Result<DB> {
    let db = Connection::open(path.into())?;
    Ok(Arc::new(Mutex::new(db)))
}

pub(crate) async fn setup_db(db: &DB) -> Result<()> {
    let conn = db.lock().await;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS events (
            id           BLOB PRIMARY KEY,
            pubkey       TEXT NOT NULL,
            created_at   INTEGER NOT NULL,
            kind         INTEGER NOT NULL,
            schema_hash  TEXT,
            data_id      BLOB NOT NULL,
            sig          BLOB NOT NULL,
            content_hash TEXT NOT NULL,
            content      BLOB
        )",
        [],
    )?;

    // a list of capabilities, either from others or self-issued
    // A capability is the association of an ability to a subject: subject x command x policy.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS capabilities (
        iss   TEXT NOT NULL,    -- Issuer: key of the sender granting the capability
        aud   TEXT NOT NULL,    -- Principal: what this capability is about (eg: a program)
        sub   TEXT NOT NULL,    -- Audience: receiver of the capability: a user or a program
        cmd   TEXT NOT NULL,    -- Command (e.g. 'read', 'write', 'follow', 'mute', 'block')
        pol   TEXT NOT NULL,    -- Policy: refinements on the command
        nonce TEXT NOT NULL,    -- Unique nonce to prevent replay attacks
        exp   INTEGER,          -- Expiration UTC Unix Timestamp in seconds (valid until)
        nbf   INTEGER,          -- 'Not before' UTC Unix Timestamp in seconds (valid from)
        sig   BLOB              -- Signature of the capability
    )",
        [],
    )?;

    Ok(())
}
