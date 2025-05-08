use std::{cell::Cell, collections::HashMap, ops::Shl};

use binius_m3::builder::{B16, B32};
use num_traits::Zero;

use super::{AccessSize, MemoryError};
use crate::{memory::vrom_allocator::VromAllocator, opcodes::Opcode};

pub(crate) type VromPendingUpdates = HashMap<u32, Vec<VromUpdate>>;

/// Represents the data needed to create a MOVE event later.
pub(crate) type VromUpdate = (
    u32,    // parent addr
    Opcode, // operation code
    B32,    // field pc
    u32,    // fp
    u32,    // timestamp
    B16,    // dst
    u32,    // dst addr
    B16,    // src
    B16,    // offset
    u32,    // pending update position only used for Mvvl
);

/// `ValueRom` represents a memory structure for storing different sized values.
#[derive(Clone, Debug, Default)]
pub struct ValueRom {
    /// Storage for values, each slot is an `Option<u32>`.
    data: Vec<Option<u32>>,
    /// Number of reads/writes per address (interior mutability).
    access_counts: Vec<Cell<u32>>,
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
    /// Creates an new ValueRom.
    pub fn new(data: Vec<Option<u32>>) -> Self {
        let len = data.len();
        Self {
            data,
            access_counts: vec![Cell::new(0); len],
            vrom_allocator: Default::default(),
            pending_updates: Default::default(),
        }
    }

    pub fn size(&self) -> usize {
        self.vrom_allocator.size()
    }

    /// Creates a default VROM and initializes it with the provided u32 values.
    pub fn new_with_init_vals(init_values: &[u32]) -> Self {
        let data = init_values.iter().copied().map(Some).collect::<Vec<_>>();
        let len = data.len();
        Self {
            data,
            access_counts: vec![Cell::new(0); len],
            vrom_allocator: Default::default(),
            pending_updates: Default::default(),
        }
    }

    /// Generic read method for supported types. This will read a value stored
    /// at the provided index.
    ///
    /// *NOTE*: Do not pass an offset to this function. Call `ctx.addr(offset)`
    /// that will scale the frame pointer with the provided offset to obtain the
    /// corresponding VROM address.
    pub fn read<T: VromValueT>(&self, index: u32) -> Result<T, MemoryError> {
        self.check_alignment::<T>(index)?;
        self.check_bounds::<T>(index)?;
        self.record_access::<T>(index);
        self.read_internal::<T>(index)
    }

    /// Peeks at the value at the given index without recording an access.
    pub fn peek<T: VromValueT>(&self, index: u32) -> Result<T, MemoryError> {
        self.check_alignment::<T>(index)?;
        self.check_bounds::<T>(index)?;
        self.read_internal::<T>(index)
    }

    fn read_internal<T: VromValueT>(&self, index: u32) -> Result<T, MemoryError> {
        let mut value = T::zero();

        // Read the entire chunk at once.
        let read_data = &self.data[index as usize..index as usize + T::word_size()];

        for (i, opt_word) in read_data.iter().enumerate() {
            let word = opt_word.ok_or(MemoryError::VromMissingValue(index))?;

            // Shift the word to its appropriate position and add to the value
            value = value + (T::from(word) << (i * 32));
        }

        Ok(value)
    }

    /// Checks if the value at the given index is set.
    pub fn check_value_set<T: VromValueT>(&self, index: u32) -> Result<bool, MemoryError> {
        self.check_alignment::<T>(index)?;
        if self.check_bounds::<T>(index).is_err() {
            // VROM hasn't been expanded to the target index, there is nothing to read yet.
            return Ok(false);
        };
        let read_data = &self.data[index as usize..index as usize + T::word_size()];
        for &opt_word in read_data {
            if opt_word.is_none() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Generic write method for supported types. Counts each write access per
    /// address.
    ///
    /// *NOTE*: Do not pass an offset to this function. Call `ctx.addr(offset)`
    /// that will scale the frame pointer with the provided offset to obtain the
    /// corresponding VROM address.
    pub fn write<T: VromValueT>(&mut self, index: u32, value: T) -> Result<(), MemoryError> {
        self.check_alignment::<T>(index)?;
        self.ensure_capacity::<T>(index);
        self.record_access::<T>(index);
        for i in 0..T::word_size() {
            let cur_word = (value.to_u128() >> (32 * i)) as u32;
            let prev_value = &mut self.data[index as usize + i];
            if let Some(prev_val) = prev_value {
                // The VROM is write-once. If a value already exists at `index`,
                // check that it matches the value we wanted to write.
                if *prev_val != cur_word {
                    return Err(MemoryError::VromRewrite(index, *prev_val, cur_word));
                }
            } else {
                // The VROM hasn't been updated yet at the provided `index`.
                *prev_value = Some(cur_word);
            }
        }

        Ok(())
    }

    /// Allocates a new frame with the specified size.
    pub(crate) fn allocate_new_frame(&mut self, requested_size: u32) -> u32 {
        let res = self.vrom_allocator.alloc(requested_size);
        self.ensure_capacity::<u32>(self.vrom_allocator.size() as u32);
        res
    }

    /// Ensures the VROM has enough capacity for an access, resizing if
    /// necessary.
    fn ensure_capacity<T: VromValueT>(&mut self, addr: u32) {
        let required_size = addr as usize + T::word_size();
        if required_size > self.data.len() {
            let new_len = required_size.next_power_of_two();
            self.data.resize(new_len, None);
            self.access_counts.resize(new_len, Cell::new(0));
        }
    }

    /// Checks if the index has proper alignment.
    fn check_alignment<T: AccessSize>(&self, index: u32) -> Result<(), MemoryError> {
        if index as usize % T::word_size() != 0 {
            Err(MemoryError::VromMisaligned(T::word_size() as u8, index))
        } else {
            Ok(())
        }
    }

    /// Checks if an address is within the current bounds of VROM.
    fn check_bounds<T: AccessSize>(&self, addr: u32) -> Result<(), MemoryError> {
        let end_addr = addr as usize + T::word_size();

        if end_addr > self.data.len() {
            return Err(MemoryError::VromAddressOutOfBounds(addr, T::word_size()));
        }

        Ok(())
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
        self.write(offset as u32, value).unwrap();
        B16::new(offset)
    }

    /// Record accesses for addresses [addr, addr+size).
    pub(crate) fn record_access<T: VromValueT>(&self, addr: u32) {
        for i in 0..T::word_size() {
            let idx = addr as usize + i;
            let count = self.access_counts[idx].get();
            self.access_counts[idx].set(count + 1);
        }
    }

    /// Returns a vector of (addr, value, access_count) sorted by access_count
    /// descending.
    pub fn sorted_access_counts(&self) -> Vec<(u32, u32, u32)> {
        let mut entries: Vec<(u32, u32, u32)> = self
            .access_counts
            .iter()
            .enumerate()
            .filter_map(|(idx, cell)| {
                let count = cell.get();
                if count > 0 {
                    self.data[idx].map(|val| (idx as u32, val, count))
                } else {
                    None
                }
            })
            .collect();
        entries.sort_by(|a, b| b.2.cmp(&a.2));
        entries
    }
}

/// Trait for types that can be read from or written to the VROM.
pub trait VromValueT:
    Copy + Default + Zero + Shl<usize, Output = Self> + Sized + From<u32> + AccessSize
{
    fn to_u128(self) -> u128;
}

impl VromValueT for u32 {
    fn to_u128(self) -> u128 {
        self as u128
    }
}
impl VromValueT for u64 {
    fn to_u128(self) -> u128 {
        self as u128
    }
}
impl VromValueT for u128 {
    fn to_u128(self) -> u128 {
        self
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
        vrom.write(2, u32_val).unwrap();
        assert_eq!(vrom.read::<u32>(2).unwrap(), u32_val);
    }

    #[test]
    fn test_set_and_get_u128() {
        let mut vrom = ValueRom::default();

        let u128_val: u128 = 0x1122334455667788_99AABBCCDDEEFF00;
        vrom.write(0, u128_val).unwrap();

        // Check that the value was stored correctly
        assert_eq!(vrom.read::<u128>(0).unwrap(), u128_val);

        // Check individual u32 components (first is least significant)
        assert_eq!(vrom.read::<u32>(0).unwrap(), 0xDDEEFF00);
        assert_eq!(vrom.read::<u32>(1).unwrap(), 0x99AABBCC);
        assert_eq!(vrom.read::<u32>(2).unwrap(), 0x55667788);
        assert_eq!(vrom.read::<u32>(3).unwrap(), 0x11223344);

        vrom.read::<u32>(3).unwrap();
        vrom.read::<u32>(2).unwrap();
        vrom.read::<u32>(2).unwrap();

        let vrom_access_counts = vrom.sorted_access_counts();
        assert_eq!(vrom_access_counts.len(), 4);
        assert_eq!(vrom_access_counts[0], (2, 0x55667788, 5));
        assert_eq!(vrom_access_counts[1], (3, 0x11223344, 4));
        assert_eq!(vrom_access_counts[2], (0, 0xDDEEFF00, 3));
        assert_eq!(vrom_access_counts[3], (1, 0x99AABBCC, 3));
    }

    #[test]
    fn test_value_rewrite_error() {
        let mut vrom = ValueRom::default();
        // First write should succeed
        vrom.write(0, 42u32).unwrap();

        // Same value write should succeed (idempotent)
        vrom.write(0, 42u32).unwrap();

        // Different value write should fail
        let result = vrom.write(0, 43u32);
        assert!(result.is_err());

        if let Err(MemoryError::VromRewrite(index, old, new)) = result {
            assert_eq!(index, 0);
            assert_eq!(old, 42);
            assert_eq!(new, 43);
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
        vrom.write(0, u128_val_1).unwrap();

        // Same value write should succeed (idempotent)
        vrom.write(0, u128_val_1).unwrap();

        // Different value write should fail at the first different 32-bit chunk
        let result = vrom.write(0, u128_val_2);
        assert!(result.is_err());

        if let Err(MemoryError::VromRewrite(index, old, new)) = result {
            assert_eq!(index, 0); // The least significant 32-bit chunk differs
            assert_eq!(old, u128_val_1 as u32);
            assert_eq!(new, u128_val_2 as u32);
        } else {
            panic!("Expected VromRewrite error");
        }
    }

    #[test]
    fn test_missing_value_error() {
        let vrom = ValueRom::default();

        // Try to get a value from an empty VROM
        let result = vrom.read::<u32>(0);
        assert!(result.is_err());

        if let Err(MemoryError::VromAddressOutOfBounds(index, word_size)) = result {
            assert_eq!(index, 0);
            assert_eq!(word_size, u32::word_size());
        } else {
            panic!("Expected VromMissingValue error");
        }
    }

    #[test]
    fn test_u128_misaligned_error() {
        let mut vrom = ValueRom::default();
        // Try to set a u128 at a misaligned index
        let result = vrom.write(1, 0u128);
        assert!(result.is_err());

        if let Err(MemoryError::VromMisaligned(alignment, index)) = result {
            assert_eq!(alignment, 4);
            assert_eq!(index, 1);
        } else {
            panic!("Expected VromMisaligned error");
        }
    }
}
