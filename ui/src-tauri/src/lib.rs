use std::sync::Arc;

use datalayer_node::node::Node;
use datalayer_node::repo::users::User;
use datalayer_node::repo::schemas::Schema;
use datalayer_node::vm::flow::{Flow, FlowOutput};

#[tauri::command]
async fn accounts_list(
    node: tauri::State<'_, Arc<Node>>,
) -> Result<Vec<User>, String> {
    let users = node.repo().users().list().await.map_err(|e| e.to_string())?;
    Ok(users)
}

#[tauri::command]
async fn schemas_list(node: tauri::State<'_, Arc<Node>>) -> Result<Vec<Schema>, String> {
    let schemas = node.repo().schemas().list(0, 1000).await.map_err(|e| e.to_string())?;
    Ok(schemas)
}

#[tauri::command]
async fn run_flow(node: tauri::State<'_, Arc<Node>>, path: &str) -> Result<FlowOutput, String> {
    println!("Current working directory: {:?} running flow: {:?}", std::env::current_dir().unwrap(), path);
    let flow = Flow::load(path).await.map_err(|e| e.to_string())?;
    let output = node
        .vm()
        .run("default", flow)
        .await
        .map_err(|e| e.to_string())?;
  Ok(output)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let node = tauri::async_runtime::block_on(async move {
        let path = std::path::PathBuf::from("../test");
        let node = datalayer_node::node::Node::open(path)
            .await
            .expect("failed to build datalayer");
        node
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(Arc::new(node))
        .invoke_handler(tauri::generate_handler![accounts_list, schemas_list, run_flow])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
