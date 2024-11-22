use anyhow::{Context, Result};

use squiggle_node::node::Node;
// use squiggle_node::space::programs::Manifest;

#[tokio::main]
async fn main() -> Result<()> {
    let path = squiggle_node::node::data_root()?;
    let node = Node::open(path).await.context("opening node")?;

    let accounts = node.accounts().list(0, -1).await?;
    let user = accounts.first().expect("user to exist");
    // let author = user.author.clone().expect("author to exist");

    let space = node
        .spaces()
        .clone()
        .get_or_create(node.router(), user, "personal", "my first space")
        .await?;

    // importing a program & running:
    // let file = tokio::fs::read("../programs/github_repo_stargazers/dist/program.json").await?;
    // let manifest: Manifest = serde_json::from_slice(file.as_slice())?;

    // let program = match space.programs().get_by_name(manifest.name).await {
    //     Ok(program) => program,
    //     Err(_) => {
    //         space
    //             .programs()
    //             .create(author.clone(), "../programs/github_repo_stargazers/dist")
    //             .await?
    //     }
    // };

    // let mut env = HashMap::new();
    // env.insert("org".to_string(), "n0-computer".to_string());
    // env.insert("repo".to_string(), "awesome-iroh".to_string());
    // env.insert("github_token".to_string(), "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".to_string());

    // space
    //     .secrets()
    //     .set_for_program_id(author.clone(), program.id, env)
    //     .await?;

    // let res = node
    //     .vm()
    //     .run_program(&space, author, program.id, HashMap::new())
    //     .await?;
    // println!("Flow output: {:?}", res);

    let ticket = space.share().await?;
    println!("Space ticket: {}", ticket);

    // let node_clone = node.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for event");
        println!("Ctrl+C received, shutting down.");
        // node_clone
        //     .shutdown()
        //     .await
        //     .expect("failed to shut down node");
    });

    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for event");
    println!("Ctrl+C rec`eived, shutting down.");
    // node.shutdown().await.expect("failed to shut down node");
    Ok(())
}
