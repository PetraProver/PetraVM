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
pub use branch::*;
pub use call::*;
pub use integer_ops::*;
pub use ldi::LdiTable;
pub use mv::MvvwTable;
pub use ret::RetTable;
