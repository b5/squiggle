use anyhow::Result;
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::db::DB;

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub pub_key: VerifyingKey,
    pub priv_key: Option<SigningKey>,
    pub name: String,
    pub about: String,
    pub picture: String,
}

impl User {
    pub(crate) async fn create(
        db: &DB,
        name: String,
        about: String,
        picture: String,
    ) -> Result<User> {
        let conn = db.lock().await;

        let mut cspring = OsRng;
        let signing_key = SigningKey::generate(&mut cspring);
        let pub_key = signing_key.verifying_key();
        let priv_key = signing_key.to_bytes();

        conn.execute(
            "INSERT INTO users (pubkey, privkey, name, about, picture) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![pub_key.to_bytes(), &priv_key, &name, &about, &picture],
        )?;
        Ok(User {
            pub_key,
            priv_key: Some(signing_key),
            name,
            about,
            picture,
        })
    }
}

pub struct Users(DB);

impl Users {
    pub fn new(db: DB) -> Self {
        Users(db)
    }

    pub async fn create(&self, name: String, about: String, picture: String) -> Result<User> {
        User::create(&self.0, name, about, picture).await
    }

    pub async fn list(&self) -> Result<Vec<User>> {
        let conn = self.0.lock().await;
        let mut stmt = conn.prepare("SELECT pubkey, privkey, name, about, picture FROM users")?;
        let rows = stmt.query_map([], |row| {
            let pub_key =
                VerifyingKey::from_bytes(&row.get::<_, [u8; 32]>(0)?).expect("Invalid public key");
            Ok(User {
                pub_key,
                // priv_key: Some(SigningKey::from_bytes(&row.get::<_, Vec<u8>>(2))?),
                priv_key: None,
                name: row.get(2)?,
                about: row.get(3)?,
                picture: row.get(4)?,
            })
        })?;

        let mut users = Vec::new();
        for user in rows {
            users.push(user?);
        }

        Ok(users)
    }
}
