//! Modular Instruction Set Architectures (ISAs) for the zCray Virtual Machine.
//!
//! An ISA defines:
//! - The tables it contributes to the arithmetic circuit.
//! - How the execution trace is interpreted into events for those tables.
//!
//! A given ISA is registered when initializing a new
//! [`Circuit`](crate::circuit::Circuit), which invokes [`ISA::register_tables`]
//! to instantiate and wire up all instruction tables needed.

use binius_m3::builder::ConstraintSystem;

use crate::{
    channels::Channels,
    table::{FillableTable, LdiTable, RetTable, Table},
};

// TODO(Robin): Maybe create some `VirtualMachine` object containing on the
// `assembly` side containing both the `ISA` and the `EventContext`?

/// Defines an Instruction Set Architecture for the zCray Virtual Machine.
///
/// Each implementation of this trait should provide the different tables
/// needed to represent its supported instructions. This can be done easily
/// through the [`register_table!`](crate::register_table) macro.
pub trait ISA {
    // TODO(Robin) should it support some decode method to catch unsupported
    // instructions during emulation?

    /// # Arguments
    /// * `cs` - [`ConstraintSystem`]` to register the tables to
    /// * `channels` - [`Channels`] IDs for communication with other tables
    fn register_tables(
        &self,
        cs: &mut ConstraintSystem,
        channels: &Channels,
    ) -> Vec<Box<dyn FillableTable>>;
}

/// Registers a table along with its associated event accessor.
///
/// # Example
///
/// ```ignore
/// register_table!(LdiTable, ldi_events, cs, channels)
/// ```
#[macro_export]
macro_rules! register_table {
    ($table_ty:ty, $trace_accessor:ident, $cs:expr, $channels:expr) => {
        Box::new($crate::table::TableEntry {
            table: Box::new(<$table_ty>::new($cs, $channels)),
            get_events: $crate::model::Trace::$trace_accessor,
        }) as Box<dyn $crate::table::FillableTable>
    };
}

// TODO: implement when possible
pub struct RecursionISA;

/// The main Instruction Set Architecture for the zCray Virtual Machine,
/// supporting all existing instructions.
pub struct GenericISA;

impl ISA for GenericISA {
    fn register_tables(
        &self,
        cs: &mut ConstraintSystem,
        channels: &Channels,
    ) -> Vec<Box<dyn FillableTable>> {
        vec![
            register_table!(LdiTable, ldi_events, cs, channels),
            register_table!(RetTable, ret_events, cs, channels),
        ]
    }
}
