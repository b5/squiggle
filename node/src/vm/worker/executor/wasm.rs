use std::path::PathBuf;

use anyhow::{Context, Result};
use extism::*;
use tracing::debug;

use crate::router::RouterClient;
use crate::vm::blobs::Blobs;

use super::Executor;

#[derive(derive_more::Debug, Clone)]
pub struct WasmExecutor {
    router: RouterClient,
    blobs: Blobs,
    /// Root folder to store shared files in
    root: PathBuf,
}

impl WasmExecutor {
    pub async fn new(router: RouterClient, blobs: Blobs, root: PathBuf) -> Result<Self> {
        Ok(WasmExecutor {
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
        let downloads_path = ctx.downloads_path(&self.root);
        let uploads_path = ctx.uploads_path(&self.root);
        tokio::fs::create_dir_all(&downloads_path).await?;
        tokio::fs::create_dir_all(&uploads_path).await?;

        debug!("downloading artifacts to {}", downloads_path.display());
        ctx.write_downloads(&downloads_path, &self.blobs, &self.router)
            .await
            .context("write downloads")?;

        let path = Wasm::file(downloads_path.join(&job.module));

        let paths = vec![
            ("downloads".to_string(), downloads_path.clone()),
            ("uploads".to_string(), uploads_path.clone()),
        ]
        .into_iter();
        let manifest = Manifest::new([path])
            .with_allowed_host("*")
            .with_allowed_paths(paths);
        let wasm_context = UserData::new(WasmContext {
            router: self.router.clone(),
        });
        let builder = PluginBuilder::new(manifest).with_wasi(true);
        let mut plugin = add_host_functions(builder, wasm_context).build()?;

        let output = plugin.call::<&str, &str>("main", "hello")?;

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
    pub module: String,
}

#[derive(Debug)]
pub struct Report {
    pub output: String,
}

struct WasmContext {
    router: RouterClient,
}

host_fn!(iroh_blob_get_ticket(_user_data: WasmContext; _ticket: &str) -> Vec<u8> {
    // let ctx = user_data.get()?;
    // let ctx = ctx.lock().unwrap();

    // let (node_addr, hash, format) = iroh::base::ticket::BlobTicket::from_str(ticket).map_err(|_| anyhow!("invalid ticket"))?.into_parts();

    // if format != iroh::blobs::BlobFormat::Raw {
    //     return Err(anyhow!("can only get raw bytes for now, not HashSequences (collections)"));
    // }
    // let client = ctx.iroh.client();
    // let buf = ctx.rt.block_on(async move {
    //     let mut stream = client.blobs().download_with_opts(hash, iroh::client::blobs::DownloadOptions {
    //         format,
    //         nodes: vec![node_addr],
    //         mode: iroh::client::blobs::DownloadMode::Queued,
    //         tag: SetTagOption::Auto,
    //     }).await?;
    //     while stream.next().await.is_some() {}

    //     let buffer = client.blobs().read(hash).await?.read_to_bytes().await?;
    //     anyhow::Ok(buffer.to_vec())
    // })?;

    // Ok(buf)
    Ok(vec![])
});

fn add_host_functions(
    builder: PluginBuilder,
    wasm_context: UserData<WasmContext>,
) -> PluginBuilder {
    builder.with_function(
        "iroh_blob_get_ticket",
        [PTR],
        [PTR],
        wasm_context,
        iroh_blob_get_ticket,
    )
}
