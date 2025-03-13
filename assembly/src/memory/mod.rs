mod error;
mod vrom;
mod vrom_allocator;

pub(crate) use error::MemoryError;
pub(crate) use vrom::{ValueRom, VromPendingUpdates, VromUpdate};
pub(crate) use vrom_allocator::VromAllocator;

use crate::InterpreterInstruction;

/// The Program ROM, or Instruction Memory, is an immutable memory where code is
/// loaded. It maps every PC to a specific instruction to execute.
pub type ProgramRom = Vec<InterpreterInstruction>;

/// The `Memory` for an execution contains an *immutable* Program ROM and a
/// *mutable* Value ROM.
#[derive(Debug, Default)]
pub struct Memory {
    prom: ProgramRom,
    vrom: ValueRom,
    // TODO: Add RAM
}

impl Memory {
    /// Initializes a new `Memory` instance.
    pub fn new(prom: ProgramRom, vrom: ValueRom) -> Self {
        Self { prom, vrom }
    }

    /// Returns a reference to the VROM.
    pub fn prom(&self) -> &ProgramRom {
        &self.prom
    }

    /// Returns a reference to the VROM.
    pub fn vrom(&self) -> &ValueRom {
        &self.vrom
    }

    /// Returns a mutable reference to the VROM.
    pub fn vrom_mut(&mut self) -> &mut ValueRom {
        &mut self.vrom
    }

    /// Reads a 32-bit value in VROM at the provided index.
    pub(crate) fn get_vrom_u32(&self, index: u32) -> Result<u32, MemoryError> {
        self.vrom.get_u32(index)
    }

    /// Reads a 128-bit value in VROM at the provided index.
    pub(crate) fn get_vrom_u128(&self, index: u32) -> Result<u128, MemoryError> {
        self.vrom.get_u128(index)
    }

    /// Returns a reference to the pending VROM updates map.
    pub(crate) fn vrom_pending_updates(&self) -> &VromPendingUpdates {
        &self.vrom.pending_updates
    }

    /// Returns a mutable reference to the pending VROM updates map.
    pub(crate) fn vrom_pending_updates_mut(&mut self) -> &VromPendingUpdates {
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

    /// Attempts to get a `u32` value from VROM, returning `None` if the value
    /// is pending.
    ///
    /// This method is used in MOVE operations to determine if a value is
    /// available or still unset.
    pub(crate) fn get_vrom_u32_move(&self, index: u32) -> Result<Option<u32>, MemoryError> {
        if self.vrom.pending_updates.contains_key(&index) {
            // Value is pending, not available yet
            Ok(None)
        } else {
            // Try to get the value from VROM
            match self.get_vrom_u32(index) {
                Ok(value) => Ok(Some(value)),
                Err(e) => Err(e),
            }
        }
    }

    /// Attempts to get a `u128` value from VROM, returning `None` if the value
    /// is pending.
    ///
    /// This method is used in MOVE operations to determine if a value is
    /// available or still unset.
    pub(crate) fn get_vrom_u128_move(&self, index: u32) -> Result<Option<u128>, MemoryError> {
        if self.vrom.pending_updates.contains_key(&index) {
            // Value is pending, not available yet
            Ok(None)
        } else {
            // Try to get the value from VROM
            match self.vrom.get_u128(index) {
                Ok(value) => Ok(Some(value)),
                Err(e) => Err(e),
            }
        }
    }
}
