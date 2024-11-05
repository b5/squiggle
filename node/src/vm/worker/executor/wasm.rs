use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{anyhow, Context, Ok, Result};
use bollard::auth;
use extism::*;
use iroh::blobs::Hash;
use iroh::docs::Author;
use iroh::net::key::PublicKey;
use tracing::debug;
use uuid::Uuid;

use crate::repo::Repo;
use crate::vm::blobs::Blobs;

use super::Executor;

#[derive(derive_more::Debug, Clone)]
pub struct WasmExecutor {
    repo: Repo,
    blobs: Blobs,
    /// Root folder to store shared files in
    root: PathBuf,
}

impl WasmExecutor {
    pub async fn new(repo: Repo, blobs: Blobs, root: PathBuf) -> Result<Self> {
        Ok(WasmExecutor { repo, blobs, root })
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
        let downloads_path = ctx.downloads_path(&self.root);
        let uploads_path = ctx.uploads_path(&self.root);
        tokio::fs::create_dir_all(&downloads_path).await?;
        tokio::fs::create_dir_all(&uploads_path).await?;

        println!("downloading artifacts to {}", downloads_path.display());
        ctx.write_downloads(&downloads_path, &self.blobs, self.repo.router())
            .await
            .context("write downloads")?;

        let path = Wasm::file(downloads_path.join(&job.module));

        // let paths = vec![
        //     ("downloads".to_string(), downloads_path.clone()),
        //     ("uploads".to_string(), uploads_path.clone()),
        // ]
        // .into_iter();
        let env = ctx.environment.clone().into_iter();

        let manifest = Manifest::new([path])
            .with_allowed_host("*")
            .with_config(env);
        // .with_allowed_paths(paths);

        let wasm_context = UserData::new(WasmContext {
            author: ctx.author.clone(),
            rt: tokio::runtime::Handle::current(),
            repo: self.repo.clone(),
        });
        let mut plugin = PluginBuilder::new(manifest)
            .with_wasi(true)
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
                wasm_context,
                event_mutate,
            )
            .build()?;

        let output = plugin.call::<&str, &str>("main", "hello")?;

        println!("uploading artifacts from {}", uploads_path.display());
        ctx.read_uploads(&uploads_path, &self.blobs, self.repo.router())
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
    pub module: String,
}

#[derive(Debug)]
pub struct Report {
    pub output: String,
}

struct WasmContext {
    rt: tokio::runtime::Handle,
    author: Author,
    repo: Repo,
}

host_fn!(event_create(ctx: WasmContext; schema: String, data: String) -> Vec<u8> {
    let ctx = ctx.get()?;
    let ctx = ctx.lock().unwrap();

    let schema = Hash::from_str(schema.as_str()).map_err(|_| anyhow!("invalid schema hash"))?;
    let author = ctx.author.clone();
    let events = ctx.repo.events();

    let e = tokio::task::block_in_place(|| {
        ctx.rt.block_on(async move {
            let event = events.create(author, schema, data).await.unwrap();
            let data = serde_json::to_vec(&event).map_err(|e| anyhow!("failed to serialize event: {}", e)).unwrap();
            println!("event_create! {:?}", event);
            data
        })
    });

    Ok(e)
});

host_fn!(event_mutate(ctx: WasmContext; schema: String, id: String, data: String) -> Vec<u8> {
    let ctx = ctx.get()?;
    let ctx = ctx.lock().unwrap();


    let schema = Hash::from_str(schema.as_str()).map_err(|_| anyhow!("invalid schema hash"))?;
    let id = Uuid::parse_str(id.clone().as_str()).map_err(|_| anyhow!("invalid id"))?;
    let author = ctx.author.clone();
    let events = ctx.repo.events();

    let res = ctx.rt.block_on(async move {
        let event = events.mutate(author, schema, id, data).await?;
        let data = serde_json::to_vec(&event).map_err(|e| anyhow!("failed to serialize event: {}", e))?;
        println!("mutate! {:?}", schema);
        data.to_bytes()
    })?;

    Ok(res)
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

fn add_host_functions(
    builder: PluginBuilder,
    wasm_context: UserData<WasmContext>,
) -> PluginBuilder {
    builder.with_function(
        "event_mutate",
        [PTR, PTR],
        [PTR],
        wasm_context,
        event_mutate,
    )
    // .with_function(
    //     "iroh_blob_get_ticket",
    //     [PTR],
    //     [PTR],
    //     wasm_context,
    //     iroh_blob_get_ticket,
    // )
}
