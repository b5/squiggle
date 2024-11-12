use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub enum Actions {
    TableRead,
    TableWrite,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Capability {
    action: Vec<Actions>,
    subject: String,
    resource: String,
}
