//! Common type aliases and traits used throughout the prover.
//!
//! This module defines reusable type aliases to simplify code across the
//! codebase.

use binius_field::arch::OptimalUnderlier;
use binius_field::as_packed_field::PackedType;
use binius_m3::builder::{Boundary, B128};

/// The preferred packed field type used by the prover
pub type ProverPackedField = PackedType<OptimalUnderlier, B128>;

/// Statement describing the circuit instance for proving and verification.
///
/// This mirrors the struct that used to be provided by `binius_m3`.
/// It simply bundles the channel boundaries together with the table sizes.
#[derive(Debug, Clone)]
pub struct Statement {
    pub boundaries: Vec<Boundary<B128>>,
    pub table_sizes: Vec<usize>,
}
