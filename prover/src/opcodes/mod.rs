//! Opcode implementations for the zCrayVM M3 circuit.
//!
//! This module contains the tables for each opcode instruction.

pub mod branch;
pub mod ldi;
pub mod ret;

pub use branch::{BnzTable, BzTable};
pub use ldi::LdiTable;
pub use ret::RetTable;
