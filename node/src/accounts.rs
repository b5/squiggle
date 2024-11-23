use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use iroh::docs::Author;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::space::users::{Profile, User};

const ACCOUNTS_FILENAME: &str = "accounts.json";

#[derive(Debug, Clone)]
pub struct Accounts {
    file_path: PathBuf,
    inner: Arc<RwLock<InnerAccounts>>,
}

impl Accounts {
    pub async fn open(base_path: impl Into<PathBuf>) -> Result<Self> {
        let path = Self::spaces_path(base_path);
        if !path.exists() {
            let blank = serde_json::to_vec(&InnerAccounts::default())?;
            tokio::fs::write(&path, blank).await?;
        }
        let inner = InnerAccounts::read_from_file(&path).await?;
        Ok(Self {
            file_path: path,
            inner: Arc::new(RwLock::new(inner)),
        })
    }

    pub async fn create_account(&mut self, author: Author, profile: Profile) -> Result<User> {
        let user = User::new(author, profile).context("creating account")?;
        let mut inner = self.inner.write().await;
        inner.accounts.push(user.clone());

        let mut details = InnerAccounts::read_from_file(&Self::spaces_path(self.file_path.clone()))
            .await
            .context("reading from file")?;
        details.accounts.push(user.clone());
        details
            .write_to_file(&self.file_path)
            .await
            .context("writing to file")?;

        Ok(user)
    }

    pub async fn current(&self) -> Option<User> {
        let inner = self.inner.read().await;
        if inner.accounts.len() < inner.current {
            return None;
        }
        Some(inner.accounts[inner.current].clone())
    }

    pub async fn get(&self, id: &Uuid) -> Option<User> {
        self.inner
            .read()
            .await
            .accounts
            .clone()
            .into_iter()
            .find(|user| user.id == *id)
    }

    pub async fn get_by_username(&self, name: &str) -> Option<User> {
        self.inner
            .read()
            .await
            .accounts
            .clone()
            .into_iter()
            .find(|user| user.profile.username == name)
    }

    fn spaces_path(path: impl Into<PathBuf>) -> PathBuf {
        path.into().join(ACCOUNTS_FILENAME)
    }

    pub async fn list(&self, _offset: i64, _limit: i64) -> Result<Vec<User>> {
        let results = InnerAccounts::read_from_file(&self.file_path).await?;
        Ok(results.accounts.clone())
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct InnerAccounts {
    current: usize, // offset of the account in the accounts array currently in use
    accounts: Vec<User>,
}

impl InnerAccounts {
    async fn read_from_file(path: &PathBuf) -> Result<InnerAccounts> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let file = tokio::fs::read(&path).await?;
        serde_json::from_slice::<Self>(&file).context("reading accounts db")
    }

    async fn write_to_file(&self, path: &PathBuf) -> Result<()> {
        let data = serde_json::to_vec(&self)?;
        tokio::fs::write(path, data).await?;
        Ok(())
    }
}
