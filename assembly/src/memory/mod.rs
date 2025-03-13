mod vrom;
mod vrom_allocator;

pub(crate) use vrom::{ValueRom, VromPendingUpdates, VromUpdate};
pub(crate) use vrom_allocator::VromAllocator;

use crate::InterpreterInstruction;

#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
pub(crate) enum MemoryError {
    VromRewrite(u32),
    VromMisaligned(u8, u32),
    VromMissingValue(u32),
}

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
    pub const fn new(prom: ProgramRom, vrom: ValueRom) -> Self {
        Self { prom, vrom }
    }

    /// Returns a reference to the VROM.
    pub const fn prom(&self) -> &ProgramRom {
        &self.prom
    }

    /// Returns a reference to the VROM.
    pub const fn vrom(&self) -> &ValueRom {
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

    /// Sets a value of any integer type and handles pending entries.
    ///
    /// This generic method works with u8, u16, and u32 values. It stores the
    /// value in VROM and processes any dependent values in `pending_updates`.
    pub(crate) fn set_vrom<T>(
        &mut self,
        index: u32,
        value: T,
    ) -> Result<Option<VromUpdate>, MemoryError>
    where
        T: Copy + Into<u32>,
    {
        // Convert to u32 for storage
        let u32_value: u32 = value.into();

        // Set the value in VROM
        self.vrom.set_value(index, u32_value)?;

        // Handle any pending entry for this index
        if let Some(pending_update) = self.vrom_mut().pending_updates.remove(&index) {
            let parent = pending_update.0;
            /// Set the pending entry
            self.set_vrom(parent, value)?;

            return Ok(Some(pending_update));
        }
        Ok(None)
    }

    /// Sets a u128 value and handles pending entries.
    pub(crate) fn set_vrom_u128(
        &mut self,
        index: u32,
        value: u128,
    ) -> Result<Option<VromUpdate>, MemoryError> {
        // Set the value in VROM
        self.vrom.set_u128(index, value)?;

        // Handle any pending entry for this index
        if let Some(pending_update) = self.vrom_mut().pending_updates.remove(&index) {
            let parent = pending_update.0;
            /// Set the pending entry
            self.set_vrom_u128(parent, value)?;

            return Ok(Some(pending_update));
        }
        Ok(None)
    }

    /// Returns a reference to the pending VROM updates map.
    pub(crate) const fn vrom_pending_updates(&self) -> &VromPendingUpdates {
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
