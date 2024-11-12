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
    let authors = node.repo().users().authors().await?;

    let mut flow =
        datalayer_node::vm::flow::Flow::load("../bots/github_repo_stargazers/stargazers.toml")
            .await?;
    flow.tasks
        .iter_mut()
        .for_each(|task| task.description.author = authors[0].to_string());

    let res = node.vm().run(DEFAULT_WORKSPACE, flow).await?;
    println!("Flow output: {:?}", res);
    // println!(
    //     "Hello, world: {:?} events: {:?} users: {:?}",
    //     node.name,
    //     events,
    //     // b5,
    //     node.repo().users().list().await?
    // );
    Ok(())
}
