//! Defines execution events for the zCray VM.
//!
//! Each event represents an instruction executed by the VM, such as arithmetic
//! operations, branching, or function calls.

use std::fmt::Debug;

use binius_field::{BinaryField16b, BinaryField32b};

use crate::execution::{InterpreterChannels, InterpreterError, InterpreterTables, ZCrayTrace};

pub(crate) mod binary_ops;
pub(crate) mod branch;
pub(crate) mod call;
pub(crate) mod integer_ops;
pub(crate) mod jump;
pub(crate) mod macros;
pub(crate) mod mv;
pub(crate) mod ret;
pub(crate) mod shift;

pub(crate) use binary_ops::{b128, b32};

#[cfg(test)]
mod test_utils;

/// An `Event` represents an instruction that can be executed by the VM.
pub trait Event {
    /// Executes the flushing rules associated to this `Event`, pushing to /
    /// pulling from their target channels.
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables);
}
