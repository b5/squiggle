use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::debug;

use super::users::User;
use super::Space;

#[derive(Debug, Deserialize, Serialize)]
pub enum Actions {
    All,
    TableRead,
    TableWrite,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Capability {
    action: Vec<Actions>,
    // TODO - improve type, define enumeration
    subject: String,
    // TODO - improve type, define enumeration
    resource: String,
}

#[derive(Debug)]
pub struct CapSet(Vec<Capability>);

impl CapSet {
    pub fn iter(&self) -> impl Iterator<Item = &Capability> {
        self.0.iter()
    }
}

pub struct Capabilities(Space);

impl Capabilities {
    pub(crate) fn new(s: Space) -> Self {
        Self(s)
    }

    pub(crate) async fn caps_for_user(&self, user: &User) -> Result<CapSet> {
        let caps = self.read_caps(user).await?;
        debug!("caps for user {:?}: {:?}", user, caps);

        // TODO - implement
        Ok(CapSet(vec![Capability {
            action: vec![Actions::All],
            subject: "TODO".to_string(),
            resource: "TODO".to_string(),
        }]))
    }

    // TODO(b5) - unfinished
    async fn read_caps(&self, user: &User) -> Result<CapSet> {
        let conn = self.0.db().lock().await;
        let mut stmt = conn.prepare("SELECT * from capabilities WHERE aud = ?")?;
        let mut res = stmt.query(params![user.pubkey.as_bytes()])?;
        let mut caps: CapSet = CapSet(Vec::new());

        while let Some(row) = res.next()? {
            let cap = Capability {
                action: vec![Actions::All],
                subject: row.get(1)?,
                resource: row.get(2)?,
            };
            caps.0.push(cap);
        }

        Ok(caps)
    }
}
