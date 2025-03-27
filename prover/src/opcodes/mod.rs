//! Opcode implementations for the zCrayVM M3 circuit.
//!
//! This module contains the tables for each opcode instruction.

pub mod ret;
pub mod ldi;

pub use ret::RetTable;
pub use ldi::LdiTable; 