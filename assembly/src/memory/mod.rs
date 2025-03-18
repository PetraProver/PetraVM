mod ram;
mod vrom;
mod vrom_allocator;

use binius_field::BinaryField32b;
pub(crate) use ram::{AccessSize, Ram};
pub use vrom::ValueRom;
pub(crate) use vrom::{VromPendingUpdates, VromUpdate};
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

    // ValueROM access methods

    /// Reads a 32-bit value in VROM at the provided index.
    ///
    /// Returns an error if the value is not found. This method should be used
    /// instead of `get_vrom_opt_u32` everywhere outside of CALL procedures.
    pub(crate) fn get_vrom_u32(&self, index: u32) -> Result<u32, MemoryError> {
        self.vrom.get_u32(index)
    }

    /// Reads an optional 32-bit value in VROM at the provided index.
    ///
    /// Used for MOVE operations that are part of a CALL procedure, since the
    /// value to move may not yet be known.
    pub(crate) fn get_vrom_opt_u32(&self, index: u32) -> Result<Option<u32>, MemoryError> {
        self.vrom.get_vrom_opt_u32(index)
    }

    /// Reads a 128-bit value in VROM at the provided index.
    ///
    /// Returns an error if the value is not found. This method should be used
    /// instead of `get_vrom_opt_u128` everywhere outside of CALL procedures.
    pub(crate) fn get_vrom_u128(&self, index: u32) -> Result<u128, MemoryError> {
        self.vrom.get_u128(index)
    }

    /// Reads an optional 128-bit value in VROM at the provided index.
    ///
    /// Used for MOVE operations that are part of a CALL procedure, since the
    /// value to move may not yet be known.
    pub(crate) fn get_vrom_opt_u128(&self, index: u32) -> Result<Option<u128>, MemoryError> {
        self.vrom.get_vrom_opt_u128(index)
    }

    /// Sets a 32-bit value in VROM at the provided index.
    pub(crate) fn set_vrom_u32(&mut self, index: u32, value: u32) -> Result<(), MemoryError> {
        self.vrom.set_u32(index, value)
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
