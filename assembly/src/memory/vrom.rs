use std::collections::HashMap;

use binius_field::{BinaryField16b, BinaryField32b};

use super::MemoryError;
use crate::{memory::VromAllocator, opcodes::Opcode};

/// `ValueRom` represents a memory structure for storing different sized values.
#[derive(Clone, Debug, Default)]
pub(crate) struct ValueRom {
    /// Storage for values, each slot is a u32
    vrom: Vec<u32>,
    /// Allocator for new frames
    vrom_allocator: VromAllocator,
    /// Initial values to populate new frames
    init_values: Vec<u32>,
    /// `HashMap` used to hold pending values to be set when pushing MOVE events
    /// during a CALL procedure.
    ///
    /// When a MOVE occurs with a value that isn't set within a CALL procedure,
    /// we assume it is a return value. Then, we add `(addr_next_frame,
    /// to_set_value)` to `pending_updates`, where `to_set_value` contains
    /// enough information to create a move event later. Whenever an address
    /// in the HashMap's keys is finally set, we populate the missing values
    /// and remove them from the map.
    pub(crate) pending_updates: VromPendingUpdates,
}

pub(crate) type VromPendingUpdates = HashMap<u32, VromUpdate>;

/// Represents the data needed to create a move event later.
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

impl ValueRom {
    /// Creates an empty `ValueRom`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `ValueRom` with initial values.
    pub fn new_with_init_values(vals: Vec<u32>) -> Self {
        Self {
            init_values: vals,
            ..Default::default()
        }
    }

    /// Inserts a pending value to be set later.
    ///
    /// Maps a destination address to a `VromUpdate` which contains necessary
    /// information to create a MOVE event once the value is available.
    pub(crate) fn insert_pending(
        &mut self,
        dst: u32,
        to_set_val: VromUpdate,
    ) -> Result<(), MemoryError> {
        if self.pending_updates.insert(dst, to_set_val).is_some() {
            return Err(MemoryError::VromRewrite(dst));
        }
        Ok(())
    }

    /// Sets a value at the specified index.
    /// The value will be stored as a `u32` regardless of its original size.
    pub(crate) fn set_value<T: Into<u32>>(
        &mut self,
        index: u32,
        value: T,
    ) -> Result<(), MemoryError> {
        self.check_bounds(index)?;

        let u32_value = value.into();
        let prev_val = self.vrom[index as usize];
        if prev_val != 0 && prev_val != u32_value {
            return Err(MemoryError::VromRewrite(index));
        }

        self.vrom[index as usize] = u32_value;
        Ok(())
    }

    /// Sets a `u128` value at the specified index.
    pub(crate) fn set_u128(&mut self, index: u32, value: u128) -> Result<(), MemoryError> {
        self.check_alignment(index, 4)?;

        // For u128, we need to store it across multiple u32 slots (4 slots)
        for i in 0..4 {
            let idx = index + i; // Store in consecutive slots
            let u32_val = (value >> (i * 32)) as u32; // Extract 32-bit chunk directly

            self.check_bounds(idx)?;

            let prev_val = self.vrom[idx as usize];
            if prev_val != 0 && prev_val != u32_val {
                return Err(MemoryError::VromRewrite(idx));
            }

            self.vrom[idx as usize] = u32_val;
        }
        Ok(())
    }

    /// Gets a `u8` value from the specified index.
    pub(crate) fn get_u8(&self, index: u32) -> Result<u8, MemoryError> {
        self.check_bounds(index)?;
        Ok(self.vrom[index as usize] as u8)
    }

    /// Gets a `u16` value from the specified index.
    pub(crate) fn get_u16(&self, index: u32) -> Result<u16, MemoryError> {
        self.check_bounds(index)?;
        Ok(self.vrom[index as usize] as u16)
    }

    /// Gets a `u32` value from the specified index.
    pub(crate) fn get_u32(&self, index: u32) -> Result<u32, MemoryError> {
        self.check_bounds(index)?;
        Ok(self.vrom[index as usize])
    }

    /// Gets a `u128` value from the specified index.
    pub(crate) fn get_u128(&self, index: u32) -> Result<u128, MemoryError> {
        self.check_alignment(index, 4)?;

        // For u128, we need to read from multiple u32 slots (4 slots)
        let mut result: u128 = 0;
        for i in 0..4 {
            let idx = index + i; // Read from consecutive slots
            self.check_bounds(idx)?;

            let u32_val = self.vrom[idx as usize];
            // Shift the value to its appropriate position and add to result
            result |= u128::from(u32_val) << (i * 32);
        }

        Ok(result)
    }

    /// Allocates a new frame with the specified size.
    pub(crate) fn allocate_new_frame(&mut self, requested_size: u32) -> u32 {
        let pos = self.vrom_allocator.alloc(requested_size);

        // Ensure the vector has enough space for the new frame.
        let required_size = (pos + requested_size) as usize;
        if required_size > self.vrom.len() {
            self.vrom.resize(required_size, 0);
        }

        // Copy initial values if available
        if !self.init_values.is_empty() {
            let copy_size = self.init_values.len().min(requested_size as usize);
            for i in 0..copy_size {
                self.vrom[pos as usize + i] = self.init_values[i];
            }
            self.init_values.clear();
        }

        pos
    }

    /// Checks if the index is within bounds.
    fn check_bounds(&self, index: u32) -> Result<(), MemoryError> {
        if index as usize >= self.vrom.len() {
            Err(MemoryError::VromMissingValue(index))
        } else {
            Ok(())
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_with_init_values() {
        let init_values = vec![1, 2, 3, 4];
        let mut vrom = ValueRom::new_with_init_values(init_values.clone());

        // Initial values should be stored but not yet copied
        assert_eq!(vrom.init_values, init_values);
        assert_eq!(vrom.vrom.len(), 0);

        // Allocate a frame
        let pos = vrom.allocate_new_frame(6);
        assert_eq!(pos, 0);
        assert_eq!(vrom.vrom.len(), 6);

        // Check init values were copied
        for (i, &val) in init_values.iter().enumerate() {
            assert_eq!(vrom.vrom[i], val);
        }

        // Init values should be cleared after copying
        assert!(vrom.init_values.is_empty());
    }

    #[test]
    fn test_set_and_get_value() {
        let mut vrom = ValueRom::new();
        vrom.allocate_new_frame(5);

        // Test u8
        let u8_val: u8 = 42;
        vrom.set_value(0, u8_val).unwrap();
        assert_eq!(vrom.get_u8(0).unwrap(), u8_val);
        assert_eq!(vrom.get_u32(0).unwrap(), u8_val as u32);

        // Test u16
        let u16_val: u16 = 12345;
        vrom.set_value(1, u16_val).unwrap();
        assert_eq!(vrom.get_u16(1).unwrap(), u16_val);
        assert_eq!(vrom.get_u32(1).unwrap(), u16_val as u32);

        // Test u32
        let u32_val: u32 = 0xABCDEF12;
        vrom.set_value(2, u32_val).unwrap();
        assert_eq!(vrom.get_u32(2).unwrap(), u32_val);
    }

    #[test]
    fn test_set_and_get_u128() {
        let mut vrom = ValueRom::new();
        vrom.allocate_new_frame(8);

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
        let mut vrom = ValueRom::new();
        vrom.allocate_new_frame(5);

        // First write should succeed
        vrom.set_value(0, 42u32).unwrap();

        // Same value write should succeed (idempotent)
        vrom.set_value(0, 42u32).unwrap();

        // Different value write should fail
        let result = vrom.set_value(0, 43u32);
        assert!(result.is_err());

        if let Err(MemoryError::VromRewrite(index)) = result {
            assert_eq!(index, 0);
        } else {
            panic!("Expected VromRewrite error");
        }
    }

    #[test]
    fn test_u128_rewrite_error() {
        let mut vrom = ValueRom::new();
        vrom.allocate_new_frame(8);

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
        let vrom = ValueRom::new();

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
        let mut vrom = ValueRom::new();
        vrom.allocate_new_frame(8);

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

    #[test]
    fn test_allocate_multiple_frames() {
        let mut vrom = ValueRom::new();

        // Allocate first frame
        let pos1 = vrom.allocate_new_frame(5);
        assert_eq!(pos1, 0);
        assert_eq!(vrom.vrom.len(), 5);

        // Allocate second frame
        let pos2 = vrom.allocate_new_frame(10);
        assert_eq!(pos2, 16);
        assert_eq!(vrom.vrom.len(), 16 + 10);

        // Allocate third frame
        let pos3 = vrom.allocate_new_frame(7);
        assert_eq!(pos3, 8);
        assert_eq!(vrom.vrom.len(), 26);

        // Verify all frames are accessible
        vrom.set_value(0, 100u32).unwrap(); // First frame
        vrom.set_value(16, 200u32).unwrap(); // Second frame
        vrom.set_value(8, 300u32).unwrap(); // Third frame

        assert_eq!(vrom.get_u32(0).unwrap(), 100);
        assert_eq!(vrom.get_u32(16).unwrap(), 200);
        assert_eq!(vrom.get_u32(8).unwrap(), 300);
    }
}
