//! Modular Instruction Set Architectures (ISAs) for the zCray Virtual Machine.
//!
//! An ISA defines:
//! - The tables it contributes to an arithmetic circuit.
//! - How an execution trace is interpreted into events for those tables.
//!
//! A given ISA is registered when initializing a new
//! [`Circuit`](crate::circuit::Circuit), which invokes [`ISA::register_tables`]
//! to instantiate and wire up all instruction tables needed.

use binius_m3::builder::ConstraintSystem;

use crate::{
    channels::Channels,
    memory::{BnzTable, BzTable},
    table::{FillableTable, LdiTable, RetTable, Table},
};

// TODO(Robin): Maybe create some `VirtualMachine` object on the
// `assembly` side containing both the `ISA` and the `EventContext`?

/// Defines an Instruction Set Architecture for the zCray Virtual Machine.
///
/// Each implementation of this trait should provide the different tables
/// needed to represent its supported instructions. This can be done easily
/// through the [`register_table!`](crate::register_table) macro.
pub trait ISA {
    // TODO(Robin) should it support some decode method to catch unsupported
    // instructions during emulation? CF TODO Above

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

/// Creates a new ISA and registers all its instruction tables.
///
/// # Example
///
/// ```ignore
/// define_isa!(MinimalISA => (LdiTable, ldi_events), (RetTable, ret_events));
/// ```
#[macro_export]
macro_rules! define_isa {
    (
        $(#[$doc:meta])*
        $isa_ty:ident => [ $( ($table_ty:ty, $accessor:ident) ),* $(,)? ]
    ) => {
        $(#[$doc])*
        pub struct $isa_ty;

        impl $crate::isa::ISA for $isa_ty {
            fn register_tables(
                &self,
                cs: &mut binius_m3::builder::ConstraintSystem,
                channels: &$crate::channels::Channels,
            ) -> Vec<Box<dyn $crate::table::FillableTable>> {
                vec![
                    $(
                        $crate::register_table!($table_ty, $accessor, cs, channels)
                    ),*
                ]
            }
        }
    };
}

// TODO: Implement Recursion VM whenever possible.
// Needs to implement #79.

// define_isa!(
//     /// A minimal ISA for the zCray Virtual Machine,
//     /// tailored for efficient recursion.
//     RecursionISA => []
// );

define_isa!(
    /// The main Instruction Set Architecture (ISA) for the zCray Virtual Machine,
    /// supporting all existing instructions.
    GenericISA => [
    (BzTable, bz_events),
    (BnzTable, bnz_events),
    (LdiTable, ldi_events),
    (RetTable, ret_events),
]);
