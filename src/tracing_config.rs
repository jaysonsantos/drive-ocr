use color_eyre::{eyre::WrapErr, Result};
use opentelemetry::sdk::{export, metrics};
use opentelemetry_otlp::{new_exporter, new_pipeline};
use tracing_error::ErrorLayer;
use tracing_subscriber::{prelude::*, EnvFilter};

pub fn init() -> Result<()> {
    color_eyre::install()?;
    let tracer = new_pipeline()
        .tracing()
        .with_exporter(new_exporter().tonic())
        .install_simple()?;
    let metrics_controller = new_pipeline()
        .metrics(
            metrics::selectors::simple::inexpensive(),
            export::metrics::aggregation::cumulative_temporality_selector(),
            opentelemetry::runtime::Tokio,
        )
        .with_exporter(new_exporter().tonic())
        .build()?;
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
    let metrics_layer = tracing_opentelemetry::MetricsLayer::new(metrics_controller);
    let error = ErrorLayer::default();
    let env = EnvFilter::try_from_default_env().or_else(|_| EnvFilter::try_new("info"))?;
    let stdout = tracing_subscriber::fmt::layer();
    tracing_subscriber::registry()
        .with(env)
        .with(error)
        .with(stdout)
        .with(otel_layer)
        .with(metrics_layer)
        .try_init()
        .wrap_err("failed to initialize tracing")
}
