use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use datalayer_node::node::Node;
use datalayer_node::space::programs::Program;
use datalayer_node::space::rows::Row;
use datalayer_node::space::schemas::Schema;
use datalayer_node::space::users::User;
use datalayer_node::space::events::Event;
use datalayer_node::space::SpaceDetails;
use datalayer_node::vm::flow::TaskOutput;
use datalayer_node::Hash;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let node = tauri::async_runtime::block_on(async move {
        let path = datalayer_node::node::data_root().unwrap();
        let node = datalayer_node::node::Node::open(path)
            .await
            .expect("failed to build datalayer");
        // TODO - capture & cleanup task handle
        node.gateway("127.0.0.1:8080")
            .await
            .expect("failed to start gateway");
        node
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(Arc::new(node))
        .invoke_handler(tauri::generate_handler![spaces_list, events_search, users_list, programs_list, program_run, program_get, schemas_list, schemas_get, rows_query])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
async fn users_list(
    node: tauri::State<'_, Arc<Node>>,
    space: &str,
    offset: i64,
    limit: i64,
) -> Result<Vec<User>, String> {
    let spaces = node.spaces().clone();
    let router = node.router().clone();
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = spaces.get(space).await.ok_or("space not found")?;
            space.users().list(&router, offset, limit).await.map_err(|e| e.to_string())
        })
    })      
}

#[tauri::command]
async fn spaces_list(
    node: tauri::State<'_, Arc<Node>>,
    offset: i64,
    limit: i64,
) -> Result<Vec<SpaceDetails>, String> {
    let node = node.clone();
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            node.spaces().list(offset, limit).await.map_err(|e| e.to_string())
        })
    })      
}

#[tauri::command]
async fn events_search(
    node: tauri::State<'_, Arc<Node>>,
    space: &str,
    query: &str,
    offset: i64,
    limit: i64,
) -> Result<Vec<Event>, String> {
    let node = node.clone();
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = node.spaces().get(space).await.ok_or("space not found")?;
            space.search(query, offset, limit).await.map_err(|e| e.to_string())
        })
    })      
}

#[tauri::command]
async fn programs_list(node: tauri::State<'_, Arc<Node>>, space: &str, offset: i64, limit: i64) -> Result<Vec<Program>, String> {
    let spaces = node.spaces().clone();
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = spaces.get(space).await.ok_or("space not found")?;
            space.programs().list(offset, limit).await.map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn program_get(node: tauri::State<'_, Arc<Node>>, space: &str, program_id: &str) -> Result<Program, String> {
    let program_id = uuid::Uuid::parse_str(program_id).map_err(|e| e.to_string())?;
    let spaces = node.spaces().clone();
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = spaces.get(space).await.ok_or("space not found")?;
            space.programs().get_by_id(program_id).await.map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn program_run(node: tauri::State<'_, Arc<Node>>, space: &str, _author: &str, program_id: &str, environment: HashMap<String, String>) -> Result<TaskOutput, String> {
    let program_id = uuid::Uuid::parse_str(program_id).map_err(|e| e.to_string())?;
    let spaces = node.spaces().clone();
    let node = node.clone();
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = spaces.get(space).await.ok_or("space not found")?;
            let author_id = node.accounts().await.map_err(|e| e.to_string())?.pop().ok_or("no author").map_err(
                |e| e.to_string())?;
            let author = node.router().authors().export(author_id).await.map_err(|e| e.to_string())?.ok_or("no author").map_err(
                |e| e.to_string())?;
            node.vm().run_program(&space, author, program_id, environment).await.map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn schemas_list(node: tauri::State<'_, Arc<Node>>, space: &str) -> Result<Vec<Schema>, String> {
    let spaces = node.spaces().clone();
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = spaces.get(space).await.ok_or("space not found")?;
            space.schemas().list(0, -1).await.map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn schemas_get(node: tauri::State<'_, Arc<Node>>, space: &str, schema: &str) -> Result<Schema, String> {
    let spaces = node.spaces().clone();
    let schema_hash = Hash::from_str(schema).map_err(|e| e.to_string())?;
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = spaces.get(space).await.ok_or("space not found")?;
            space.schemas().get_by_hash(schema_hash).await.map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn rows_query(node: tauri::State<'_, Arc<Node>>, space: &str, schema: &str, offset: i64, limit: i64) -> Result<Vec<Row>, String> {
    let spaces = node.spaces().clone();
    let schema_hash = Hash::from_str(schema).map_err(|e| e.to_string())?;
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = spaces.get(space).await.ok_or("space not found")?;
            space.rows().query(schema_hash, String::from(""), offset, limit).await.map_err(|e| e.to_string())
        })
    })
}