use anyhow::Result;
use std::path::PathBuf;

pub type Router = iroh::node::FsNode;
pub type RouterClient = iroh::client::Iroh;

pub async fn router(path: impl Into<PathBuf>) -> Result<Router> {
    let path = path.into();
    let router = iroh::node::Node::persistent(path)
        .await?
        .enable_docs()
        .spawn()
        .await?;
    Ok(router)
}
