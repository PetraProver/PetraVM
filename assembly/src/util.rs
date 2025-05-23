use binius_utils::rayon::adjust_thread_pool;
use tracing_forest::ForestLayer;
use tracing_profile::init_tracing;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter};
/// Initializes the global tracing subscriber.
///
/// The default `Level` is `INFO`. It can be overridden with `RUSTFLAGS`.
pub fn init_logger() -> Option<impl Drop> {
    if cfg!(feature = "tracing-profile") {
        adjust_thread_pool()
            .as_ref()
            .expect("failed to init thread pool");

        let guard = init_tracing().expect("failed to initialize tracing");
        return Some(guard);
    } else {
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        tracing_subscriber::registry()
            .with(filter)
            .with(ForestLayer::default())
            .init();
    }
    None
}
