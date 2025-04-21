//! Opcode implementations for the zCrayVM M3 circuit.
//!
//! This module contains the tables for each opcode instruction.

pub mod binary;
pub mod branch;
pub mod call;
pub mod integer_ops;
pub mod ldi;
pub mod mv;
pub mod ret;

pub use binary::*;
pub use branch::{BnzTable, BzTable};
pub use call::TailiTable;
pub use integer_ops::AddTable;
pub use ldi::LdiTable;
pub use mv::{MvihTable, MvvwTable};
pub use ret::RetTable;
