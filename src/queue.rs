use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::Arc,
    time::Duration,
};

use async_channel::Receiver;
use async_trait::async_trait;
use color_eyre::{eyre::Context, Result};
use opentelemetry::{
    propagation::TextMapPropagator, sdk::propagation::TraceContextPropagator, trace::SpanKind,
};
use rsmq_async::{PooledRsmq, RedisBytes, RsmqConnection, RsmqMessage};
use serde::{Deserialize, Serialize};
use tokio::{select, task, time};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, info_span, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;

use crate::{run_ocr_background, storage, Claim, Config, Payload};

///! An async trait to represent a queue system i.e. RabbitMQ, Redis, etc.
///! It can both send and subscribe to messages with a callback, in case of errors it is sent back to the queue.
#[async_trait]
pub trait Queue: Debug + Sized + Send + Sync + 'static {
    /// Send a message to the queue
    async fn send<T: Serialize + Send>(&mut self, message: T) -> Result<()>;
    async fn subscribe(
        &mut self,
        cancel: CancellationToken,
        config: Arc<Config>,
        storage: Arc<storage::Redis>,
    ) -> Result<()>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub properties: HashMap<String, String>,
    pub payload: Payload,
    pub claim: Claim,
}

pub struct Redis {
    client: PooledRsmq,
    queue_name: String,
    time_to_process: u64,
    maximum_parallel_messages: usize,
}

impl Redis {
    pub async fn new(config: &Config) -> Result<Self> {
        let host = config
            .redis_dsn
            .host_str()
            .expect("no host provided")
            .to_string();
        let port = config.redis_dsn.port().unwrap_or(6379);
        let db_path: String = config.redis_dsn.path().chars().skip(1).collect();
        let db = db_path
            .parse()
            .wrap_err_with(|| format!("invalid db provided {db_path}"))?;
        let username = config.redis_dsn.username();
        let username = if username.is_empty() {
            None
        } else {
            Some(username.to_string())
        };
        let password = config.redis_dsn.password().map(String::from);
        let maximum_parallel_messages = 2;
        let namespace = "drive-ocr-local".to_string();
        let options = rsmq_async::RsmqOptions {
            host,
            port,
            db,
            realtime: true,
            username,
            password,
            ns: namespace.clone(),
        };
        let pool_options = rsmq_async::PoolOptions {
            max_size: Some(maximum_parallel_messages + 1),
            min_idle: None,
        };
        Ok(Self {
            client: PooledRsmq::new(options, pool_options).await?,
            queue_name: "pending-ocr-documents".to_string(),
            time_to_process: 5 * 60,
            maximum_parallel_messages: maximum_parallel_messages as usize,
        })
    }

    async fn ensure_queue(&mut self) -> Result<()> {
        match self
            .client
            .create_queue(&self.queue_name, None, None, None)
            .await
        {
            Ok(_) | Err(rsmq_async::RsmqError::QueueExists) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }
}

impl Debug for Redis {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Redis")
            .field("queue_name", &self.queue_name)
            .field("time_to_process", &self.time_to_process)
            .field("maximum_parallel_messages", &self.maximum_parallel_messages)
            .finish()
    }
}

#[async_trait]
impl Queue for Redis {
    async fn send<T: Serialize + Send>(&mut self, message: T) -> Result<()> {
        let message = serde_json::to_vec(&message)?;
        let message = RedisBytes::from(message.as_slice());

        self.client
            .send_message(self.queue_name.as_str(), message, None)
            .await?;
        Ok(())
    }

    async fn subscribe(
        &mut self,
        _cancel: CancellationToken,
        config: Arc<Config>,
        redis: Arc<storage::Redis>,
    ) -> Result<()> {
        self.ensure_queue().await?;
        let (tx, rx) = async_channel::bounded(self.maximum_parallel_messages);
        let workers = (0..self.maximum_parallel_messages)
            .map(|i| {
                info!("Starting worker {}", i);
                let rx = rx.clone();
                let client = self.client.clone();
                let queue_name = self.queue_name.clone();
                let redis = redis.clone();
                let config = config.clone();

                task::spawn(worker(rx, client, queue_name, redis, config))
            })
            .collect::<Vec<_>>();

        info!("Waiting for messages");
        let mut interval = time::interval(Duration::from_secs(1));

        loop {
            let message = self
                .client
                .receive_message(self.queue_name.as_str(), Some(self.time_to_process))
                .await
                .wrap_err_with(|| format!("Failed to read from queue {}", self.queue_name))?;
            if let Some(message) = message {
                tx.send(message).await?;
            }

            select! {
                _ = interval.tick() => {}
                _ = _cancel.cancelled() => {
                    info!("Cancelling");
                    break;
                }
            }
        }

        drop(tx);

        for worker in workers {
            worker.await??;
        }

        Ok(())
    }
}

async fn worker(
    rx: Receiver<RsmqMessage<Vec<u8>>>,
    mut client: PooledRsmq,
    queue_name: String,
    redis: Arc<storage::Redis>,
    config: Arc<Config>,
) -> Result<()> {
    while let Ok(message) = rx.recv().await {
        let deserialized: Message = serde_json::from_slice(message.message.as_slice())?;
        let propagator = TraceContextPropagator::new();
        let context = propagator.extract(&deserialized.properties);
        let span = info_span!("processing message", message_id = %deserialized.id, otel.kind = ?SpanKind::Consumer);
        span.set_parent(context);

        match run_ocr_background(
            deserialized.claim,
            deserialized.payload,
            config.clone(),
            redis.clone(),
        )
        .instrument(span)
        .await
        {
            Ok(_) => {
                client
                    .delete_message(queue_name.as_str(), message.id.as_str())
                    .await?;
            }
            Err(err) => {
                error!(?err, "Error processing message");
                client
                    .change_message_visibility(queue_name.as_str(), message.id.as_str(), 10)
                    .await?;
            }
        }
    }
    Ok(())
}
