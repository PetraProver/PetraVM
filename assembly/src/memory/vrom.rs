use std::collections::HashMap;

use binius_field::{BinaryField16b, BinaryField32b};

use super::MemoryError;
use crate::{execution::ZCrayTrace, memory::vrom_allocator::VromAllocator, opcodes::Opcode};

pub(crate) type VromPendingUpdates = HashMap<u32, Vec<VromUpdate>>;

/// Represents the data needed to create a MOVE event later.
pub(crate) type VromUpdate = (
    u32,            // parent addr
    Opcode,         // operation code
    BinaryField32b, // field pc
    u32,            // fp
    u32,            // timestamp
    BinaryField16b, // dst
    BinaryField16b, // src
    BinaryField16b, // offset
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

impl ValueRom {
    /// Creates an empty ValueRom.
    pub fn new(vrom: HashMap<u32, u32>) -> Self {
        Self {
            vrom,
            ..Default::default()
        }
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
        self.check_alignment(index, 2)?;

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
        self.check_alignment(index, 4)?;

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
        self.check_alignment(index, 2)?;

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

    /// Gets a u128 value from the specified index.
    ///
    /// Returns an error if the value is not found. This method should be used
    /// instead of `get_opt_u128` everywhere outside of CALL procedures.
    pub(crate) fn get_u128(&self, index: u32) -> Result<u128, MemoryError> {
        self.check_alignment(index, 4)?;

        // For u128, we need to read from multiple u32 slots (4 slots)
        let mut result: u128 = 0;
        for i in 0..4 {
            let idx = index + i; // Read from consecutive slots

            let word = self.get_u32(idx)?;
            // Shift the value to its appropriate position and add to result
            result += (u128::from(word) << (i * 32));
        }

        Ok(result)
    }

    /// Gets an optional u128 value from the specified index.
    ///
    /// Used for MOVE operations that are part of a CALL procedure, since the
    /// value to move may not yet be known.
    pub(crate) fn get_opt_u128(&self, index: u32) -> Result<Option<u128>, MemoryError> {
        // We need to read four words.
        self.check_alignment(index, 4)?;

        let opt_words = (0..4)
            .map(|i| self.get_opt_u32(index + i).unwrap())
            .collect::<Vec<_>>();
        if opt_words.iter().any(|v| v.is_none()) {
            Ok(None)
        } else {
            let result = opt_words
                .into_iter()
                .enumerate()
                .fold(0u128, |a, (i, opt_w)| {
                    a + ((opt_w.unwrap() as u128) << (32 * i))
                });
            Ok(Some(result))
        }
    }

    /// Allocates a new frame with the specified size.
    pub(crate) fn allocate_new_frame(&mut self, requested_size: u32) -> u32 {
        self.vrom_allocator.alloc(requested_size)
    }

    /// Checks if the index has proper alignment.
    fn check_alignment(&self, index: u32, alignment: u32) -> Result<(), MemoryError> {
        if index % alignment != 0 {
            Err(MemoryError::VromMisaligned(
                alignment.try_into().unwrap(),
                index,
            ))
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
    /// [`BinaryField16b`] for that offset
    #[cfg(test)]
    pub fn set_value_at_offset(&mut self, offset: u16, value: u32) -> BinaryField16b {
        self.set_u32(offset as u32, value).unwrap();
        BinaryField16b::new(offset)
    }
}

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
        assert_eq!(vrom.get_u128(0).unwrap(), u128_val);

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
