//! zCrayVM Proving System using Binius M3 Arithmetization.
//!
//! This library implements the proving system for the zCrayVM using M3
//! arithmetization. The design is modular, with each opcode
//! instruction having its own M3 table implementation.

#![allow(dead_code)]
pub mod channels;
pub mod circuit;
pub mod gadgets;
pub mod model;
pub mod opcodes;
pub mod prover;
pub mod tables;
pub mod types;
pub mod utils;
