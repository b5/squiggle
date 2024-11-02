use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{ensure, Result};
use bytes::Bytes;
use futures::future::BoxFuture;
use futures::FutureExt;
use iroh::blobs::util::SetTagOption;
use serde::{Deserialize, Serialize};
use tokio::io::BufReader;
use tokio::task::JoinSet;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

use crate::blobs::Blobs;
use crate::job::{JobDescription, JobNameContext, JobResult, JobResultStatus};
use crate::metrics::Metrics;
use crate::node::IrohNodeClient;
use crate::scheduler::Scheduler;
use crate::workspace::Workspace;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Flow {
    pub name: String,
    pub tasks: Vec<Task>,
    /// Uploads that are added into the system from the scheduler.
    #[serde(default)]
    pub uploads: Vec<Upload>,
    /// Artifacts that are downloaded onto the scheduler at the end.
    #[serde(default)]
    pub downloads: Vec<Download>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Upload {
    /// The name in the system, will be available in jobs under `{scope}/<name>`.
    pub name: String,
    /// The source
    pub source: UploadSource,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Download {
    /// The name in the system from which to fetch.
    pub name: String,
    /// The place to download to.
    pub path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum UploadSource {
    #[serde(rename = "file")]
    File {
        /// The path from which to upload.
        path: String,
    },
    #[serde(rename = "inline")]
    Inline {
        /// The content of the file
        content: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct FlowOutput {
    /// The name of this flow.
    pub name: String,
    /// The assigned Uuid of this flow.
    pub id: Uuid,
    /// Output of all tasks
    pub tasks: Vec<TaskOutput>,
    /// Downloads from the flow
    pub downloads: Vec<Download>,
}

impl Flow {
    #[instrument(skip_all, fields(flow_name = %self.name))]
    pub async fn run(self, node: &IrohNodeClient, workspace: &Workspace) -> Result<FlowOutput> {
        iroh_metrics::inc!(Metrics, flow_run_started);
        let scope = Uuid::new_v4();

        // Upload inputs
        for upload in &self.uploads {
            debug!("uploading {}", upload.name);
            let res = match &upload.source {
                UploadSource::File { path } => {
                    let file_path = PathBuf::from(path);
                    ensure!(
                        file_path.exists() && file_path.is_file(),
                        "unknown file: {}",
                        file_path.display()
                    );

                    let file = tokio::fs::File::open(file_path).await?;
                    let file = BufReader::new(file);
                    node.blobs()
                        .add_reader(file, SetTagOption::Auto)
                        .await?
                        .await?
                }
                UploadSource::Inline { content } => node.blobs().add_bytes(content.clone()).await?,
            };
            let name = format!("{}/{}", scope.as_simple(), upload.name);
            workspace
                .blobs()
                .put_object(&name, res.hash, res.size)
                .await?;
        }

        let mut out = Vec::new();
        for task in self.tasks.into_iter() {
            let job_id = Uuid::new_v4();
            let i = task
                .run(
                    scope,
                    node,
                    workspace.scheduler().clone(),
                    workspace.blobs().clone(),
                    job_id,
                )
                .await;
            out.extend(i);
        }

        iroh_metrics::inc!(Metrics, flow_run_completed);

        let ctx = JobNameContext { scope };

        let mut downloads = Vec::new();
        for download in self.downloads {
            let path = PathBuf::from(&download.path);
            let name = ctx.render(&download.name)?;
            debug!("downloading {} to {}", name, path.display());
            let data = workspace.blobs().get_object(&name).await?;
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(path, data).await?;
            downloads.push(download);
        }

        Ok(FlowOutput {
            name: self.name,
            id: scope,
            tasks: out,
            downloads,
        })
    }

    /// Check that invariants are upheld
    pub fn validate(&self) -> Result<()> {
        let mut job_names = HashSet::new();

        // job names must be unique per flow
        let mut task_list = vec![&self.tasks[..]];
        while let Some(tasks) = task_list.pop() {
            for task in tasks {
                if !job_names.insert(&task.description.name) {
                    anyhow::bail!("duplicate job name: {}", task.description.name);
                }
                task_list.push(&task.tasks);
            }
        }

        // job names must not be overlap with uploads
        for upload in &self.uploads {
            if !job_names.insert(&upload.name) {
                anyhow::bail!("upload name conflicts with job name: {}", upload.name);
            }
        }

        Ok(())
    }
}

impl FlowOutput {
    /// Helper function to generate the name of an artifact.
    pub fn artifact_name(&self, job_name: &str, artifact_name: &str) -> String {
        format!("{}/{}/{}", self.id.as_simple(), job_name, artifact_name)
    }

    /// Get a generated artifact.
    pub async fn get_artifact(
        &self,
        ws: &Workspace,
        job_name: &str,
        artifact_name: &str,
    ) -> Result<Bytes> {
        let name = self.artifact_name(job_name, artifact_name);
        ws.blobs().get_object(&name).await
    }
}

impl std::fmt::Display for Flow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = toml::to_string_pretty(self).unwrap();
        f.write_str(&s)
    }
}

/// from_str assumes a TOML string
impl std::str::FromStr for Flow {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parsed: Flow = toml::from_str(s)?;
        parsed.validate()?;
        Ok(parsed)
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Task {
    #[serde(default)]
    tasks: Vec<Task>,
    description: JobDescription,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskOutput {
    pub name: String,
    /// The assigned id of this job
    pub id: Uuid,
    pub result: JobResult,
}

impl Task {
    #[instrument(skip_all, fields(task_name = %self.description.name))]
    pub fn run(
        self,
        scope: Uuid,
        node: &IrohNodeClient,
        scheduler: Scheduler,
        blobs: Blobs,
        job_id: Uuid,
    ) -> BoxFuture<'static, Vec<TaskOutput>> {
        let mut set = JoinSet::default();
        let mut meta = HashMap::new();

        iroh_metrics::inc!(Metrics, task_run_started);

        for task in self.tasks.into_iter() {
            let n2 = node.clone();
            let s2 = scheduler.clone();
            let b2 = blobs.clone();
            let job_id = Uuid::new_v4();
            let job_name = task.description.name.clone();
            let handle = set.spawn(async move { task.run(scope, &n2, s2, b2, job_id).await });
            meta.insert(handle.id(), (job_name, job_id));
        }

        let description = self.description.clone();
        let job_name = description.name.clone();

        let sched = scheduler.clone();
        let execute_job = async move {
            // Wait for dependencies to be available
            let job_name_ctx = JobNameContext { scope };
            let mut deps: HashSet<String> = description
                .dependencies(job_name_ctx)
                .collect::<Result<_>>()?;
            let job_name = description.name.clone();

            loop {
                // TODO: avoid polling
                let mut found_deps = Vec::new();
                for dep in &deps {
                    info!("looking for dependency: {}", dep);
                    if blobs.has_object(dep).await? {
                        found_deps.push(dep.clone());
                        info!("found dependency: {}", dep);
                    }
                }

                for dep in found_deps {
                    deps.remove(&dep);
                }
                if deps.is_empty() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            // run principle job
            let timeout = description.timeout.try_into()?;

            let res = tokio::time::timeout(timeout, async {
                let result = sched.run_job_and_wait(scope, job_id, description).await;

                let result = result?;
                anyhow::Ok(TaskOutput {
                    name: job_name,
                    id: job_id,
                    result,
                })
            })
            .await;
            anyhow::Ok(res)
        };

        let sched = scheduler.clone();
        let handle = set.spawn(async move {
            let out = match execute_job.await {
                Ok(Ok(Ok(job))) => job,
                Ok(Ok(Err(err))) => TaskOutput {
                    name: job_name,
                    id: job_id,
                    result: JobResult {
                        worker: None,
                        status: JobResultStatus::Err(err.to_string()),
                    },
                },
                Ok(Err(_)) => {
                    if let Err(err) = sched.cancel_job(job_id).await {
                        warn!("failed to cancel job: {:?}", err);
                    }
                    // timeout
                    TaskOutput {
                        name: job_name,
                        id: job_id,
                        result: JobResult {
                            worker: None,
                            status: JobResultStatus::ErrTimeout,
                        },
                    }
                }
                Err(err) => TaskOutput {
                    name: job_name,
                    id: job_id,
                    result: JobResult {
                        worker: None,
                        status: JobResultStatus::Err(err.to_string()),
                    },
                },
            };
            vec![out]
        });
        meta.insert(handle.id(), (self.description.name.clone(), job_id));

        (async move {
            let mut task_ids = Vec::new();
            while let Some(res) = set.join_next_with_id().await {
                match res {
                    Ok((_id, outputs)) => {
                        task_ids.extend(outputs);
                    }
                    Err(err) => {
                        let id = err.id();
                        let (job_name, job_id) = meta.remove(&id).expect("invalid state");
                        task_ids.push(TaskOutput {
                            name: job_name,
                            id: job_id,
                            result: JobResult {
                                worker: None,
                                status: JobResultStatus::Err(err.to_string()),
                            },
                        })
                    }
                }
            }
            task_ids
        })
        .boxed()
    }

    #[allow(dead_code)]
    pub(crate) fn dependencies(&self, ctx: &JobNameContext) -> HashSet<String> {
        let mut deps = HashSet::new();

        // own dependencies
        for dep in self.description.dependencies(ctx.clone()) {
            deps.insert(dep.unwrap());
        }

        // sub dependencies
        let mut tasks = vec![&self.tasks];
        while let Some(t) = tasks.pop() {
            for task in t {
                for dep in task.description.dependencies(ctx.clone()) {
                    deps.insert(dep.unwrap());
                }
                tasks.push(&task.tasks);
            }
        }

        deps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        job::{Artifact, Artifacts, JobDetails, JobOutput, JobStatus, DEFAULT_TIMEOUT},
        node::node_author_id,
        test_utils::{create_nodes, setup_logging},
    };

    #[test]
    fn test_flow_parse() {
        let f = Flow {
            name: "test".into(),
            downloads: Vec::new(),
            uploads: vec![Upload {
                name: "foo".into(),
                source: UploadSource::File {
                    path: "foo.txt".into(),
                },
            }],
            tasks: vec![Task {
                description: JobDescription {
                    name: "job".into(),
                    details: JobDetails::Wasm {
                        module: "foo.wasm".into(),
                    },
                    artifacts: Default::default(),
                    timeout: DEFAULT_TIMEOUT,
                },
                tasks: vec![Task {
                    description: JobDescription {
                        name: "job-nested".into(),
                        details: JobDetails::Docker {
                            image: "docker-image".into(),
                            command: vec!["ls".into()],
                        },
                        artifacts: Default::default(),
                        timeout: DEFAULT_TIMEOUT,
                    },
                    tasks: Vec::new(),
                }],
            }],
        };

        println!("{}", f);
        let f2: Flow = f.to_string().parse().unwrap();
        assert_eq!(f, f2);
    }

    #[ignore]
    #[tokio::test]
    async fn test_flow_docker_timeout() -> Result<()> {
        if std::env::var("RUN_DOCKER_TESTS").is_err() {
            println!("Skipping Docker test because RUN_DOCKER_TESTS is not set");
            return Ok(());
        }

        setup_logging();
        let flow = r#"
            name = "flow1"

            # First Task
            [[tasks]]

            [tasks.description]
            name = "job1"
            timeout = "1.0" # in seconds
            [tasks.description.details.docker]
            image = "alpine:latest"
            command = ["sleep", "3"]

            # Second Task
            [[tasks]]

            # Details for the second task
            [tasks.description]
            name = "job2"
            timeout = "5.0"
            [tasks.description.details.docker]
            image = "alpine:3"
            command = ["sleep", "1"]
        "#;

        let flow: Flow = flow.parse().unwrap();

        let dir = tempfile::tempdir().unwrap();
        // still need 2 nodes, one to schedule, one to work
        let nodes = create_nodes(&dir, 2).await.unwrap();

        let ws = &nodes[0].1;
        let flow_res = flow.run(&nodes[0].0, ws).await.unwrap();
        let task_res = &flow_res.tasks;
        assert_eq!(task_res.len(), 2);

        assert_eq!(task_res[0].result.status, JobResultStatus::ErrTimeout);

        assert!(task_res[1].result.worker.is_some());
        assert_eq!(
            task_res[1].result.status,
            JobResultStatus::Ok(JobOutput::Docker {
                code: 0,
                stdout: Default::default(),
                stderr: Default::default(),
            })
        );

        let status = ws.scheduler().get_job_status(task_res[0].id).await?;
        assert_eq!(
            status.unwrap(),
            JobStatus::Completed(node_author_id(&nodes[0].0.node_id()))
        );

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_flow_docker_job() -> Result<()> {
        if std::env::var("RUN_DOCKER_TESTS").is_err() {
            println!("Skipping Docker test because RUN_DOCKER_TESTS is not set");
            return Ok(());
        }

        setup_logging();

        let dir = tempfile::tempdir().unwrap();
        // still need 2 nodes, one to schedule, one to work
        let nodes = create_nodes(&dir, 2).await.unwrap();
        let (node, ws) = &nodes[0];

        let flow: Flow = r#"
            name = "flow1"

            [[uploads]]
            name = "hello"
            [uploads.source.inline]
            content = "hello world!"

            # First Task
            [[tasks]]

            [tasks.description]
            name = "job1"

            [[tasks.description.artifacts.downloads]]
            path = "my_blob.txt"
            name = "{scope}/hello"

            [[tasks.description.artifacts.uploads]]
            path = "blob_back.txt"
            name = "blob_back"

            [tasks.description.details.docker]
            image = "alpine:3"
            command = ["cp", "/downloads/my_blob.txt", "/uploads/blob_back.txt"]
        "#
        .parse()
        .unwrap();

        dbg!(&flow);
        let flow_res = flow.run(node, ws).await.unwrap();
        let task_res = &flow_res.tasks;
        assert_eq!(task_res.len(), 1);
        let task = &task_res[0];
        assert_eq!(task.name, "job1");
        assert_eq!(
            task.result.status,
            JobResultStatus::Ok(JobOutput::Docker {
                code: 0,
                stdout: "".into(),
                stderr: Default::default(),
            })
        );

        let blob_back = flow_res.get_artifact(ws, &task.name, "blob_back").await?;
        assert_eq!(blob_back, b"hello world!".as_ref());

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_flow_docker_folders_job() -> Result<()> {
        if std::env::var("RUN_DOCKER_TESTS").is_err() {
            println!("Skipping Docker test because RUN_DOCKER_TESTS is not set");
            return Ok(());
        }

        setup_logging();

        let dir = tempfile::tempdir().unwrap();
        // still need 2 nodes, one to schedule, one to work
        let nodes = create_nodes(&dir, 2).await.unwrap();
        let (node, ws) = &nodes[0];

        let flow: Flow = r#"
            name = "flow1"

            [[uploads]]
            name = "hello"
            [uploads.source.inline]
            content = "hello world!"

            [[uploads]]
            name = "foo"
            [uploads.source.inline]
            content = "hello foo!"

            # First Task
            [[tasks]]

            [tasks.description]
            name = "job1"

            [[tasks.description.artifacts.downloads]]
            path = "hello.txt"
            name = "{scope}/hello"

            [[tasks.description.artifacts.downloads]]
            path = "foo.txt"
            name = "{scope}/foo"

            [[tasks.description.artifacts.uploads]]
            path = "files"
            name = "files"

            [tasks.description.details.docker]
            image = "alpine:3"
            command = [
              "bin/sh", "-c",
              "mkdir /uploads/files && cp /downloads/*.txt /uploads/files/"
            ]
        "#
        .parse()
        .unwrap();

        dbg!(&flow);
        let flow_res = flow.run(node, ws).await.unwrap();
        let task_res = &flow_res.tasks;
        assert_eq!(task_res.len(), 1);
        let task = &task_res[0];
        assert_eq!(task.name, "job1");
        assert!(task.result.worker.is_some());
        assert_eq!(
            task.result.status,
            JobResultStatus::Ok(JobOutput::Docker {
                code: 0,
                stdout: "".into(),
                stderr: Default::default(),
            })
        );

        let blob_back = flow_res
            .get_artifact(ws, &task.name, "files/hello.txt")
            .await?;
        assert_eq!(blob_back, b"hello world!".as_ref());
        let blob_back = flow_res
            .get_artifact(ws, &task.name, "files/foo.txt")
            .await?;
        assert_eq!(blob_back, b"hello foo!".as_ref());

        Ok(())
    }

    #[tokio::test]
    async fn test_flow_wasm_simple_job() -> Result<()> {
        setup_logging();

        let dir = tempfile::tempdir().unwrap();
        // still need 2 nodes, one to schedule, one to work
        let nodes = create_nodes(&dir, 2).await.unwrap();
        let (node, ws) = &nodes[0];

        let flow: Flow = r#"
            name = "flow1"

            [[uploads]]
            name = "min.wat"
            [uploads.source.file]
            path = "./tests/min.wat"

            [[tasks]]
            [tasks.description]
            name = "wasm-run"

            [[tasks.description.artifacts.downloads]]
            path = "min.wat"
            name = "{scope}/min.wat"

            [tasks.description.details.wasm]
            module = "min.wat"
        "#
        .parse()
        .unwrap();

        dbg!(&flow);
        let flow_res = flow.run(node, ws).await.unwrap();
        let task_res = &flow_res.tasks;
        assert_eq!(task_res.len(), 1);

        let task = &task_res[0];
        assert_eq!(task.name, "wasm-run");
        assert!(task.result.worker.is_some());
        assert_eq!(
            task.result.status,
            JobResultStatus::Ok(JobOutput::Wasm {
                stdout: "hello world\n".into(),
                stderr: Default::default(),
            })
        );

        Ok(())
    }

    #[ignore]
    #[tokio::test]
    async fn test_flow_docker_fanout_job() -> Result<()> {
        if std::env::var("RUN_DOCKER_TESTS").is_err() {
            println!("Skipping Docker test because RUN_DOCKER_TESTS is not set");
            return Ok(());
        }

        setup_logging();

        let flow = r#"
            name = "flow-fanout"

            [[tasks]]

            [tasks.description]
            name = "job1"

            [[tasks.description.artifacts.downloads]]
            path = "job.sh"
            executable = true
            name = "job"

            [[tasks.description.artifacts.uploads]]
            path = "out.txt"
            name = "out.txt"

            [tasks.description.details.docker]
            image = "alpine:3"
            command = ["/bin/sh", "-c", "/downloads/job.sh 1"]

            [[tasks]]

            [tasks.description]
            name = "job2"

            [[tasks.description.artifacts.downloads]]
            path = "job.sh"
            name = "job"
            executable = true

            [[tasks.description.artifacts.uploads]]
            path = "out.txt"
            name = "out.txt"

            [tasks.description.details.docker]
            image = "alpine:3"
            command = ["/bin/sh", "-c", "/downloads/job.sh 2"]
        "#;

        let flow: Flow = flow.parse().unwrap();
        dbg!(&flow);

        let dir = tempfile::tempdir().unwrap();
        let nodes = create_nodes(&dir, 3).await?;
        let (node, ws) = &nodes[0];

        // add content
        let job_sh = r#"
        #!/bin/sh

        sleep 2
        echo "good job $1" > /uploads/out.txt
        echo "done"
        "#;
        let _ = ws.blobs().put_bytes("job", job_sh).await?;

        let flow_res = flow.run(node, ws).await.unwrap();
        let task_res = &flow_res.tasks;
        assert_eq!(task_res.len(), 2);

        for (i, task) in task_res.iter().enumerate() {
            assert_eq!(task.name, format!("job{}", i + 1));
            assert!(task.result.worker.is_some());
            assert_eq!(
                task.result.status,
                JobResultStatus::Ok(JobOutput::Docker {
                    code: 0,
                    stdout: "done\n".into(),
                    stderr: Default::default(),
                })
            );

            let blob_back = flow_res.get_artifact(ws, &task.name, "out.txt").await?;
            assert_eq!(blob_back, format!("good job {}\n", i + 1));
        }
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_flow_docker_dependencies_job() -> Result<()> {
        if std::env::var("RUN_DOCKER_TESTS").is_err() {
            println!("Skipping Docker test because RUN_DOCKER_TESTS is not set");
            return Ok(());
        }

        setup_logging();

        let flow = r#"
            name = "flow-deps"

            [[tasks]]

            [tasks.description]
            name = "job1"

            [[tasks.description.artifacts.downloads]]
            path = "job.sh"
            executable = true
            name = "job"

            [[tasks.description.artifacts.uploads]]
            path = "out.txt"
            name = "out.txt"

            [tasks.description.details.docker]
            image = "alpine:3"
            command = ["/bin/sh", "-c", "/downloads/job.sh 1"]

            [[tasks]]
            [tasks.description]
            name = "job2"

            [[tasks.description.artifacts.downloads]]
            path = "job.sh"
            name = "job"
            executable = true

            [[tasks.description.artifacts.uploads]]
            path = "out.txt"
            name = "out.txt"

            [tasks.description.details.docker]
            image = "alpine:3"
            command = ["/bin/sh", "-c", "/downloads/job.sh 2"]

            [[tasks]]
            [tasks.description]
            name = "job3"

            [[tasks.description.artifacts.downloads]]
            path = "out1.txt"
            name = "{scope}/job1/out.txt"

            [[tasks.description.artifacts.downloads]]
            path = "out2.txt"
            name = "{scope}/job2/out.txt"

            [[tasks.description.artifacts.uploads]]
            path = "out-final.txt"
            name = "out-final.txt"

            [tasks.description.details.docker]
            image = "alpine:3"
            command = ["/bin/sh", "-c", "cat /downloads/out1.txt /downloads/out2.txt > /uploads/out-final.txt"]
        "#;

        let flow: Flow = flow.parse().unwrap();
        dbg!(&flow);

        let dir = tempfile::tempdir().unwrap();
        let nodes = create_nodes(&dir, 3).await?;
        let (node, ws) = &nodes[0];

        // add content
        let job_sh = r#"
        #!/bin/sh

        sleep 2
        echo "good job $1" > /uploads/out.txt
        echo "done"
        "#;
        let _ = ws.blobs().put_bytes("job", job_sh).await?;

        let flow_res = flow.run(node, ws).await.unwrap();
        let task_res = &flow_res.tasks;
        assert_eq!(task_res.len(), 3);

        // Task 1
        {
            let task = &task_res[0];
            assert_eq!(task.name, "job1");
            assert!(task.result.worker.is_some());
            assert_eq!(
                task.result.status,
                JobResultStatus::Ok(JobOutput::Docker {
                    code: 0,
                    stdout: "done\n".into(),
                    stderr: Default::default(),
                })
            );

            let blob_back = flow_res.get_artifact(ws, &task.name, "out.txt").await?;
            assert_eq!(blob_back, "good job 1\n");
        }

        // Task 2
        {
            let task = &task_res[1];
            assert_eq!(task.name, "job2");
            assert!(task.result.worker.is_some());
            assert_eq!(
                task.result.status,
                JobResultStatus::Ok(JobOutput::Docker {
                    code: 0,
                    stdout: "done\n".into(),
                    stderr: Default::default(),
                })
            );

            let blob_back = flow_res.get_artifact(ws, &task.name, "out.txt").await?;
            assert_eq!(blob_back, "good job 2\n");
        }

        // Task 3
        {
            let task = &task_res[2];
            assert_eq!(task.name, "job3");
            assert!(task.result.worker.is_some());
            assert_eq!(
                task.result.status,
                JobResultStatus::Ok(JobOutput::Docker {
                    code: 0,
                    stdout: "".into(),
                    stderr: Default::default(),
                })
            );

            let blob_back = flow_res
                .get_artifact(ws, &task.name, "out-final.txt")
                .await?;
            assert_eq!(blob_back, "good job 1\ngood job 2\n");
        }

        Ok(())
    }

    #[test]
    fn test_flow_validate() {
        let flow = Flow {
            name: "flow".into(),
            uploads: Vec::new(),
            downloads: Vec::new(),
            tasks: vec![
                Task {
                    description: JobDescription {
                        name: "job-1".into(),
                        details: JobDetails::Wasm {
                            module: "me.wasm".into(),
                        },
                        artifacts: Default::default(),
                        timeout: DEFAULT_TIMEOUT,
                    },
                    tasks: vec![Task {
                        description: JobDescription {
                            name: "duplicate-1-job".into(),
                            details: JobDetails::Wasm {
                                module: "me.wasm".into(),
                            },
                            artifacts: Default::default(),
                            timeout: DEFAULT_TIMEOUT,
                        },
                        tasks: Vec::new(),
                    }],
                },
                Task {
                    description: JobDescription {
                        name: "duplicate-1-job".into(),
                        details: JobDetails::Wasm {
                            module: "me.wasm".into(),
                        },
                        artifacts: Default::default(),
                        timeout: DEFAULT_TIMEOUT,
                    },
                    tasks: Vec::new(),
                },
            ],
        };
        let err = flow.validate().unwrap_err();
        assert!(err.to_string().contains("duplicate-1-job"));
    }

    #[test]
    fn test_flow_dependencies() {
        let task = Task {
            description: JobDescription {
                name: "job-1".into(),
                details: JobDetails::Wasm {
                    module: "me.wasm".into(),
                },
                artifacts: Artifacts {
                    downloads: vec!["job-1-bar".into()].into_iter().collect(),
                    uploads: Default::default(),
                },
                timeout: DEFAULT_TIMEOUT,
            },
            tasks: vec![Task {
                description: JobDescription {
                    name: "job-1-1".into(),
                    details: JobDetails::Wasm {
                        module: "me.wasm".into(),
                    },
                    artifacts: Artifacts {
                        downloads: vec!["job-1-1-foo".into()].into_iter().collect(),
                        uploads: Default::default(),
                    },
                    timeout: DEFAULT_TIMEOUT,
                },
                tasks: Vec::new(),
            }],
        };
        let ctx = JobNameContext {
            scope: Uuid::new_v4(),
        };
        let deps = task.dependencies(&ctx);
        assert_eq!(
            deps,
            vec!["job-1-bar".into(), "job-1-1-foo".into()]
                .into_iter()
                .collect()
        );

        let task = Task {
            description: JobDescription {
                name: "job-2".into(),
                details: JobDetails::Wasm {
                    module: "me.wasm".into(),
                },
                artifacts: Artifacts {
                    downloads: vec![Artifact {
                        name: "{scope}/job-2-foo".into(),
                        path: "foo-dep".into(),
                        executable: false,
                    }]
                    .into_iter()
                    .collect(),
                    uploads: Default::default(),
                },
                timeout: DEFAULT_TIMEOUT,
            },
            tasks: Vec::new(),
        };

        let deps = task.dependencies(&ctx);
        assert_eq!(
            deps,
            vec![format!("{}/job-2-foo", ctx.scope.as_simple())]
                .into_iter()
                .collect()
        );
    }
}
