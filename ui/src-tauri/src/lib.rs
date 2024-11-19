use std::cmp::Ordering;
use std::str::FromStr;
use std::sync::Arc;

use datalayer_node::node::Node;
use datalayer_node::space::programs::Program;
use datalayer_node::space::rows::Row;
use datalayer_node::space::schemas::Schema;
use datalayer_node::space::users::User;
use datalayer_node::space::SpaceDetails;
use datalayer_node::Hash;
use tauri::{Runtime, LogicalPosition, LogicalSize, WebviewUrl, Listener};
use tauri::webview::PageLoadEvent;

const FRAME_LABEL: &str = "frame";
const WEB_LABEL: &str = "web";
const COZY_LABEL: &str = "cozy";
const CHROME_LABEL: &str = "chrome";

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
        .setup(|app| {
            let width = 1600.;
            let height = 900.;
            let window = tauri::window::WindowBuilder::new(app, "main")
                .inner_size(width, height)
                .build()?;

            window.add_child(
                tauri::webview::WebviewBuilder::new(FRAME_LABEL, WebviewUrl::App("sidebar.html".into()))
                    .transparent(true)
                    .auto_resize(),
                LogicalPosition::new(0., 0.),
                LogicalSize::new(width, height),
            )?;

            let url = tauri::Url::parse("https://iroh.computer")?;
            let page_builder = tauri::webview::WebviewBuilder::new(WEB_LABEL, WebviewUrl::External(url))
                .auto_resize()
                .on_page_load(|_webview, payload| {
                    match payload.event() {
                    PageLoadEvent::Started => {
                        println!("{} started loading", payload.url());
                    }
                    PageLoadEvent::Finished => {
                        println!("{} finished loading", payload.url());
                    }
                    }
                });

            window.add_child(page_builder,
                LogicalPosition::new(12., 12.),
                LogicalSize::new(width - 24., height - 24.),
            )?;

            window.add_child(
                tauri::webview::WebviewBuilder::new(COZY_LABEL, WebviewUrl::App("cozy.html".into()))
                    .auto_resize(),
                LogicalPosition::new(12., 12.),
                LogicalSize::new(width - 24., height - 24.),
            )?;

            window.add_child(
                tauri::webview::WebviewBuilder::new(CHROME_LABEL, WebviewUrl::App("chrome.html".into()))
                    .auto_resize(),
                LogicalPosition::new(0., 0.),
                LogicalSize::new(width, height),
            )?;


            let window2 = window.clone();
            window.listen("dismiss-ui", move |_event| {
                println!("dismissing ui");
                for view in window2.webviews() {
                    if view.label() == CHROME_LABEL {
                        view.hide().expect("failed to hide webview");
                    }
                }
            });

            let window3 = window.clone();
            window.listen("show-ui", move |_event| {
                println!("showing ui");
                for view in window3.webviews() {
                    if view.label() == CHROME_LABEL {
                        view.show().expect("failed to show webview");
                    }
                }
            });

            Ok(())
        })
        .manage(Arc::new(node))
        .invoke_handler(tauri::generate_handler![navigate, spaces_list, users_list, programs_list, schemas_list, schemas_get, rows_query])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
async fn navigate<R: Runtime>(window: tauri::Window<R>, url: &str) -> Result<(), String> {
    if url.starts_with("http://") || url.starts_with("https://") {
        for mut view in window.webviews() {
            match view.label() {
                WEB_LABEL => {
                    let url = tauri::Url::parse(url).map_err(|e| e.to_string())?;
                    view.navigate(url.clone()).map_err(|e| e.to_string())?;
                    view.show().map_err(|e| e.to_string())?;
                }
                COZY_LABEL => {
                    view.hide().map_err(|e| e.to_string())?;
                }
                _ => {}
            }
        }
    } else if url.starts_with("cozy://") {
        for mut view in window.webviews() {
            match view.label() {
                WEB_LABEL => {
                    view.hide().map_err(|e| e.to_string())?;
                }
                COZY_LABEL => {
                    let raw_hash_len = "cozy://".len() + 32;
                    let url = match url.chars().count().cmp(&raw_hash_len) {
                        Ordering::Equal => url.replace("cozy://", "http://localhost:8080/collection/"),
                        // for dev server access
                        _ => url.replace("cozy://", "http://")
                    };
                        
                    println!("navigating to cozy url: {:?}", url);
                    let url = tauri::Url::parse(url.as_str()).map_err(|e| e.to_string())?;
                    view.navigate(url.clone()).map_err(|e| e.to_string())?;
                    view.show().map_err(|e| e.to_string())?;
                }
                _ => {}
            }
        }
    }

    Ok(())
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
async fn programs_list(node: tauri::State<'_, Arc<Node>>, space: &str, offset: i64, limit: i64) -> Result<Vec<Program>, String> {
    let spaces = node.spaces().clone();
    let router = node.router().clone();
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = spaces.get(space).await.ok_or("space not found")?;
            space.programs().list(&router, offset, limit).await.map_err(|e| e.to_string())
        })
    })
}

// #[tauri::command]
// async fn program_run(node: tauri::State<'_, Arc<Node>>, id: &str, environment: HashMap<String, String>) -> Result<TaskOutput, String> {
//     let id = uuid::Uuid::parse_str(id).map_err(|e| e.to_string())?;
//     let node = node.clone();
//     tokio::task::block_in_place(|| {
//         tauri::async_runtime::block_on(async move {
//             let author_id = node.repo().users().authors().await.map_err(|e| e.to_string())?.pop().ok_or("no author").map_err(
//                 |e| e.to_string())?;
//             let author = node.repo().router().authors().export(author_id).await.map_err(|e| e.to_string())?.expect("author to exist");
//             node.vm().run_program(DEFAULT_WORKSPACE, author, id, environment).await.map_err(|e| e.to_string())
//         })
//     })
// }

#[tauri::command]
async fn schemas_list(node: tauri::State<'_, Arc<Node>>, space: &str) -> Result<Vec<Schema>, String> {
    let spaces = node.spaces().clone();
    let router = node.router().clone();
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = spaces.get(space).await.ok_or("space not found")?;
            space.schemas().list(&router, 0, -1).await.map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn schemas_get(node: tauri::State<'_, Arc<Node>>, space: &str, schema: &str) -> Result<Schema, String> {
    let spaces = node.spaces().clone();
    let router = node.router().clone();
    let schema_hash = Hash::from_str(schema).map_err(|e| e.to_string())?;
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = spaces.get(space).await.ok_or("space not found")?;
            space.schemas().get_by_hash(&router, schema_hash).await.map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn rows_query(node: tauri::State<'_, Arc<Node>>, space: &str, schema: &str, offset: i64, limit: i64) -> Result<Vec<Row>, String> {
    let spaces = node.spaces().clone();
    let router = node.router().clone();
    let schema_hash = Hash::from_str(schema).map_err(|e| e.to_string())?;
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            let space = spaces.get(space).await.ok_or("space not found")?;
            space.rows().query(&router, schema_hash, String::from(""), offset, limit).await.map_err(|e| e.to_string())
        })
    })
}