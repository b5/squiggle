use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use squiggle_node::node::Node;
use squiggle_node::space::events::Event;
use squiggle_node::space::programs::Program;
use squiggle_node::space::rows::Row;
use squiggle_node::space::secrets::Secret;
use squiggle_node::space::tables::Table;
use squiggle_node::space::users::User;
use squiggle_node::space::SpaceDetails;
use squiggle_node::vm::flow::TaskOutput;
use squiggle_node::Hash;
use uuid::Uuid;

mod app_state;

use crate::app_state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let path = squiggle_node::node::data_root().unwrap();

    let path2 = path.clone();
    let (node, state) = tauri::async_runtime::block_on(async move {
        let node = squiggle_node::node::Node::open(path2)
            .await
            .expect("failed to build datalayer");
        // TODO - capture & cleanup task handle
        node.gateway("127.0.0.1:8080")
            .await
            .expect("failed to start gateway");

        let state = AppState::open_or_create(path, &node)
            .await
            .expect("failed to open app state");

        (node, state)
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(Arc::new(state))
        .manage(Arc::new(node))
        .invoke_handler(tauri::generate_handler![
            spaces_list,
            current_space,
            current_space_set,
            events_search,
            users_list,
            programs_list,
            program_run,
            program_get,
            secrets_get,
            secrets_set,
            tables_list,
            table_get,
            rows_query
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
async fn users_list(
    node: tauri::State<'_, Arc<Node>>,
    space_id: Uuid,
    offset: i64,
    limit: i64,
) -> Result<Vec<User>, String> {
    let spaces = node.spaces().clone();
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = spaces.get(&space_id).await.ok_or("space not found")?;
            space
                .users()
                .list(offset, limit)
                .await
                .map_err(|e| e.to_string())
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
            node.spaces()
                .list(offset, limit)
                .await
                .map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn current_space(
    state: tauri::State<'_, Arc<AppState>>,
    node: tauri::State<'_, Arc<Node>>,
) -> Result<SpaceDetails, String> {
    let state = state.clone();
    let node = node.clone();
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = node
                .spaces()
                .get(&state.current_space_id)
                .await
                .ok_or("space not found")?;
            Ok(space.details())
        })
    })
}

#[tauri::command]
async fn current_space_set(
    state: tauri::State<'_, Arc<AppState>>,
    node: tauri::State<'_, Arc<Node>>,
    space_id: Uuid,
) -> Result<SpaceDetails, String> {
    let _state = state.clone();
    let node = node.clone();

    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = node
                .spaces()
                .get(&space_id)
                .await
                .ok_or("space not found")?;
            // state.current_space_id = space_id;
            Ok(space.details())
        })
    })
}

#[tauri::command]
async fn events_search(
    node: tauri::State<'_, Arc<Node>>,
    space_id: Uuid,
    query: &str,
    offset: i64,
    limit: i64,
) -> Result<Vec<Event>, String> {
    let node = node.clone();
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = node
                .spaces()
                .get(&space_id)
                .await
                .ok_or("space not found")?;
            space
                .search(query, offset, limit)
                .await
                .map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn programs_list(
    node: tauri::State<'_, Arc<Node>>,
    space_id: Uuid,
    offset: i64,
    limit: i64,
) -> Result<Vec<Program>, String> {
    let spaces = node.spaces().clone();
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = spaces.get(&space_id).await.ok_or("space not found")?;
            space
                .programs()
                .list(offset, limit)
                .await
                .map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn program_get(
    node: tauri::State<'_, Arc<Node>>,
    space_id: Uuid,
    program_id: Uuid,
) -> Result<Program, String> {
    let spaces = node.spaces().clone();
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = spaces.get(&space_id).await.ok_or("space not found")?;
            space
                .programs()
                .get_by_id(program_id)
                .await
                .map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn secrets_get(
    node: tauri::State<'_, Arc<Node>>,
    space_id: Uuid,
    program_id: Uuid,
) -> Result<HashMap<String, String>, String> {
    let spaces = node.spaces().clone();
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = spaces.get(&space_id).await.ok_or("space not found")?;
            let secrets = space
                .secrets()
                .for_program_id(program_id)
                .await
                .map_err(|e| e.to_string())?
                .map(|s| s.config)
                .unwrap_or_default()
                .into_keys()
                .map(|k| (k, "********".to_string()))
                .collect();
            Ok(secrets)
        })
    })
}

#[tauri::command]
async fn secrets_set(
    node: tauri::State<'_, Arc<Node>>,
    space_id: Uuid,
    program_id: Uuid,
    secrets: HashMap<String, String>,
) -> Result<Secret, String> {
    let spaces = node.spaces().clone();
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let user = node
                .accounts()
                .current()
                .await
                .ok_or_else(|| "user not found")?;
            let author = user.author.ok_or_else(|| "author not found".to_string())?;

            let space = spaces.get(&space_id).await.ok_or("space not found")?;
            space
                .secrets()
                .set_for_program_id(author, program_id, secrets)
                .await
                .map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn program_run(
    node: tauri::State<'_, Arc<Node>>,
    space_id: Uuid,
    _author: &str,
    program_id: Uuid,
    environment: HashMap<String, String>,
) -> Result<TaskOutput, String> {
    let spaces = node.spaces().clone();
    let node = node.clone();
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = spaces.get(&space_id).await.ok_or("space not found")?;
            let user = node
                .accounts()
                .current()
                .await
                .ok_or_else(|| "user not found".to_string())?;
            let author = user.author.ok_or_else(|| "author not found".to_string())?;
            node.vm()
                .run_program(&space, author, program_id, environment)
                .await
                .map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn tables_list(
    node: tauri::State<'_, Arc<Node>>,
    space_id: Uuid,
) -> Result<Vec<Table>, String> {
    let spaces = node.spaces().clone();
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = spaces.get(&space_id).await.ok_or("space not found")?;
            space.tables().list(0, -1).await.map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn table_get(
    node: tauri::State<'_, Arc<Node>>,
    space_id: Uuid,
    table: &str,
) -> Result<Table, String> {
    let spaces = node.spaces().clone();
    let table_hash = Hash::from_str(table).map_err(|e| e.to_string())?;
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = spaces.get(&space_id).await.ok_or("space not found")?;
            space
                .tables()
                .get_by_hash(table_hash)
                .await
                .map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn rows_query(
    node: tauri::State<'_, Arc<Node>>,
    space_id: Uuid,
    table: &str,
    offset: i64,
    limit: i64,
) -> Result<Vec<Row>, String> {
    let spaces = node.spaces().clone();
    let table_hash = Hash::from_str(table).map_err(|e| e.to_string())?;
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = spaces.get(&space_id).await.ok_or("space not found")?;
            space
                .rows()
                .query(table_hash, String::from(""), offset, limit)
                .await
                .map_err(|e| e.to_string())
        })
    })
}
