//! zCrayVM Proving System using Binius M3 Arithmetization.
//!
//! This library implements the proving system for the zCrayVM using M3 (Mersenne3)
//! arithmetization. The design is modular, with each opcode instruction having its
//! own M3 table implementation.

pub mod model;
pub mod tables;
pub mod channels;
pub mod circuit;
pub mod prover;
pub mod error;
pub mod ldi;
pub mod opcodes;