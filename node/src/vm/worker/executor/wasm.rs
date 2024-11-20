use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{anyhow, Context, Ok, Result};
use extism::*;
use iroh::blobs::Hash;
use iroh::docs::Author;
use tracing::debug;
use uuid::Uuid;

use crate::router::RouterClient;
use crate::space::{Space, Spaces};
use crate::vm::blobs::Blobs;
use crate::vm::job::Source;

use super::Executor;

const MAIN_FUNC_NAME: &str = "main";

#[derive(derive_more::Debug, Clone)]
pub struct WasmExecutor {
    spaces: Spaces,
    router: RouterClient,
    blobs: Blobs,
    /// Root folder to store shared files in
    root: PathBuf,
}

impl WasmExecutor {
    pub async fn new(
        spaces: Spaces,
        router: RouterClient,
        blobs: Blobs,
        root: PathBuf,
    ) -> Result<Self> {
        Ok(WasmExecutor {
            spaces,
            router,
            blobs,
            root,
        })
    }
}

impl Executor for WasmExecutor {
    type Job = Job;
    type Report = Report;

    async fn execute(
        &self,
        ctx: &crate::vm::job::JobContext,
        job: Self::Job,
    ) -> Result<Self::Report> {
        let space = self
            .spaces
            .get_by_name(&ctx.space)
            .await
            .ok_or_else(|| anyhow!("can't find space: {}", ctx.space))?;
        debug!("executing job: {:?}. context: {:?}", job, ctx.id);
        let downloads_path = ctx.downloads_path(&self.root);
        let uploads_path = ctx.uploads_path(&self.root);
        tokio::fs::create_dir_all(&downloads_path).await?;
        tokio::fs::create_dir_all(&uploads_path).await?;

        println!("downloading artifacts to {}", downloads_path.display());
        ctx.write_downloads(&downloads_path, &self.blobs, &self.router)
            .await
            .context("write downloads")?;

        let program = match job.module {
            Source::LocalBlob(hash) => {
                let result = self.router.blobs().read_to_bytes(hash).await?;
                Wasm::data(result)
            }
            Source::LocalPath(path) => Wasm::file(downloads_path.join(&path)),
        };
        let mut environment = ctx.environment.clone();

        let space2 = space.clone();
        let stored_secrets = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let stored_secrets = space2
                    .secrets()
                    .for_program_id(ctx.author.clone(), ctx.program_id)
                    .await?;
                Ok(stored_secrets)
            })
        })?;

        println!("stored secrets: {:?}", stored_secrets);
        if let Some(secrets) = stored_secrets {
            for (key, value) in secrets.config {
                environment.insert(key, value);
            }
        }

        let manifest = Manifest::new([program])
            .with_allowed_host("*")
            .with_config(environment.into_iter());

        let wasm_context = UserData::new(WasmContext {
            author: ctx.author.clone(),
            rt: tokio::runtime::Handle::current(),
            space: space.clone(),
            output: String::new(),
        });
        let mut plugin = PluginBuilder::new(manifest)
            .with_wasi(true)
            .with_function("print", [PTR], [], wasm_context.clone(), print)
            .with_function("sleep", [ValType::I64], [], wasm_context.clone(), sleep)
            .with_function(
                "schema_load_or_create",
                [PTR],
                [PTR],
                wasm_context.clone(),
                schema_load_or_create,
            )
            .with_function(
                "event_create",
                [PTR, PTR],
                [PTR],
                wasm_context.clone(),
                event_create,
            )
            .with_function(
                "event_mutate",
                [PTR, PTR, PTR],
                [PTR],
                wasm_context.clone(),
                event_mutate,
            )
            .with_function("event_query", [PTR, PTR], [PTR], wasm_context, event_query)
            .build()?;

        let output = plugin.call::<_, &str>(MAIN_FUNC_NAME, ())?;

        debug!("uploading artifacts from {}", uploads_path.display());
        ctx.read_uploads(&uploads_path, &self.blobs, &self.router)
            .await
            .context("read uploads")?;

        Ok(Report {
            output: output.to_string(),
        })
    }
}

#[derive(Debug)]
pub struct Job {
    /// Module file path
    pub module: Source,
}

#[derive(Debug)]
pub struct Report {
    pub output: String,
}

struct WasmContext {
    rt: tokio::runtime::Handle,
    author: Author,
    space: Space,
    output: String,
}

host_fn!(print(ctx: WasmContext; msg: String) -> () {
    let ctx = ctx.get()?;
    let mut ctx = ctx.lock().unwrap();
    ctx.output = ctx.output.to_owned() + &msg;
    println!("{}", msg);
    Ok(())
});

host_fn!(sleep(ctx: WasmContext; ms: u64) -> () {
    let ctx = ctx.get()?;
    let ctx = ctx.lock().unwrap();
    ctx.rt.block_on(tokio::time::sleep(std::time::Duration::from_millis(ms)));
    Ok(())
});

host_fn!(schema_load_or_create(ctx: WasmContext; data: String) -> Vec<u8> {
    let ctx = ctx.get()?;
    let ctx = ctx.lock().unwrap();
    let schemas = ctx.space.tables();
    let author = ctx.author.clone();

    tokio::task::block_in_place(|| {
        ctx.rt.block_on(async move {
            let schema = schemas.load_or_create(author, data.into()).await.context("failed to load or create schema")?;
            serde_json::to_vec(&schema).context("failed to serialize schema")
        })
    })
});

host_fn!(event_create(ctx: WasmContext; schema: String, data: String) -> Vec<u8> {
    let ctx = ctx.get()?;
    let ctx = ctx.lock().unwrap();
    let schema_hash = Hash::from_str(schema.as_str()).context("invalid schema hash")?;
    let author = ctx.author.clone();
    let space = ctx.space.clone();
    let parsed = serde_json::from_str::<serde_json::Value>(&data).context("parsing JSON")?;

    tokio::task::block_in_place(|| {
        ctx.rt.block_on(async move {
            let mut schema = space.tables().get_by_hash(schema_hash).await.context("loading schema")?;
            let row = schema.create_row(&space, author, parsed).await.context("failed to created row")?;
            serde_json::to_vec(&row).context("failed to serialize event")
        })
    })
});

host_fn!(event_mutate(ctx: WasmContext; schema: String, id: String, data: String) -> Vec<u8> {
    let ctx = ctx.get()?;
    let ctx = ctx.lock().unwrap();


    let schema = Hash::from_str(schema.as_str()).map_err(|_| anyhow!("invalid schema hash"))?;
    let id = Uuid::parse_str(id.clone().as_str()).map_err(|_| anyhow!("invalid id"))?;
    let author = ctx.author.clone();
    let rows = ctx.space.rows();

    tokio::task::block_in_place(|| {
        ctx.rt.block_on(async move {
            let data = serde_json::from_str::<serde_json::Value>(data.as_str()).map_err(|e| anyhow!("failed to parse data: {}", e))?;
            let event = rows.mutate(author, schema, id, data).await?;
            let data = serde_json::to_vec(&event).map_err(|e| anyhow!("failed to serialize event: {}", e))?;
            data.to_bytes()
        })
    })
});

host_fn!(event_query(ctx: WasmContext; schema: String, query: String) -> Vec<u8> {
    let ctx = ctx.get()?;
    let ctx = ctx.lock().unwrap();

    let schema = Hash::from_str(schema.as_str()).map_err(|_| anyhow!("invalid schema hash"))?;
    let rows = ctx.space.rows().clone();

    tokio::task::block_in_place(|| {
        ctx.rt.block_on(async move {
            let res = rows.query(schema, query, 0, -1).await?;
            let data = serde_json::to_vec(&res).map_err(|e| anyhow!("failed to serialize events: {}", e))?;
            data.to_bytes()
        })
    })
});

// host_fn!(iroh_blob_get_ticket(_user_data: WasmContext; _ticket: &str) -> Vec<u8> {
//     // let ctx = user_data.get()?;
//     // let ctx = ctx.lock().unwrap();

//     // let (node_addr, hash, format) = iroh::base::ticket::BlobTicket::from_str(ticket).map_err(|_| anyhow!("invalid ticket"))?.into_parts();

//     // if format != iroh::blobs::BlobFormat::Raw {
//     //     return Err(anyhow!("can only get raw bytes for now, not HashSequences (collections)"));
//     // }
//     // let client = ctx.iroh.client();
//     // let buf = ctx.rt.block_on(async move {
//     //     let mut stream = client.blobs().download_with_opts(hash, iroh::client::blobs::DownloadOptions {
//     //         format,
//     //         nodes: vec![node_addr],
//     //         mode: iroh::client::blobs::DownloadMode::Queued,
//     //         tag: SetTagOption::Auto,
//     //     }).await?;
//     //     while stream.next().await.is_some() {}

//     //     let buffer = client.blobs().read(hash).await?.read_to_bytes().await?;
//     //     anyhow::Ok(buffer.to_vec())
//     // })?;

//     // Ok(buf)
//     Ok(vec![])
// });
