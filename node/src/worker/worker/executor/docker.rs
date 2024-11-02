use std::path::PathBuf;

use anyhow::{Context, Result};
use bollard::container::LogOutput;
use futures::StreamExt;
use tracing::{debug, info};

use crate::{
    blobs::Blobs,
    docker::{delete_container, get_docker, pull_docker_image, stop_container},
    job::JobContext,
    node::IrohNodeClient,
};

use super::Executor;

#[derive(Debug, Clone)]
pub struct Docker {
    docker: bollard::Docker,
    node: IrohNodeClient,
    blobs: Blobs,
    /// Root folder to store shared files in
    root: PathBuf,
}

impl Docker {
    pub async fn new(node: IrohNodeClient, blobs: Blobs, root: PathBuf) -> Result<Self> {
        let docker = get_docker().await?;
        tokio::fs::create_dir_all(&root).await?;
        let root = root.canonicalize()?;

        Ok(Self {
            node,
            docker,
            blobs,
            root,
        })
    }
}

impl Executor for Docker {
    type Job = Job;
    type Report = Report;

    async fn execute(&self, ctx: &JobContext, job: Self::Job) -> Result<Self::Report> {
        let downloads_path = ctx.downloads_path(&self.root);
        let uploads_path = ctx.uploads_path(&self.root);

        debug!("downloading artifacts to {}", downloads_path.display());
        ctx.write_downloads(&downloads_path, &self.blobs, &self.node)
            .await?;

        // TODO: parallelize with artifact writing
        debug!("pulling image {}", job.image);
        pull_docker_image(&self.docker, &job.image)
            .await
            .context("pull image")?;

        let container_name = ctx.job_scope("docker");
        debug!("creating container: {}", container_name);

        // Setup volumen bindings
        let binds = vec![
            format!("{}:/downloads", downloads_path.to_string_lossy()),
            format!("{}:/uploads", uploads_path.to_string_lossy()),
        ];

        let host_config = bollard::models::HostConfig {
            binds: Some(binds),
            ..Default::default()
        };

        let config = bollard::container::Config {
            image: Some(job.image.clone()),
            tty: Some(false),
            host_config: Some(host_config),
            cmd: Some(job.command.clone()),
            ..Default::default()
        };

        let container_options = bollard::container::CreateContainerOptions {
            name: &container_name,
            platform: None,
        };

        let id = self
            .docker
            .create_container(Some(container_options), config)
            .await?
            .id;
        self.docker
            .start_container::<String>(&id, None)
            .await
            .context("start container")?;

        let mut wait_result = self.docker.wait_container(
            &id,
            Some(bollard::container::WaitContainerOptions {
                condition: "not-running",
            }),
        );

        debug!("waiting for container to exit");
        let mut code = 0;
        while let Some(response) = wait_result.next().await {
            info!("docker wait: {:?}", response);
            match response {
                Ok(res) => {
                    code = res.status_code;
                }
                Err(bollard::errors::Error::DockerContainerWaitError { code: c, .. }) => {
                    code = c;
                }
                _ => {}
            }
        }

        debug!("collecting logs");
        let mut logs = self.docker.logs(
            &id,
            Some(bollard::container::LogsOptions::<String> {
                stdout: true,
                stderr: true,
                ..Default::default()
            }),
        );

        let mut stdout = String::new();
        let mut stderr = String::new();

        while let Some(Ok(msg)) = logs.next().await {
            match msg {
                LogOutput::StdErr { message } => {
                    let message = String::from_utf8_lossy(&message);
                    info!("[docker:stderr] {}", message);
                    stderr.push_str(&message);
                }
                LogOutput::StdOut { message } => {
                    let message = String::from_utf8_lossy(&message);
                    info!("[docker:stdout] {}", message);
                    stdout.push_str(&message);
                }
                LogOutput::Console { message } => {
                    info!("[docker:console] {}", String::from_utf8_lossy(&message));
                }
                LogOutput::StdIn { message } => {
                    info!("[docker:stdin] {}", String::from_utf8_lossy(&message));
                }
            }
        }

        debug!("uploading artifacts from {}", uploads_path.display());
        // TODO: parallelize the with container stopping
        ctx.read_uploads(&uploads_path, &self.blobs, &self.node)
            .await?;

        debug!("stopping container");
        stop_container(&self.docker, &container_name).await?;
        delete_container(&self.docker, &container_name).await?;

        Ok(Report {
            code,
            stdout,
            stderr,
        })
    }
}

#[derive(Debug)]
pub struct Job {
    pub image: String,
    pub command: Vec<String>,
}

#[derive(Debug)]
pub struct Report {
    pub code: i64,
    pub stdout: String,
    pub stderr: String,
}
