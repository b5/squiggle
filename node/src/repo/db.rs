use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use rusqlite::Connection;
use tokio::sync::Mutex;

pub(crate) type DB = Arc<Mutex<Connection>>;

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

pub(crate) async fn open_db(path: impl Into<PathBuf>) -> Result<DB> {
    let db = Connection::open(path.into())?;
    Ok(Arc::new(Mutex::new(db)))
}

pub(crate) async fn setup_db(db: &DB) -> Result<()> {
    let conn = db.lock().await;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS events (
            id TEXT PRIMARY KEY,
            pubkey TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            kind INTEGER NOT NULL,
            schema TEXT NOT NULL,
            data_id TEXT NOT NULL,
            content TEXT NOT NULL,
            sig TEXT NOT NULL
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
        sig   TEXT              -- Signature of the capability
    )",
        [],
    )?;

    // conn.execute(
    //     "CREATE TABLE IF NOT EXISTS users (
    //         pubkey BLOB NOT NULL,
    //         privkey BLOB,
    //         name TEXT,
    //         about TEXT,
    //         picture TEXT
    //     )",
    //     [],
    // )?;

    // conn.execute(
    //     "CREATE TABLE IF NOT EXISTS data (
    //         schema TEXT NOT NULL,
    //         id TEXT PRIMARY KEY NOT NULL,
    //         data TEXT NOT NULL
    // )",
    //     [],
    // )?;

    Ok(())
}
