mod ram;
mod vrom;
mod vrom_allocator;

use binius_m3::builder::B32;
pub(crate) use ram::Ram;
pub use vrom::ValueRom;
pub(crate) use vrom::{VromLoad, VromPendingUpdates, VromStore, VromUpdate};
pub(crate) use vrom_allocator::VromAllocator;

use crate::execution::InterpreterInstruction;

#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
pub enum MemoryError {
    VromRewrite(u32),
    VromMisaligned(u8, u32),
    VromMissingValue(u32),
    RamAddressOutOfBounds(u32, usize),
    RamMisalignedAccess(u32, usize),
}

pub(crate) trait AccessSize {
    fn byte_size(&self) -> usize;
    fn word_size(&self) -> usize;
    fn for_type<T>() -> Self;
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

    /// Sets a 32-bit value in VROM at the provided index.
    pub(crate) fn set_vrom_u32(&mut self, index: u32, value: u32) -> Result<(), MemoryError> {
        self.vrom.set_u32(index, value)
    }

    /// Sets a 64-bit value in VROM at the provided index.
    pub(crate) fn set_vrom_u64(&mut self, index: u32, value: u64) -> Result<(), MemoryError> {
        self.vrom.set_u64(index, value)
    }

    /// Sets a u128 value and handles pending entries.
    pub(crate) fn set_vrom_u128(&mut self, index: u32, value: u128) -> Result<(), MemoryError> {
        // Set the value in VROM
        self.vrom.set_u128(index, value)
    }

    /// Returns a reference to the pending VROM updates map.
    pub(crate) const fn vrom_pending_updates(&self) -> &VromPendingUpdates {
        &self.vrom.pending_updates
    }

    /// Returns a mutable reference to the pending VROM updates map.
    pub(crate) fn vrom_pending_updates_mut(&mut self) -> &mut VromPendingUpdates {
        &mut self.vrom.pending_updates
    }

    /// Inserts a pending value in VROM to be set later.
    ///
    /// Maps a destination address to a `VromUpdate` which contains necessary
    /// information to create a MOVE event once the value is available.
    pub(crate) fn insert_pending(
        &mut self,
        dst: u32,
        pending_update: VromUpdate,
    ) -> Result<(), MemoryError> {
        self.vrom.insert_pending(dst, pending_update)
    }
}
