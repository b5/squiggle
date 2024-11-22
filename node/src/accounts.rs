use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use iroh::docs::Author;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::space::users::{Profile, User};

const ACCOUNTS_FILENAME: &str = "accounts.json";

#[derive(Debug, Clone)]
pub struct Accounts {
    path: PathBuf,
    users: Arc<RwLock<Vec<User>>>,
}

impl Accounts {
    pub async fn open(base_path: impl Into<PathBuf>) -> Result<Self> {
        let path = Self::spaces_path(base_path);
        if !path.exists() {
            tokio::fs::write(&path, b"[]").await?;
        }
        let users = Self::read_from_file(&path).await?;
        Ok(Self {
            path,
            users: Arc::new(RwLock::new(users)),
        })
    }

    pub async fn create_account(&mut self, author: Author, profile: Profile) -> Result<User> {
        let user = User::new(author, profile).context("creating account")?;
        let mut users = self.users.write().await;
        users.push(user.clone());

        let mut details = Accounts::read_from_file(Self::spaces_path(self.path.clone()))
            .await
            .context("reading from file")?;
        details.push(user.clone());
        self.write_to_file(details)
            .await
            .context("writing to file")?;

        Ok(user)
    }

    pub async fn get(&self, id: &Uuid) -> Option<User> {
        self.users
            .read()
            .await
            .clone()
            .into_iter()
            .find(|user| user.id == *id)
    }

    pub async fn get_by_username(&self, name: &str) -> Option<User> {
        self.users
            .read()
            .await
            .clone()
            .into_iter()
            .find(|user| user.profile.username == name)
    }

    fn spaces_path(path: impl Into<PathBuf>) -> PathBuf {
        path.into().join(ACCOUNTS_FILENAME)
    }

    async fn read_from_file(spaces_path: impl Into<PathBuf>) -> Result<Vec<User>> {
        let path = spaces_path.into();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file = tokio::fs::read(&path).await?;
        let spaces: Vec<User> = serde_json::from_slice(&file)?;
        Ok(spaces)
    }

    async fn write_to_file(&self, details: Vec<User>) -> Result<()> {
        let file = serde_json::to_vec(&details)?;
        println!("Writing to file: {:?}", self.path);
        tokio::fs::write(&self.path, file).await?;
        Ok(())
    }

    pub async fn list(&self, _offset: i64, _limit: i64) -> Result<Vec<User>> {
        let results = Self::read_from_file(&self.path).await?;
        Ok(results)
    }
}
