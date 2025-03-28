//! Opcode implementations for the zCrayVM M3 circuit.
//!
//! This module contains the tables for each opcode instruction.

pub mod ldi;
pub mod ret;
pub mod branch;
pub mod cpu;
pub mod binary_ops;
pub mod integer_ops;

pub use ldi::LdiTable;
pub use ret::RetTable;
pub use binary_ops::b32::XoriTable;
