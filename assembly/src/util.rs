use tracing_forest::ForestLayer;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter};

/// Initializes the global tracing subscriber.
///
/// The default `Level` is `INFO`. It can be overridden with `RUSTFLAGS`.
pub fn init_logger() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(ForestLayer::default())
        .init();
}
