use color_eyre::{eyre::WrapErr, Result};
use opentelemetry::runtime;
use opentelemetry_otlp::{new_exporter, new_pipeline};
use tracing::metadata::LevelFilter;
use tracing_error::ErrorLayer;
use tracing_subscriber::{prelude::*, EnvFilter};

pub fn init() -> Result<()> {
    let tracer = new_pipeline()
        .tracing()
        .with_exporter(new_exporter().tonic())
        .install_batch(runtime::Tokio)?;
    let metrics_controller = new_pipeline()
        .metrics(runtime::Tokio)
        .with_exporter(new_exporter().tonic())
        .build()?;
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
    let metrics_layer = tracing_opentelemetry::MetricsLayer::new(metrics_controller);
    let error = ErrorLayer::default();
    let env = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    let stdout = tracing_subscriber::fmt::layer();
    tracing_subscriber::registry()
        .with(metrics_layer)
        .with(otel_layer)
        .with(stdout)
        .with(env)
        .with(error)
        .try_init()
        .wrap_err("failed to initialize tracing")?;

    color_eyre::install()
}
