use anyhow::Result;

use datalayer::node::Node;

#[tokio::main]
async fn main() -> Result<()> {
    let path = std::path::PathBuf::from("test");
    let node = Node::open(path).await?;
    let events = node.repo().list_events().await?;
    println!("Hello, world: {:?} events: {:?}", node.name, events);
    Ok(())
}
