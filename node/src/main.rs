use std::collections::HashMap;

use anyhow::Result;

use datalayer_node::node::Node;
use datalayer_node::repo::programs::Manifest;
use datalayer_node::vm::DEFAULT_WORKSPACE;

#[tokio::main]
async fn main() -> Result<()> {
    let path = datalayer_node::node::data_root()?;
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
    let author = node
        .repo()
        .router()
        .authors()
        .export(authors[0])
        .await?
        .expect("author to exist");

    // running a flow from a file:
    // let mut flow =
    //     datalayer_node::vm::flow::Flow::load("../programs/github_repo_stargazers/stargazers.toml")
    //         .await?;
    // flow.tasks
    //     .iter_mut()
    //     .for_each(|task| task.description.author = authors[0].to_string());
    // let res = node.vm().run_flow(DEFAULT_WORKSPACE, flow).await?;

    // importing a program & running:
    let file = tokio::fs::read("../programs/github_repo_stargazers/dist/program.json").await?;
    let manifest: Manifest = serde_json::from_slice(file.as_slice())?;

    let program = match node.repo().programs().get_by_name(manifest.name).await {
        Ok(program) => program,
        Err(_) => {
            node.repo()
                .programs()
                .create(author.clone(), "../programs/github_repo_stargazers/dist")
                .await?
        }
    };

    let mut env = HashMap::new();
    env.insert("org".to_string(), "n0-computer".to_string());
    env.insert("repo".to_string(), "awesome-iroh".to_string());
    env.insert("github_token".to_string(), "TOKEN_HERE".to_string());

    let res = node
        .vm()
        .run_program(DEFAULT_WORKSPACE, author, program.id, env)
        .await?;
    println!("Flow output: {:?}", res);
    Ok(())
}
