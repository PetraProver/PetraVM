use binius_m3::builder::ConstraintSystem;
use zcrayvm_assembly::Opcode;

use crate::{channels::Channels, tables::Table};

pub trait Instruction {
    fn opcode(&self) -> Opcode;

    // TODO(Robin) methods
}

pub trait ISA {
    fn decode(&self, opcode: u8, args: &[u32]) -> Option<Box<dyn Instruction>>;
    fn register_tables(
        &self,
        cs: &mut ConstraintSystem,
        channels: &Channels,
    ) -> Vec<Box<dyn Table>>;
}

pub struct RecursionISA;

impl ISA for RecursionISA {
    fn decode(&self, opcode: u8, args: &[u32]) -> Option<Box<dyn Instruction>> {
        todo!()
    }

    fn register_tables(
        &self,
        cs: &mut ConstraintSystem,
        channels: &Channels,
    ) -> Vec<Box<dyn Table>> {
        todo!()
    }
}
