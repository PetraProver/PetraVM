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

pub fn u32_to_bytes(input: &[u32]) -> Vec<u8> {
    let mut output = Vec::with_capacity(input.len() * 4);
    for &value in input {
        output.extend_from_slice(&value.to_le_bytes());
    }
    output
}

pub(crate) fn bytes_to_u64(input: &[u8]) -> Vec<u64> {
    let mut output = Vec::with_capacity(input.len() / 8);
    for chunk in input.chunks_exact(8) {
        let value = u64::from_le_bytes(
            chunk
                .try_into()
                .expect("Each chunk contains exactly 8 bytes"),
        );
        output.push(value);
    }
    output
}
