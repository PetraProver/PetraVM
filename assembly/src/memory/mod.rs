mod ram;
pub mod vrom;
pub mod vrom_allocator;

pub(crate) use ram::{Ram, RamValueT};
use strum_macros::Display;
pub use vrom::ValueRom;
pub(crate) use vrom::{VromPendingUpdates, VromUpdate, VromValueT};

use crate::execution::InterpreterInstruction;

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Display)]
pub enum MemoryError {
    VromRewrite(u32, u32, u32),
    VromMisaligned(u8, u32),
    VromMissingValue(u32),
    VromAddressOutOfBounds(u32, usize),
    RamAddressOutOfBounds(u32, usize),
    RamMisalignedAccess(u32, usize),
}

/// Trait that defines access granularity in memory, like word size (e.g., u32,
/// u128). Can be used to determine how many 32-bit words are required.
pub trait AccessSize {
    fn byte_size() -> usize;
    fn word_size() -> usize;
}

impl AccessSize for u8 {
    fn byte_size() -> usize {
        1
    }

    fn word_size() -> usize {
        1 // TODO: Should it panic instead?
    }
}

impl AccessSize for u16 {
    fn byte_size() -> usize {
        2
    }

    fn word_size() -> usize {
        1 // TODO: Should it panic instead?
    }
}

impl AccessSize for u32 {
    fn byte_size() -> usize {
        4
    }

    fn word_size() -> usize {
        1
    }
}

impl AccessSize for u64 {
    fn byte_size() -> usize {
        8
    }

    fn word_size() -> usize {
        2
    }
}

impl AccessSize for u128 {
    fn byte_size() -> usize {
        16
    }

    fn word_size() -> usize {
        4
    }
}

/// The Program ROM, or Instruction Memory, is an immutable memory where code is
/// loaded. It maps every PC to a specific instruction to execute.
pub type ProgramRom = Vec<InterpreterInstruction>;

/// The `Memory` for an execution contains an *immutable* Program ROM,
/// and a *mutable* Value ROM.
#[derive(Debug, Default)]
pub struct Memory {
    prom: ProgramRom,
    vrom: ValueRom,
    // TODO: We won't need to implement RAM ops at all for the first version.
}

impl Memory {
    /// Initializes a new `Memory` instance.
    pub const fn new(prom: ProgramRom, vrom: ValueRom) -> Self {
        Self { prom, vrom }
    }

    /// Returns a reference to the PROM.
    pub const fn prom(&self) -> &ProgramRom {
        &self.prom
    }

    /// Returns a reference to the VROM.
    pub const fn vrom(&self) -> &ValueRom {
        &self.vrom
    }

    /// Returns a mutable reference to the VROM.
    pub(crate) fn vrom_mut(&mut self) -> &mut ValueRom {
        &mut self.vrom
    }

    /// Returns a reference to the RAM.
    pub const fn ram(&self) -> &Ram {
        todo!()
    }

    /// Returns a mutable reference to the RAM.
    pub fn ram_mut(&mut self) -> &mut Ram {
        todo!()
    }
    #[cfg(test)]
    /// Returns a reference to the pending VROM updates map.
    pub(crate) const fn vrom_pending_updates(&self) -> &VromPendingUpdates {
        &self.vrom.pending_updates
    }

    /// Returns a mutable reference to the pending VROM updates map.
    pub(crate) fn vrom_pending_updates_mut(&mut self) -> &mut VromPendingUpdates {
        &mut self.vrom.pending_updates
    }
}
