use color_eyre::eyre::WrapErr;
use google_drive3::{api::Scope, oauth2, oauth2::InstalledFlowReturnMethod};
use hmac::digest::KeyInit;
use jwt::SignWithKey;
use uuid::Uuid;

use crate::{storage::Redis, Claim, Config, Hmac256};

pub async fn generate_key(token_id: Uuid, config: &Config) -> color_eyre::Result<String> {
    let redis = Redis::from_dsn(config.redis_dsn.clone());
    let auth = oauth2::InstalledFlowAuthenticator::builder(
        config.google_credentials.clone(),
        InstalledFlowReturnMethod::HTTPPortRedirect(12346),
    )
    .with_storage(Box::new(redis.get_storage(token_id)))
    .build()
    .await
    .wrap_err("failed to build authenticator")?;
    let _ = auth
        .token(&[Scope::Full])
        .await
        .wrap_err("failed to fetch token")?;

    let claim = Claim { token_id };
    let key = Hmac256::new_from_slice(config.secret_key.as_bytes())?;
    claim.sign_with_key(&key).wrap_err("failed to sign claim")
}
