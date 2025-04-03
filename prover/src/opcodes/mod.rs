//! Opcode implementations for the zCrayVM M3 circuit.
//!
//! This module contains the tables for each opcode instruction.

pub mod binary;
pub mod ldi;
pub mod ret;

pub use binary::B32MulTable;
pub use ldi::LdiTable;
pub use ret::RetTable;
