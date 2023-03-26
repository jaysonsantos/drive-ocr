use std::{collections::HashMap, convert::Infallible, net::SocketAddr, sync::Arc};

use camino::Utf8PathBuf;
use color_eyre::{eyre::WrapErr, Result};
use google_drive3::oauth2::ApplicationSecret;
use hmac::{
    digest::{core_api::CoreWrapper, KeyInit},
    Hmac, HmacCore,
};
use jwt::VerifyWithKey;
use opentelemetry::{
    propagation::TextMapPropagator, sdk::propagation::TraceContextPropagator, trace::SpanKind,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::Sha256;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, instrument, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use url::Url;
use uuid::Uuid;
use warp::{http::StatusCode, Filter, Rejection, Reply};

use crate::{
    errors::Error,
    ocr::{process_input, LANGUAGE_REGEX},
    queue::{Message, Queue},
    storage::Redis,
};

mod errors;
pub mod generate_key;
mod ocr;
mod queue;
mod storage;
pub mod tracing_config;
mod upload;
pub mod worker;
pub use crate::worker::worker;

type Hmac256 = Hmac<Sha256>;
type Jwt = CoreWrapper<HmacCore<Sha256>>;

#[derive(Debug)]
pub struct Config {
    pub redis_dsn: Url,
    pub secret_key: String,
    pub google_credentials: ApplicationSecret,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Payload {
    filename: String,
    path: Utf8PathBuf,
    file_url: Url,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claim {
    token_id: Uuid,
}

pub async fn serve<S>(
    secret_key: S,
    listen_address: String,
    config: Config,
    cancel_token: CancellationToken,
) -> Result<()>
where
    S: AsRef<[u8]>,
{
    let _ = &*LANGUAGE_REGEX; // Just fail fast in the regex is broken
    let key = Arc::new(
        Hmac256::new_from_slice(secret_key.as_ref()).wrap_err("failed to construct an hmac")?,
    );
    let key = warp::any().map(move || key.clone());

    let queue = Arc::new(RwLock::new(queue::Redis::new(&config).await?));
    let queue = warp::any().map(move || queue.clone());

    let health = warp::path("health").map(|| "OK".to_string());

    let ocr = warp::path("ocr")
        .and(warp::path::param().and(key).and_then(verify_token))
        .and(warp::body::content_length_limit(1024 * 4))
        .and(warp::body::json::<Payload>())
        .and(queue)
        .and_then(run_ocr)
        .recover(handle_error)
        .with(warp::trace::request());

    let cancelled = cancel_token.cancelled_owned();
    let addr: SocketAddr = listen_address.parse().wrap_err("invalid listen address")?;
    let (addr, server) = warp::serve(health.or(ocr)).bind_with_graceful_shutdown(addr, cancelled);
    let git_commit = env!("GIT_COMMIT");
    let git_branch = env!("GIT_BRANCH");
    let build_time = env!("BUILD_TIME");
    info!(
        ?addr,
        git_commit, git_branch, build_time, "Server listenings"
    );
    server.await;
    Ok(())
}

async fn verify_token(token: String, key: Arc<Jwt>) -> std::result::Result<Claim, Rejection> {
    info!(monotonic_counter.ocr_call = 1);
    token.verify_with_key(key.as_ref()).map_err(|err| {
        error!(?err, "Invalid token");
        warp::reject::custom(Error::AccessDenied)
    })
}

#[instrument(skip_all, fields(otel.kind = ?SpanKind::Server))]
async fn run_ocr<Q>(
    claim: Claim,
    payload: Payload,
    queue: Arc<RwLock<Q>>,
) -> std::result::Result<impl Reply, Rejection>
where
    Q: Queue,
{
    info!("Queueing request");
    let propagator = TraceContextPropagator::new();
    let mut properties = HashMap::new();
    propagator.inject_context(&Span::current().context(), &mut properties);
    let message_id = Uuid::now_v7();

    let message = Message {
        id: message_id,
        properties,
        payload,
        claim,
    };

    match queue.write().await.send(message).await {
        Ok(_) => {
            info!(%message_id, "Queued request");
        }
        Err(err) => {
            error!(?err, %message_id, "Failed to queue request");
            return Err(warp::reject::custom(Error::Queue(err)));
        }
    }
    let response = json!({"status": "queued", "id": message_id});
    Ok(warp::reply::json(&response))
}

#[instrument(skip_all, ret)]
async fn run_ocr_background(
    claim: Claim,
    payload: Payload,
    config: Arc<Config>,
    redis: Arc<Redis>,
) -> Result<()> {
    info!(app = %claim.token_id, "Got payload");

    let files = process_input(&payload).await.map_err(Error::Orc)?;
    let upload_path = payload
        .path
        .as_path()
        .parent()
        .map(|p| p.join("Done"))
        .unwrap_or_else(|| payload.path.join("Done"));
    upload::upload_files(claim, &files, upload_path.as_path(), config, redis)
        .await
        .map_err(Error::Upload)?;
    cleanup(files).await.map_err(Error::Cleanup)?;
    info!(monotonic_counter.success_ocr_call = 1);
    Ok(())
}

#[instrument]
async fn cleanup(files: Vec<Utf8PathBuf>) -> Result<()> {
    match files.first().and_then(|f| f.parent()) {
        None => Ok(()),
        Some(folder) => tokio::fs::remove_dir_all(folder)
            .await
            .wrap_err("failed to clean up directory"),
    }
}

async fn handle_error(_err: Rejection) -> std::result::Result<impl Reply, Infallible> {
    info!(monotonic_counter.ocr_error_call = 1);
    let code = StatusCode::INTERNAL_SERVER_ERROR;
    let message = "Internal Server Error";

    let json = warp::reply::json(&json!({
        "code": code.as_u16(),
        "message": message,
    }));
    Ok(warp::reply::with_status(json, code))
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn double_check_ssl_flags() {
        assert_eq!(
            reqwest::get("https://httbin.org/ip")
                .await
                .unwrap()
                .status()
                .as_u16(),
            200
        );
    }
}
