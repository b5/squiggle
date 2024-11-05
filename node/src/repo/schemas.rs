use std::str::FromStr;

use anyhow::Result;
use iroh::blobs::Hash;
use serde::{Deserialize, Serialize};

use super::Repo;

#[derive(Debug, Serialize, Deserialize)]
pub struct Schema {
    // TODO(b5): over time this'll become a JSON schema
    name: String,
}

impl Schema {
    pub fn new(name: String) -> Self {
        Schema { name }
    }

    pub fn id(&self) -> Result<Hash> {
        let res = serde_json::to_string(self).map(|data| Hash::from_str(data.as_str()))??;
        Ok(res)
    }
}

pub struct Schemas(Repo);

impl Schemas {
    pub fn new(repo: Repo) -> Self {
        Schemas(repo)
    }

    pub async fn create(&self, data: &str) -> Result<Hash> {
        let schema = Schema::new(data.to_string());
        let bytes = serde_json::to_vec(&schema)?;

        let res = self.0.router.blobs().add_bytes(bytes).await?;
        let id = schema.id()?;
        assert!(res.hash.eq(&id));

        // TODO - should construct a HashSeq, place the new schema as the 1th element
        // and update the metadata in 0th element
        // schema.write(&self.db).await?;
        schema.id()
    }

    pub async fn update(&self, id: Hash, data: &str) -> Result<Hash> {
        let schema = Schema::new(data.to_string());
        // TODO - should construct a HashSeq, place the new schema as the 1th element
        // and update the metadata in 0th element
        // schema.write(&self.db).await
        schema.id()
    }
}
