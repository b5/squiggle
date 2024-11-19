use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use bytes::Bytes;
use iroh::blobs::{util::SetTagOption, Hash};
use iroh::docs::{Author, AuthorId};
use serde::{Deserialize, Serialize};
use tinytemplate::TinyTemplate;
use tokio::io::AsyncWriteExt;
use tracing::debug;
use uuid::Uuid;

use crate::router::RouterClient;

use super::blobs::Blobs;

pub(crate) const JOBS_PREFIX: &str = "jobs";

#[derive(Copy, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Scheduling,
    Assigned(AuthorId),
    Completed(AuthorId),
    Canceled(Option<AuthorId>), // TODO: when should this be deleted?
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Scheduling => write!(f, "scheduling"),
            Self::Assigned(id) => write!(f, "assigned-{}", id),
            Self::Completed(id) => write!(f, "completed-{}", id),
            Self::Canceled(Some(id)) => write!(f, "canceled-{}", id),
            Self::Canceled(None) => write!(f, "canceled"),
        }
    }
}

impl std::str::FromStr for JobStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        if s == "scheduling" {
            return Ok(Self::Scheduling);
        }
        if let Some(p) = s.strip_prefix("assigned-") {
            let id: AuthorId = p.parse()?;
            return Ok(Self::Assigned(id));
        }
        if let Some(p) = s.strip_prefix("completed-") {
            let id: AuthorId = p.parse()?;
            return Ok(Self::Completed(id));
        }
        if let Some(p) = s.strip_prefix("canceled-") {
            let id: AuthorId = p.parse()?;
            return Ok(Self::Canceled(Some(id)));
        }
        if s == "canceled" {
            return Ok(Self::Canceled(None));
        }
        bail!("Unknown job status: {}", s);
    }
}

impl JobStatus {
    pub fn merge(&mut self, other: JobStatus) -> bool {
        let mut replaced = false;
        *self = match (*self, other) {
            (JobStatus::Scheduling, JobStatus::Assigned(_)) => {
                replaced = true;
                other
            }
            (JobStatus::Assigned(a), JobStatus::Completed(b)) => {
                if a == b {
                    replaced = true;
                    other
                } else {
                    *self
                }
            }
            (status, _) => status,
        };
        replaced
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum Source {
    LocalPath(String),
    LocalBlob(iroh::blobs::Hash),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum JobDetails {
    /// Run a job inside of a docker container.
    #[serde(rename = "docker")]
    Docker {
        /// Docker image to execute job within.
        image: String,
        /// Command to execute
        command: Vec<String>,
    },
    #[serde(rename = "wasm")]
    Wasm {
        /// Path to the compiled `.wasm` module.
        /// Expects to be a wasi module
        module: Source,
    },
}

impl JobDetails {
    pub fn typ(&self) -> JobType {
        match self {
            JobDetails::Docker { .. } => JobType::Docker,
            JobDetails::Wasm { .. } => JobType::Wasm,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum JobType {
    Docker,
    Wasm,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct JobDescription {
    /// name of the space to run the job in
    /// TODO - this should be the space id
    pub space: String,
    /// Human-readable name of the job
    pub name: String,
    /// the identifier of the user to run the job as.
    /// Must have private half of key stored locally
    pub author: String,
    // configuration to pass to execution environment
    pub environment: HashMap<String, String>,
    /// Job details.
    pub details: JobDetails,
    #[serde(default)]
    pub artifacts: Artifacts,
    #[serde(default = "default_timeout")]
    pub timeout: time::Duration,
}

pub const DEFAULT_TIMEOUT: time::Duration = time::Duration::HOUR;

fn default_timeout() -> time::Duration {
    DEFAULT_TIMEOUT
}

#[derive(Default, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Artifacts {
    /// List of artifacts to download.
    #[serde(default)]
    pub downloads: BTreeSet<Artifact>,
    /// List of outputs to save.
    #[serde(default)]
    pub uploads: BTreeSet<Artifact>,
}

impl Artifacts {
    pub fn get_download_by_path(&self, path: &str) -> Option<&Artifact> {
        self.downloads.iter().find(|a| a.path == path)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Artifact {
    /// The object name of the artifact
    pub name: String,
    /// The path for this artifact. If this is a folder, the whole folder is added.
    pub path: String,
    /// Should the executable bit be set?
    #[serde(default)]
    pub executable: bool,
}

impl From<&str> for Artifact {
    fn from(value: &str) -> Self {
        Artifact {
            name: value.into(),
            path: value.into(),
            executable: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct JobNameContext {
    #[serde(with = "uuid::serde::simple")]
    pub scope: Uuid,
}

impl JobNameContext {
    pub fn render(&self, template: &str) -> Result<String> {
        let mut tt = TinyTemplate::new();
        tt.add_template("current", template)?;
        let name = tt.render("current", self)?;
        Ok(name)
    }
}

impl Artifact {
    /// Get the file to use for this file
    pub fn mode(&self) -> u32 {
        if self.executable {
            0o755
        } else {
            0o644
        }
    }

    pub async fn content_hash(&self, ctx: &JobNameContext, blobs: &Blobs) -> Result<Hash> {
        let name = ctx.render(&self.name)?;
        let entry = blobs.get_object_info(&name).await?;
        Ok(entry.content_hash())
    }

    pub async fn set_name(
        &self,
        job_name_ctx: &JobNameContext,
        blobs: &Blobs,
        job_name: &str,
        hash: Hash,
        size: u64,
    ) -> Result<()> {
        let template = format!("{{scope}}/{job_name}/{}", self.name);
        let name = job_name_ctx.render(&template)?;
        blobs.put_object(&name, hash, size).await?;

        Ok(())
    }
}

impl JobDescription {
    pub fn to_bytes(&self) -> Result<Bytes> {
        let data = serde_json::to_vec(self).context("failed to serialize job description")?;
        Ok(data.into())
    }

    pub fn job_type(&self) -> JobType {
        self.details.typ()
    }

    pub fn dependencies(&self, ctx: JobNameContext) -> impl Iterator<Item = Result<String>> + '_ {
        self.artifacts
            .downloads
            .iter()
            .map(move |artifact| ctx.render(&artifact.name))
    }
}

impl TryFrom<Bytes> for JobDescription {
    type Error = serde_json::Error;

    fn try_from(b: Bytes) -> std::result::Result<Self, Self::Error> {
        serde_json::from_slice(&b)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ScheduledJob {
    pub author: AuthorId,
    pub description: JobDescription,
    pub scope: Uuid,
    pub result: JobResult,
}

impl ScheduledJob {
    pub fn to_bytes(&self) -> Result<Bytes> {
        let data = serde_json::to_vec(self).context("failed to serialize job description")?;
        Ok(data.into())
    }

    pub fn job_type(&self) -> JobType {
        self.description.job_type()
    }
}

impl TryFrom<Bytes> for ScheduledJob {
    type Error = serde_json::Error;

    fn try_from(b: Bytes) -> std::result::Result<Self, Self::Error> {
        serde_json::from_slice(&b)
    }
}

#[derive(Default, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct JobResult {
    /// The worker that executed the job.
    pub worker: Option<AuthorId>,
    pub status: JobResultStatus,
}

#[derive(Default, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum JobResultStatus {
    #[default]
    Unknown,
    Ok(JobOutput),
    Err(String),
    ErrTimeout,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum JobOutput {
    Docker {
        code: i64,
        stderr: String,
        stdout: String,
    },
    Wasm {
        output: String,
    },
}

#[derive(Debug)]
pub struct JobContext {
    // space to run the job within
    pub space: String,
    /// Job id
    pub id: Uuid,
    pub environment: HashMap<String, String>,
    /// Job name
    pub name: String,
    pub name_context: JobNameContext,
    pub author: Author,
    pub artifacts: Artifacts,
}

impl JobContext {
    pub fn downloads_path(&self, root: impl AsRef<Path>) -> PathBuf {
        root.as_ref()
            .join(self.name_context.scope.as_simple().to_string())
            .join(&self.name)
            .join("downloads")
    }

    pub fn uploads_path(&self, root: impl AsRef<Path>) -> PathBuf {
        root.as_ref()
            .join(self.name_context.scope.as_simple().to_string())
            .join(&self.name)
            .join("uploads")
    }

    pub fn job_scope(&self, ctx: &str) -> String {
        format!(
            "{}-{}-{}-{}",
            self.name_context.scope.as_simple(),
            self.name,
            self.author,
            ctx,
        )
    }

    /// Writes all download artifacts relative to the given path.
    pub async fn write_downloads(
        &self,
        path: impl AsRef<Path>,
        blobs: &Blobs,
        node: &RouterClient,
    ) -> Result<()> {
        // Todo: parallelize

        let path = path.as_ref();

        debug!("downloading to {}", path.display());
        // TODO: sanitize path (strip `..` etc)

        tokio::fs::create_dir_all(path)
            .await
            .context("create_dir_all")?;

        for artifact in &self.artifacts.downloads {
            debug!("writing download {:?}", artifact);
            let artifact_hash = artifact.content_hash(&self.name_context, blobs).await?;
            let mut blob_reader = node.blobs().read(artifact_hash).await?;
            let file_path = path.join(&artifact.path);

            let mode = artifact.mode();
            let mut out_file = tokio::fs::OpenOptions::new();
            out_file.create(true).write(true);
            #[cfg(unix)]
            {
                out_file.mode(mode);
            }
            let mut out = out_file.open(&file_path).await.context("open")?;
            tokio::io::copy(&mut blob_reader, &mut out)
                .await
                .context("copy")?;
            out.flush().await?;
            drop(out)
        }

        Ok(())
    }

    pub async fn read_uploads(
        &self,
        path: impl AsRef<Path>,
        blobs: &Blobs,
        node: &RouterClient,
    ) -> Result<()> {
        // Todo: parallelize
        let path = path.as_ref();

        debug!("uploading from {}", path.display());

        for artifact in &self.artifacts.uploads {
            debug!("reading upload {:?}", artifact);
            let file_path = path.join(&artifact.path);

            let upload_file = |fp: PathBuf, prefix: Option<PathBuf>| async {
                debug!("reading {}", fp.display());
                let source = tokio::fs::File::open(fp).await?;
                let res = node
                    .blobs()
                    .add_reader(source, SetTagOption::Auto)
                    .await?
                    .await?;

                let template = if let Some(prefix) = prefix {
                    format!("{{scope}}/{}/{}", self.name, prefix.to_string_lossy())
                } else {
                    format!("{{scope}}/{}/{}", self.name, artifact.name)
                };
                let name = self.name_context.render(&template)?;
                debug!("uploaded artifact {}", name);
                blobs.put_object(&name, res.hash, res.size).await?;
                anyhow::Ok(())
            };

            if file_path.is_file() {
                upload_file(file_path, None).await?;
            } else if file_path.is_dir() {
                let root = file_path.clone();
                let sources = tokio::task::spawn_blocking(move || {
                    let files = walkdir::WalkDir::new(root).into_iter();
                    files
                        .map(|entry| {
                            let entry = entry?;
                            if !entry.file_type().is_file() {
                                // Skip symlinks. Directories are handled by WalkDir.
                                return Ok(None);
                            }
                            let path = entry.into_path();
                            anyhow::Ok(Some(path))
                        })
                        .filter_map(Result::transpose)
                        .collect::<Result<Vec<_>>>()
                })
                .await??;
                debug!("found {} files in {}", sources.len(), file_path.display());
                for source in sources {
                    let prefix = source.strip_prefix(path)?.into();
                    upload_file(source, Some(prefix)).await?;
                }
            } else {
                bail!("unable to read file: {}", file_path.display());
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rand::thread_rng;

    use super::*;

    #[test]
    fn test_render_job_name() {
        let ctx = JobNameContext {
            scope: Uuid::new_v4(),
        };

        assert_eq!(ctx.render("hello").unwrap(), "hello".to_string(),);
        assert_eq!(
            ctx.render("{scope}/hello").unwrap(),
            format!("{}/hello", ctx.scope.as_simple()),
        );
        assert!(ctx.render("{other}/hello").is_err());
    }

    #[test]
    fn test_job_dependencies() {
        let author_id = Author::new(&mut thread_rng()).id();
        let job = JobDescription {
            author: author_id,
            name: "foo".into(),
            environment: Default::default(),
            details: JobDetails::Docker {
                image: "alpine:latest".into(),
                command: vec!["ls".into()],
            },
            artifacts: Artifacts {
                downloads: vec!["foo".into(), "bar".into(), "baz".into()]
                    .into_iter()
                    .collect(),
                uploads: Default::default(),
            },
            timeout: DEFAULT_TIMEOUT,
        };

        let ctx = JobNameContext {
            scope: Uuid::new_v4(),
        };

        let mut deps = job.dependencies(ctx).collect::<Result<Vec<_>>>().unwrap();
        deps.sort();
        assert_eq!(deps, vec!["bar".to_string(), "baz".into(), "foo".into()]);
    }

    #[test]
    fn test_display_job_status() {
        assert_eq!(
            JobStatus::Scheduling
                .to_string()
                .parse::<JobStatus>()
                .unwrap(),
            JobStatus::Scheduling
        );
        let id = iroh::docs::Author::new(&mut thread_rng()).id();
        assert_eq!(
            JobStatus::Assigned(id)
                .to_string()
                .parse::<JobStatus>()
                .unwrap(),
            JobStatus::Assigned(id)
        );
        assert_eq!(
            JobStatus::Canceled(Some(id))
                .to_string()
                .parse::<JobStatus>()
                .unwrap(),
            JobStatus::Canceled(Some(id))
        );
        assert_eq!(
            JobStatus::Canceled(None)
                .to_string()
                .parse::<JobStatus>()
                .unwrap(),
            JobStatus::Canceled(None)
        );
    }
}
