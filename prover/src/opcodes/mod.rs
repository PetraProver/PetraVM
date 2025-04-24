//! Opcode implementations for the zCrayVM M3 circuit.
//!
//! This module contains the tables for each opcode instruction.

pub mod binary;
pub mod branch;
pub mod call;
pub mod integer_ops;
pub mod jump;
pub mod ldi;
pub mod mv;
pub mod ret;

pub use binary::*;
use binius_field::BinaryField;
use binius_m3::builder::B32;
pub use branch::{BnzTable, BzTable};
pub use call::TailiTable;
pub use integer_ops::AddTable;
pub use jump::{JumpiTable, JumpvTable};
pub use ldi::LdiTable;
pub use mv::{MvihTable, MvvwTable};
pub use ret::RetTable;
pub(crate) const G: B32 = B32::MULTIPLICATIVE_GENERATOR;
