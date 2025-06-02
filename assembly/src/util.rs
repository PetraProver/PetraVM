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
    bytemuck::cast_slice(input).to_vec()
}

pub fn bytes_to_u32(input: &[u8]) -> Vec<u32> {
    if let Ok(words) = bytemuck::try_cast_slice::<u8, u32>(input) {
        words.to_vec()
    } else {
        let mut output = Vec::with_capacity(input.len() / 4);
        for chunk in input.chunks_exact(4) {
            let value = u32::from_le_bytes(
                chunk
                    .try_into()
                    .expect("The chunk contains exactly 4 bytes"),
            );
            output.push(value);
        }
        output
    }
}
