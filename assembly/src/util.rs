use tracing_subscriber::EnvFilter;

/// Initializes the global tracing subscriber.
///
/// The default `Level` is `INFO`. It can be overridden with `RUSTFLAGS`.
pub fn init_logger() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
