//! Common type aliases and traits used throughout the prover.
//!
//! This module defines reusable type aliases to simplify code across the
//! codebase.

use binius_field::arch::OptimalUnderlier128b;
use binius_field::as_packed_field::PackedType;
use binius_m3::builder::B128;

/// The preferred packed field type used by the prover
pub type ProverPackedField = PackedType<OptimalUnderlier128b, B128>;
