use std::sync::Arc;

use color_eyre::Result;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{ocr::LANGUAGE_REGEX, queue, queue::Queue, storage, Config};

pub async fn worker(config: Config, cancel: CancellationToken) -> Result<()> {
    let _ = &*LANGUAGE_REGEX; // Just fail fast in the regex is broken
    let storage = storage::Redis::from_dsn(config.redis_dsn.clone());
    let mut worker = queue::Redis::new(&config).await?;
    info!("Worker started");
    worker
        .subscribe(cancel, Arc::new(config), Arc::new(storage))
        .await?;
    Ok(())
}
