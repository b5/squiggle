use std::str::FromStr;
use std::sync::Arc;

use datalayer_node::node::Node;
use datalayer_node::repo::users::User;
use datalayer_node::repo::events::Event;
use datalayer_node::repo::schemas::Schema;
use datalayer_node::vm::flow::{Flow, FlowOutput};
use datalayer_node::Hash;

#[tauri::command]
async fn accounts_list(
    node: tauri::State<'_, Arc<Node>>,
) -> Result<Vec<User>, String> {
    let users = node.repo().users().list().await.map_err(|e| e.to_string())?;
    Ok(users)
}

#[tauri::command]
async fn schemas_list(node: tauri::State<'_, Arc<Node>>) -> Result<Vec<Schema>, String> {
    let node = node.clone();
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            node.repo().schemas().list(0, -1).await.map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn events_query(node: tauri::State<'_, Arc<Node>>, schema: &str, offset: i64, limit: i64) -> Result<Vec<Event>, String> {
    let node = node.clone();
    let schema_hash = Hash::from_str(schema).map_err(|e| e.to_string())?;
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            node.repo().events().query(schema_hash, String::from(""), offset, limit).await.map_err(|e| e.to_string())
        })
    })
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
        let path = std::path::PathBuf::from("../../node/test");
        let node = datalayer_node::node::Node::open(path)
            .await
            .expect("failed to build datalayer");
        node
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(Arc::new(node))
        .invoke_handler(tauri::generate_handler![accounts_list, schemas_list, events_query, run_flow])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
