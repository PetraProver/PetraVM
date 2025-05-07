//! PetraVM Proving System using Binius M3 Arithmetization.
//!
//! This library implements the proving system for the PetraVM using M3
//! arithmetization. The design is modular, with each opcode
//! instruction having its own M3 table implementation.

#![allow(dead_code)]
pub mod channels;
pub mod circuit;
pub mod gadgets;
pub mod memory;
pub mod model;
pub mod opcodes;
pub mod prover;
pub mod table;
pub mod types;
pub mod utils;

/// Publicly exported module for testing purposes only
pub mod test_utils;
