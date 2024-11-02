use anyhow::Result;

use datalayer::node::Node;

#[tokio::main]
async fn main() -> Result<()> {
    let path = std::path::PathBuf::from("test");
    let node = Node::open(path).await?;
    let events = node.repo().list_events().await?;
    // let b5 = node
    //     .repo()
    //     .users()
    //     .create(
    //         String::from("b5"),
    //         String::from("some nerd from canada"),
    //         String::from(""),
    //     )
    //     .await?;
    println!(
        "Hello, world: {:?} events: {:?} users: {:?}",
        node.name,
        events,
        // b5,
        node.repo().users().list().await?
    );
    Ok(())
}
