use binius_m3::builder::ConstraintSystem;

use crate::{
    channels::Channels,
    tables::{LdiTable, RetTable, Table},
};

// TODO(Robin): Maybe create some `VirtualMachine` object containing on the
// `assembly` side containing both the `ISA` and the `EventContext`?

pub trait ISA {
    // TODO(Robin) should it support some decode method to catch unsupported
    // instructions during emulation?

    fn register_tables(
        &self,
        cs: &mut ConstraintSystem,
        channels: &Channels,
    ) -> Vec<Box<dyn Table>>;
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
    ) -> Vec<Box<dyn Table>> {
        let ldi_table = LdiTable::new(cs, &channels);
        let ret_table = RetTable::new(cs, &channels);

        vec![Box::new(ldi_table), Box::new(ret_table)]
    }
}
