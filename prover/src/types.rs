//! Common type aliases and traits used throughout the prover.
//!
//! This module defines reusable type aliases to simplify code across the
//! codebase.

use binius_field::arch::OptimalUnderlier128b;
use binius_field::as_packed_field::PackScalar;
use binius_field::underlier::Divisible;
use binius_m3::builder::{B1, B128, B16, B32};
use bytemuck::Pod;

/// Type alias for common trait bounds required by TableFiller implementations
/// for all zCrayVM tables.
pub trait CommonTableBounds:
    Pod
    + PackScalar<B1>
    + PackScalar<B16>
    + PackScalar<B32>
    + PackScalar<B128>
    + Divisible<u16>
    + Divisible<u32>
    + Divisible<u128>
{
}

// Implement our trait for the OptimalUnderlier128b type, which is used by the
// prover
impl CommonTableBounds for OptimalUnderlier128b {}
