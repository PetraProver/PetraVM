//! Opcode implementations for the PetraVM M3 circuit.
//!
//! This module contains the tables for each opcode instruction.

use binius_field::BinaryField;
use binius_m3::builder::B32;

pub mod binary;
pub mod branch;
pub mod call;
pub mod comparison;
pub mod integer_ops;
pub mod jump;
pub mod ldi;
pub mod mv;
pub mod ret;
pub mod shift;

pub use binary::*;
pub use branch::{BnzTable, BzTable};
pub use call::{CalliTable, CallvTable, TailiTable, TailvTable};
pub use comparison::SltuTable;
pub use integer_ops::{
    SubTable, {AddTable, AddiTable},
};
pub use jump::{JumpiTable, JumpvTable};
pub use ldi::LdiTable;
pub use mv::{MvihTable, MvvlTable, MvvwTable};
pub use ret::RetTable;
pub use shift::{SllTable, SlliTable, SraTable, SraiTable, SrlTable, SrliTable};
pub(crate) const G: B32 = B32::MULTIPLICATIVE_GENERATOR;
