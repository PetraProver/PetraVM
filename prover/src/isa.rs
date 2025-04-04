//! Implementation of the different Instruction Set Architectures (ISAs) to be
//! supported by the zCray Virtual Machine.

use binius_m3::builder::ConstraintSystem;

use crate::{
    channels::Channels,
    model::Trace,
    table::{Fill, LdiTable, RetTable, Table, TableEntry},
};

// TODO(Robin): Maybe create some `VirtualMachine` object containing on the
// `assembly` side containing both the `ISA` and the `EventContext`?

pub trait ISA {
    // TODO(Robin) should it support some decode method to catch unsupported
    // instructions during emulation?

    fn register_tables(&self, cs: &mut ConstraintSystem, channels: &Channels)
        -> Vec<Box<dyn Fill>>;
}

// TODO: implement when possible
pub struct RecursionISA;

/// The main Instruction Set Architecture for zCray Virtual Machine, supporting
/// all existing instructions.
pub struct GenericISA;

impl ISA for GenericISA {
    fn register_tables(
        &self,
        cs: &mut ConstraintSystem,
        channels: &Channels,
    ) -> Vec<Box<dyn Fill>> {
        vec![
            Box::new(TableEntry {
                table: Box::new(LdiTable::new(cs, channels)),
                get_events: Trace::ldi_events,
            }),
            Box::new(TableEntry {
                table: Box::new(RetTable::new(cs, channels)),
                get_events: Trace::ret_events,
            }),
        ]
    }
}
