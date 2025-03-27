//! Error types for the zCrayVM proving system.

use thiserror::Error;

/// Errors that can occur during the proving process
#[derive(Debug, Error)]
pub enum ProverError {
    /// Error from Binius
    #[error("Binius error: {0}")]
    BiniusError(#[from] anyhow::Error),

    /// Error from the Assembly crate
    #[error("Assembly error: {0}")]
    AssemblyError(String),

    /// Invalid trace data
    #[error("Invalid trace data: {0}")]
    InvalidTraceData(String),

    /// Missing opcode implementation
    #[error("Missing opcode implementation: {0}")]
    MissingOpcodeImplementation(String),

    /// Unimplemented feature
    #[error("Unimplemented feature: {0}")]
    Unimplemented(String),
}

/// Result type for the proving system
pub type Result<T> = std::result::Result<T, ProverError>;
