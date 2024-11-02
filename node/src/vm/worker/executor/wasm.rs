use std::path::PathBuf;

use anyhow::{Context, Result};
use tracing::debug;
use wasi_common::sync::{ambient_authority, Dir};
use wasmtime::{Config, Engine, Linker, Module, Store};
use wasmtime_wasi::pipe::MemoryOutputPipe;
use wasmtime_wasi::{preview1, DirPerms, FilePerms};
use wasmtime_wasi::{WasiCtxBuilder, WasiP1Ctx};

use crate::vm::{blobs::Blobs, node::IrohNodeClient};

use super::Executor;

#[derive(derive_more::Debug, Clone)]
pub struct Wasm {
    node: IrohNodeClient,
    blobs: Blobs,
    /// Root folder to store shared files in
    root: PathBuf,
    #[debug("wasmtime::Engine")]
    engine: Engine,
}

impl Wasm {
    pub async fn new(node: IrohNodeClient, blobs: Blobs, root: PathBuf) -> Result<Self> {
        // Construct the wasm engine with async support enabled.
        let mut config = Config::new();
        config.async_support(true);
        let engine = Engine::new(&config)?;

        Ok(Wasm {
            node,
            blobs,
            root,
            engine,
        })
    }
}

impl Executor for Wasm {
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
        ctx.write_downloads(&downloads_path, &self.blobs, &self.node)
            .await
            .context("write downloads")?;

        // TODO: figure out if there is a more efficient way to reuse the store & linker

        let mut linker: Linker<WasiP1Ctx> = Linker::new(&self.engine);
        preview1::add_to_linker_async(&mut linker, |t| t)?;

        let stdout = MemoryOutputPipe::new(1024 * 1024);
        let stderr = MemoryOutputPipe::new(1024 * 1024);
        let wasi_ctx = WasiCtxBuilder::new()
            .stdout(stdout.clone())
            .stderr(stderr.clone())
            .preopened_dir(
                Dir::open_ambient_dir(&uploads_path, ambient_authority())?,
                DirPerms::MUTATE,
                FilePerms::WRITE,
                "uploads",
            )
            .preopened_dir(
                Dir::open_ambient_dir(&downloads_path, ambient_authority())?,
                DirPerms::MUTATE,
                FilePerms::WRITE,
                "downloads",
            )
            .build_p1();
        let mut store = Store::new(&self.engine, wasi_ctx);

        // Note: This is a module built against the preview1 WASI API.
        let mod_path = downloads_path.join(&job.module);
        debug!("loading module from {}", mod_path.display());
        let module = Module::from_file(&self.engine, mod_path)?;
        let func = linker
            .module_async(&mut store, "", &module)
            .await?
            .get_default(&mut store, "")?
            .typed::<(), ()>(&store)?;

        // Invoke the WASI program default function.
        func.call_async(&mut store, ()).await?;

        debug!("uploading artifacts from {}", uploads_path.display());
        ctx.read_uploads(&uploads_path, &self.blobs, &self.node)
            .await
            .context("read uploads")?;

        Ok(Report {
            stdout: String::from_utf8_lossy(&stdout.contents()).into(),
            stderr: String::from_utf8_lossy(&stderr.contents()).into(),
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
    pub stdout: String,
    pub stderr: String,
}
