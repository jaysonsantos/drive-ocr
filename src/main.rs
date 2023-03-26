use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use color_eyre::{eyre::WrapErr, Result};
use dotenvy::dotenv;
use drive_ocr::{generate_key, serve, worker};
use google_drive3::oauth2::read_application_secret;
use opentelemetry::global::shutdown_tracer_provider;
use tokio::signal::ctrl_c;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use url::Url;

#[derive(Debug, Parser)]
struct Config {
    #[clap(short, long, env, help = "Secret key to sign generated token ids")]
    secret_key: String,
    #[clap(
        short,
        long,
        env,
        help = "Redis connection to persist google credentials"
    )]
    redis_dsn: Url,
    #[clap(
        short,
        long,
        env,
        help = "Path to google's credentials JSON generated on google's dev console."
    )]
    google_credentials: Utf8PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand, Clone)]
enum Command {
    #[command(
        long_about = "Generate a key to be used on IFTT's webhook, you will need to open a link in your browser and authorize the app."
    )]
    GenerateKey,
    #[command(about = "Start a webserver to answer for IFTT's webhooks.")]
    Serve {
        #[clap(short, long, default_value("127.0.0.1:12345"), env)]
        listen_address: String,
    },
    #[command(about = "Start a worker to process the queue.")]
    Worker,
}

#[tokio::main]
async fn main() -> Result<()> {
    drive_ocr::tracing_config::init()?;
    if let Err(err) = dotenv() {
        warn!(?err, "Failed to load dotenv file");
    }

    let config = Config::parse();
    let lib_config = drive_ocr::Config {
        redis_dsn: config.redis_dsn.clone(),
        secret_key: config.secret_key.clone(),
        google_credentials: read_application_secret(config.google_credentials.clone())
            .await
            .wrap_err_with(|| {
                format!(
                    "Failed to load google credentials {}",
                    config.google_credentials
                )
            })?,
    };
    match config.command {
        Command::GenerateKey => {
            let uuid = uuid::Uuid::now_v7();
            let key = generate_key::generate_key(uuid, &lib_config).await?;
            info!(?key, ?uuid, "Key generated");
        }
        Command::Serve { listen_address } => {
            let c = CancellationToken::new();

            let token = c.clone();
            tokio::spawn(async move {
                ctrl_c().await.ok();
                info!("Control-C received");
                token.cancel();
            });

            serve(&config.secret_key, listen_address, lib_config, c).await?;
        }
        Command::Worker => {
            let c = CancellationToken::new();

            let token = c.clone();
            tokio::spawn(async move {
                ctrl_c().await.ok();
                info!("Control-C received");
                token.cancel();
            });

            worker(lib_config, c).await?;
        }
    }
    shutdown_tracer_provider();
    Ok(())
}
