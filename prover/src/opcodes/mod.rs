//! Opcode implementations for the zCrayVM M3 circuit.
//!
//! This module contains the tables for each opcode instruction.

pub mod binary;
pub mod binary_ops;
pub mod branch;
pub mod cpu;
pub mod ldi;
pub mod ret;

pub use binary::B32MulTable;
pub use binary_ops::b32::{AndiTable, XoriTable};
pub use branch::{BnzTable, BzTable};
pub use ldi::LdiTable;
pub use ret::RetTable;
