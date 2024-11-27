use anyhow::Result;
use serde::{Deserialize, Serialize};

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

    pub(crate) async fn caps_for_user(&self, _user: &User) -> Result<CapSet> {
        // TODO - implement
        Ok(CapSet(vec![Capability {
            action: vec![Actions::All],
            subject: "TODO".to_string(),
            resource: "TODO".to_string(),
        }]))
    }
}
