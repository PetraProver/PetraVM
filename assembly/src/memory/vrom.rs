use std::{collections::HashMap, ops::Shl};

use binius_m3::builder::{B16, B32};
use num_traits::Zero;

use super::{AccessSize, MemoryError};
use crate::{
    event::context::EventContext, execution::ZCrayTrace, memory::vrom_allocator::VromAllocator,
    opcodes::Opcode,
};

pub(crate) type VromPendingUpdates = HashMap<u32, Vec<VromUpdate>>;

/// Represents the data needed to create a MOVE event later.
pub(crate) type VromUpdate = (
    u32,    // parent addr
    Opcode, // operation code
    B32,    // field pc
    u32,    // fp
    u32,    // timestamp
    B16,    // dst
    B16,    // src
    B16,    // offset
);

/// `ValueRom` represents a memory structure for storing different sized values.
#[derive(Clone, Debug, Default)]
pub struct ValueRom {
    /// Storage for values, each slot is a u32
    vrom: HashMap<u32, u32>,
    /// Allocator for new frames
    vrom_allocator: VromAllocator,
    /// HashMap used to set values and push MV events during a CALL procedure.
    /// When a MV occurs with a value that isn't set within a CALL procedure, we
    /// assume it is a return value. Then, we add (addr_next_frame,
    /// pending_update) to `pending_updates`, where `pending_update` contains
    /// enough information to create a MOVE event later. Whenever an address
    /// in the HashMap's keys is finally set, we populate the missing values
    /// and remove them from the HashMap.
    pub(crate) pending_updates: VromPendingUpdates,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VromAccessSize {
    U32 = 1,
    U64 = 2,
    U128 = 4,
}

impl AccessSize for VromAccessSize {
    fn byte_size(&self) -> usize {
        unimplemented!("VROM accesses are done at the word level.")
    }

    fn word_size(&self) -> usize {
        *self as usize
    }

    fn for_type<T>() -> Self {
        match size_of::<T>() {
            4 => VromAccessSize::U32,
            8 => VromAccessSize::U64,
            16 => VromAccessSize::U128,
            _ => panic!("Unsupported type size for VROM access"),
        }
    }
}

impl ValueRom {
    /// Creates an empty ValueRom.
    pub fn new(vrom: HashMap<u32, u32>) -> Self {
        Self {
            vrom,
            ..Default::default()
        }
    }

    pub fn size(&self) -> usize {
        self.vrom_allocator.size()
    }

    /// Creates a default VROM and intializes it with the provided u32 values.
    pub fn new_with_init_vals(init_values: &[u32]) -> Self {
        let mut vrom = Self::default();
        for (i, &value) in init_values.iter().enumerate() {
            vrom.set_u32(i as u32, value).unwrap();
        }

        vrom
    }

    /// Used for memory initialization before the start of the trace generation.
    ///
    /// Initializes a u32 value in the VROM without checking whether there are
    /// associated values in `pending_updates`.
    pub(crate) fn set_u32(&mut self, index: u32, value: u32) -> Result<(), MemoryError> {
        if let Some(prev_val) = self.vrom.insert(index, value) {
            if prev_val != value {
                return Err(MemoryError::VromRewrite(index));
            }
        }

        Ok(())
    }

    /// Used for memory initialization before the start of the trace generation.
    ///
    /// Initializes a u64 value in the VROM without checking whether there are
    /// associated values in `pending_updates`.
    pub(crate) fn set_u64(&mut self, index: u32, value: u64) -> Result<(), MemoryError> {
        self.check_alignment(index, AccessSize::for_type::<u64>())?;

        for i in 0..2 {
            let cur_word = (value >> (32 * i)) as u32;
            if let Some(prev_val) = self.vrom.insert(index + i, cur_word) {
                if prev_val != cur_word {
                    return Err(MemoryError::VromRewrite(index));
                }
            }
        }

        Ok(())
    }

    /// Used for memory initialization before the start of the trace generation.
    ///
    /// Initializes a u32 value in the VROM without checking whether there are
    /// associated values in `pending_updates`.
    pub(crate) fn set_u128(&mut self, index: u32, value: u128) -> Result<(), MemoryError> {
        self.check_alignment(index, AccessSize::for_type::<u128>())?;

        for i in 0..4 {
            let cur_word = (value >> (32 * i)) as u32;
            if let Some(prev_val) = self.vrom.insert(index + i, cur_word) {
                if prev_val != cur_word {
                    return Err(MemoryError::VromRewrite(index));
                }
            }
        }

        Ok(())
    }

    /// Gets a u32 value from the specified index.
    ///
    /// Returns an error if the value is not found. This method should be used
    /// instead of `get_opt_u32` everywhere outside of CALL procedures.
    pub(crate) fn get_u32(&self, index: u32) -> Result<u32, MemoryError> {
        match self.vrom.get(&index) {
            Some(&value) => Ok(value),
            None => Err(MemoryError::VromMissingValue(index)),
        }
    }

    /// Gets an optional u32 value from the specified index.
    ///
    /// Used for MOVE operations that are part of a CALL procedure, since the
    /// value to move may not yet be known.
    pub(crate) fn get_opt_u32(&self, index: u32) -> Result<Option<u32>, MemoryError> {
        Ok(self.vrom.get(&index).copied())
    }

    /// Gets a u64 value from the specified index.
    ///
    /// Returns an error if the value is not found. This method should be used
    /// instead of `get_vrom_opt_u128` everywhere outside of CALL procedures.
    pub(crate) fn get_u64(&self, index: u32) -> Result<u64, MemoryError> {
        self.check_alignment(index, AccessSize::for_type::<u64>())?;

        // For u64, we need to read from multiple u32 slots (2 slots)
        let mut result: u64 = 0;
        for i in 0..2 {
            let idx = index + i; // Read from consecutive slots

            let word = self.get_u32(idx)?;
            // Shift the value to its appropriate position and add to result
            result += (u64::from(word) << (i * 32));
        }

        Ok(result)
    }

    /// Generic read method for supported types.
    ///
    /// *NOTE*: Do not pass an offset to this function. Call `ctx.addr(offset)`
    /// that will scale the frame pointer with the provided offset to obtain the
    /// corresponding VROM address.
    pub fn read<T: VromValue>(&self, index: u32) -> Result<T, MemoryError> {
        let access_size = AccessSize::for_type::<T>();
        self.check_alignment(index, access_size)?;

        let mut value = T::zero();

        for i in 0..access_size.word_size() {
            let idx = index + i as u32; // Read from consecutive slots

            let word = self.get_u32(idx)?;
            // Shift the word to its appropriate position and add to the value
            value = value + (T::from(word) << (i * 32));
        }

        Ok(value)
    }

    /// Fallible version of the `Self::read` method, to account for values yet
    /// to be set.
    ///
    /// *NOTE*: Do not pass an offset to this function. Call `ctx.addr(offset)`
    /// that will scale the frame pointer with the provided offset to obtain the
    /// corresponding VROM address.
    pub fn read_opt<T: VromValue>(&self, index: u32) -> Result<Option<T>, MemoryError> {
        let access_size = AccessSize::for_type::<T>();
        self.check_alignment(index, access_size)?;

        let mut value = T::zero();

        for i in 0..access_size.word_size() {
            let idx = index + i as u32; // Read from consecutive slots

            let word = self.get_opt_u32(idx)?;

            if let Some(v) = word {
                // Shift the word to its appropriate position and add to the value
                value = value + (T::from(v) << (i * 32));
            } else {
                return Ok(None);
            }
        }

        Ok(Some(value))
    }

    /// Allocates a new frame with the specified size.
    pub(crate) fn allocate_new_frame(&mut self, requested_size: u32) -> u32 {
        self.vrom_allocator.alloc(requested_size)
    }

    /// Checks if the index has proper alignment.
    fn check_alignment(&self, index: u32, size: VromAccessSize) -> Result<(), MemoryError> {
        if index as usize % size.word_size() != 0 {
            Err(MemoryError::VromMisaligned(size.word_size() as u8, index))
        } else {
            Ok(())
        }
    }

    /// Inserts a pending value to be set later.
    ///
    /// Maps a destination address to a `VromUpdate` which contains necessary
    /// information to create a MOVE event once the value is available.
    pub(crate) fn insert_pending(
        &mut self,
        parent: u32,
        pending_value: VromUpdate,
    ) -> Result<(), MemoryError> {
        self.pending_updates
            .entry(parent)
            .or_default()
            .push(pending_value);

        Ok(())
    }

    /// Helper method to set a value at the given VROM offset and returns a
    /// [`B16`] for that offset.
    #[cfg(test)]
    pub fn set_value_at_offset(&mut self, offset: u16, value: u32) -> B16 {
        self.set_u32(offset as u32, value).unwrap();
        B16::new(offset)
    }
}

/// Trait for storing values in the VROM.
///
/// It abstracts over the different data types that can be written into the VROM
/// during program execution.
pub(crate) trait VromStore {
    /// Stores the given `value` at the specified `addr` in the VROM.
    ///
    /// # Arguments
    /// * `ctx` - The current [`EventContext`].
    /// * `addr` - The target VROM address.
    /// * `value` - The value to store.
    ///
    /// *NOTE*: Do not pass an offset to this function. Call `ctx.addr(offset)`
    /// that will scale the frame pointer with the provided offset to obtain the
    /// corresponding VROM address.
    fn store(&self, ctx: &mut EventContext, addr: u32) -> Result<(), MemoryError>;
}

impl VromStore for u16 {
    // *NOTE*: This will be stored as a `u32`.
    fn store(&self, ctx: &mut EventContext, addr: u32) -> Result<(), MemoryError> {
        ctx.store_vrom_u32(addr, *self as u32)
    }
}

impl VromStore for u32 {
    fn store(&self, ctx: &mut EventContext, addr: u32) -> Result<(), MemoryError> {
        ctx.store_vrom_u32(addr, *self)
    }
}

impl VromStore for u64 {
    fn store(&self, ctx: &mut EventContext, addr: u32) -> Result<(), MemoryError> {
        ctx.trace.set_vrom_u64(addr, *self)
    }
}

impl VromStore for u128 {
    fn store(&self, ctx: &mut EventContext, addr: u32) -> Result<(), MemoryError> {
        ctx.trace.set_vrom_u128(addr, *self)
    }
}

/// Trait for types that can be read from or written to the VROM.
pub trait VromValue: Copy + Zero + Shl<usize, Output = Self> + Sized + From<u32> {}

impl VromValue for u32 {}
impl VromValue for u64 {}
impl VromValue for u128 {}

/// Trait for loading values from the VROM of the zCrayVM.
///
/// It abstracts over the different data types that can be read from the VROM
/// during program execution, and provides both strict and fallible
/// variants of the load operation, to account for values yet to be set.
pub(crate) trait VromLoad: VromValue {
    /// Loads a value from VROM at the given address.
    ///
    /// # Arguments
    /// * `ctx` - The current event context.
    /// * `addr` - The memory address to read from.
    ///
    /// *NOTE*: Do not pass an offset to this function. Call `ctx.addr(offset)`
    /// /// that will scale the frame pointer with the provided offset to obtain
    /// the corresponding VROM address.
    fn load(ctx: &EventContext, addr: u32) -> Result<Self, MemoryError> {
        ctx.trace.vrom().read::<Self>(addr)
    }

    /// Fallible version of [`Self::load`].
    fn load_opt(ctx: &EventContext, addr: u32) -> Result<Option<Self>, MemoryError> {
        ctx.trace.vrom().read_opt::<Self>(addr)
    }
}

impl VromLoad for u32 {}
impl VromLoad for u64 {}
impl VromLoad for u128 {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get_value() {
        let mut vrom = ValueRom::default();

        // Test u32
        let u32_val: u32 = 0xABCDEF12;
        vrom.set_u32(2, u32_val).unwrap();
        assert_eq!(vrom.get_u32(2).unwrap(), u32_val);
    }

    #[test]
    fn test_set_and_get_u128() {
        let mut vrom = ValueRom::default();

        let u128_val: u128 = 0x1122334455667788_99AABBCCDDEEFF00;
        vrom.set_u128(0, u128_val).unwrap();

        // Check that the value was stored correctly
        assert_eq!(vrom.read::<u128>(0).unwrap(), u128_val);

        // Check individual u32 components (first is least significant)
        assert_eq!(vrom.get_u32(0).unwrap(), 0xDDEEFF00);
        assert_eq!(vrom.get_u32(1).unwrap(), 0x99AABBCC);
        assert_eq!(vrom.get_u32(2).unwrap(), 0x55667788);
        assert_eq!(vrom.get_u32(3).unwrap(), 0x11223344);
    }

    #[test]
    fn test_value_rewrite_error() {
        let mut vrom = ValueRom::default();
        // First write should succeed
        vrom.set_u32(0, 42u32).unwrap();

        // Same value write should succeed (idempotent)
        vrom.set_u32(0, 42u32).unwrap();

        // Different value write should fail
        let result = vrom.set_u32(0, 43u32);
        assert!(result.is_err());

        if let Err(MemoryError::VromRewrite(index)) = result {
            assert_eq!(index, 0);
        } else {
            panic!("Expected VromRewrite error");
        }
    }

    #[test]
    fn test_u128_rewrite_error() {
        let mut vrom = ValueRom::default();
        let u128_val_1: u128 = 0x1122334455667788_99AABBCCDDEEFF00;
        let u128_val_2: u128 = 0x1122334455667788_99AABBCCDDEEFF01; // One bit different

        // First write should succeed
        vrom.set_u128(0, u128_val_1).unwrap();

        // Same value write should succeed (idempotent)
        vrom.set_u128(0, u128_val_1).unwrap();

        // Different value write should fail at the first different 32-bit chunk
        let result = vrom.set_u128(0, u128_val_2);
        assert!(result.is_err());

        if let Err(MemoryError::VromRewrite(index)) = result {
            assert_eq!(index, 0); // The least significant 32-bit chunk differs
        } else {
            panic!("Expected VromRewrite error");
        }
    }

    #[test]
    fn test_missing_value_error() {
        let vrom = ValueRom::default();

        // Try to get a value from an empty VROM
        let result = vrom.get_u32(0);
        assert!(result.is_err());

        if let Err(MemoryError::VromMissingValue(index)) = result {
            assert_eq!(index, 0);
        } else {
            panic!("Expected VromMissingValue error");
        }
    }

    #[test]
    fn test_u128_misaligned_error() {
        let mut vrom = ValueRom::default();
        // Try to set a u128 at a misaligned index
        let result = vrom.set_u128(1, 0);
        assert!(result.is_err());

        if let Err(MemoryError::VromMisaligned(alignment, index)) = result {
            assert_eq!(alignment, 4);
            assert_eq!(index, 1);
        } else {
            panic!("Expected VromMisaligned error");
        }
    }
}
