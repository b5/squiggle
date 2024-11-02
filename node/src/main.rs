use anyhow::Result;

use datalayer_node::node::Node;
use datalayer_node::vm::DEFAULT_WORKSPACE;

#[tokio::main]
async fn main() -> Result<()> {
    let path = std::path::PathBuf::from("test");
    let node = Node::open(path).await?;
    // let events = node.repo().list_events().await?;
    // let b5 = node
    //     .repo()
    //     .users()
    //     .create(
    //         String::from("b5"),
    //         String::from("some nerd from canada"),
    //         String::from(""),
    //     )
    //     .await?;
    println!("Current working directory: {:?}", std::env::current_dir()?);
    let flow = datalayer_node::vm::flow::Flow::load("./tests/wasm.toml").await?;
    node.vm().run(DEFAULT_WORKSPACE, flow).await?;
    // println!(
    //     "Hello, world: {:?} events: {:?} users: {:?}",
    //     node.name,
    //     events,
    //     // b5,
    //     node.repo().users().list().await?
    // );
    Ok(())
}
