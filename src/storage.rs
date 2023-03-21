use std::fmt::Debug;

use async_trait::async_trait;
use google_drive3::oauth2::storage::{TokenInfo, TokenStorage};
use redis::{AsyncCommands, Client};
use sha2::Digest;
use tracing::instrument;
use url::Url;
use uuid::Uuid;

trait Storage<S>: Debug + Clone {
    fn get_storage_for_uuid() -> S;
}

#[derive(Debug, Clone)]
pub struct Redis {
    client: Client,
}

impl Redis {
    pub fn from_dsn(dsn: Url) -> Self {
        let client = Client::open(dsn).unwrap();
        Self { client }
    }
    pub(crate) fn get_storage(&self, token_id: Uuid) -> RedisTokenStorage {
        let client = self.client.clone();
        RedisTokenStorage { token_id, client }
    }
}

pub(crate) struct RedisTokenStorage {
    token_id: Uuid,
    client: Client,
}

impl RedisTokenStorage {
    #[instrument(skip(self), ret)]
    fn get_key(&self, scopes: &[&str]) -> String {
        let mut hash = sha2::Sha256::new();
        for scope in scopes {
            hash.update(scope);
        }
        format!("{}_{}", self.token_id, hex::encode(hash.finalize()))
    }
}

#[async_trait]
impl TokenStorage for RedisTokenStorage {
    async fn set(&self, scopes: &[&str], token: TokenInfo) -> anyhow::Result<()> {
        let key = self.get_key(scopes);
        let value = serde_json::to_string(&token)?;
        self.client
            .get_async_connection()
            .await?
            .set(key, value)
            .await?;
        Ok(())
    }

    async fn get(&self, scopes: &[&str]) -> Option<TokenInfo> {
        let key = self.get_key(scopes);

        let mut client = self.client.get_async_connection().await.ok()?;
        let value: String = client.get(key).await.ok()?;
        serde_json::from_str(&value).ok()
    }
}
