use binius_m3::builder::B32;

use super::AccessSize;
use crate::memory::MemoryError;

/// Represents the RAM for the zCrayVM
#[derive(Debug, Clone)]
pub struct Ram {
    /// The actual RAM data
    data: Vec<u8>,
    /// History of RAM accesses for trace generation
    access_history: Vec<RamAccessEvent>,
}

/// Minimum RAM size in bytes (1KB)
pub const MIN_RAM_SIZE: usize = 1024;

/// The concrete type of accessed RAM values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RamValue {
    /// 8-bit value
    Byte(u8),
    /// 16-bit value
    HalfWord(u16),
    /// 32-bit value
    Word(u32),
}

/// Represents a RAM access event for tracing/proving
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RamAccessEvent {
    /// Address at which the RAM access happened.
    pub address: u32,
    /// Value read/written in RAM at `address`
    pub value: RamValue,
    /// Value previously stored at `address`.
    /// This will be `None` for READ operations and `Some(v)` for WRITE ones.
    pub previous_value: Option<RamValue>,
    /// Timestamp at which the RAM access happened.
    pub timestamp: u32,
    /// Program Counter at which the RAM access happened.
    pub pc: B32,
    /// Type of the RAM access, i.e. whether READ or WRITE.
    pub is_write: bool,
}

/// Trait for types that can be read from or written to the RAM.
pub trait RamValueT: Copy + Default + Sized + AccessSize {
    /// Converts a slice of bytes into a [`RamValueT`].
    ///
    /// *NOTE*: This will panic if the provided slice is too small.
    fn from_le_bytes(bytes: &[u8]) -> Self;

    fn to_le_bytes(self) -> Vec<u8>;

    /// Converts this [`RamValueT`] into a concrete [`RamValue`].
    fn value(self) -> RamValue;
}

impl RamValueT for u8 {
    fn from_le_bytes(bytes: &[u8]) -> Self {
        assert!(!bytes.is_empty());
        bytes[0]
    }

    fn to_le_bytes(self) -> Vec<u8> {
        vec![self]
    }

    fn value(self) -> RamValue {
        RamValue::Byte(self)
    }
}

impl RamValueT for u16 {
    fn from_le_bytes(bytes: &[u8]) -> Self {
        assert!(bytes.len() >= 2);
        u16::from_le_bytes([bytes[0], bytes[1]])
    }

    fn to_le_bytes(self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }

    fn value(self) -> RamValue {
        RamValue::HalfWord(self)
    }
}

impl RamValueT for u32 {
    fn from_le_bytes(bytes: &[u8]) -> Self {
        assert!(bytes.len() >= 4);
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    fn to_le_bytes(self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }

    fn value(self) -> RamValue {
        RamValue::Word(self)
    }
}

impl Default for Ram {
    fn default() -> Self {
        Self::new(MIN_RAM_SIZE)
    }
}

impl Ram {
    /// Creates a new RAM with initial capacity (rounded up to the next power of
    /// two)
    pub fn new(initial_capacity: usize) -> Self {
        // Ensure initial capacity is at least MIN_RAM_SIZE and a power of two
        let capacity = if initial_capacity < MIN_RAM_SIZE {
            MIN_RAM_SIZE
        } else {
            initial_capacity.next_power_of_two()
        };

        Self {
            data: vec![0; capacity],
            access_history: Vec::new(),
        }
    }

    pub fn capacity(&self) -> usize {
        self.data.len()
    }

    pub fn access_history(&self) -> &[RamAccessEvent] {
        &self.access_history
    }

    /// Ensures RAM has enough capacity for an access, resizing if necessary.
    fn ensure_capacity<T: AccessSize>(&mut self, addr: u32) {
        let required_size = addr as usize + T::byte_size();
        if required_size > self.data.len() {
            self.data.resize(required_size.next_power_of_two(), 0);
        }
    }

    /// Checks if an access is properly aligned
    fn check_alignment<T: AccessSize>(&self, addr: u32) -> Result<(), MemoryError> {
        let addr_usize = addr as usize;

        if addr_usize % T::byte_size() != 0 {
            return Err(MemoryError::RamMisalignedAccess(addr, T::byte_size()));
        }

        Ok(())
    }

    /// Checks if an address is within the current bounds of RAM.
    fn check_bounds<T: AccessSize>(&self, addr: u32) -> Result<(), MemoryError> {
        let end_addr = addr as usize + T::byte_size();

        if end_addr > self.data.len() {
            return Err(MemoryError::RamAddressOutOfBounds(addr, T::byte_size()));
        }

        Ok(())
    }

    /// Generic read method for supported types (u8, u16, u32)
    pub fn read<T: RamValueT>(
        &mut self,
        addr: u32,
        timestamp: u32,
        pc: B32,
    ) -> Result<T, MemoryError> {
        self.check_alignment::<T>(addr)?;
        self.check_bounds::<T>(addr)?;

        let addr_usize = addr as usize;
        let end_addr = addr_usize + T::byte_size();
        let value = T::from_le_bytes(&self.data[addr_usize..end_addr]);

        self.access_history.push(RamAccessEvent {
            address: addr,
            value: value.value(),
            previous_value: None,
            timestamp,
            pc,
            is_write: false,
        });

        Ok(value)
    }

    /// Generic write method for supported types (u8, u16, u32)
    pub fn write<T: RamValueT>(
        &mut self,
        addr: u32,
        value: T,
        timestamp: u32,
        pc: B32,
    ) -> Result<(), MemoryError> {
        self.check_alignment::<T>(addr)?;
        self.ensure_capacity::<T>(addr);

        let addr_usize = addr as usize;
        let end_addr = addr_usize + T::byte_size();

        let previous_value = T::from_le_bytes(&self.data[addr_usize..end_addr]);

        let bytes = value.to_le_bytes();
        self.data[addr_usize..addr_usize + bytes.len()].copy_from_slice(&bytes);

        self.access_history.push(RamAccessEvent {
            address: addr,
            value: value.value(),
            previous_value: Some(previous_value.value()),
            timestamp,
            pc,
            is_write: true,
        });

        Ok(())
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use binius_field::Field;

    use super::*;

    #[test]
    fn test_ram_read_write() {
        let mut ram = Ram::new(16); // Start with small RAM, will be MIN_RAM_SIZE

        ram.write::<u32>(0, 0x12345678, 1, B32::ONE).unwrap();

        let value: u32 = ram.read(0, 2, B32::ONE).unwrap();
        assert_eq!(value, 0x12345678);

        assert_eq!(ram.access_history.len(), 2);

        let write_history = RamAccessEvent {
            address: 0,
            value: RamValue::Word(0x12345678),
            previous_value: Some(RamValue::Word(0)),
            timestamp: 1,
            pc: B32::ONE,
            is_write: true,
        };
        let read_history = RamAccessEvent {
            address: 0,
            value: RamValue::Word(0x12345678),
            previous_value: None,
            timestamp: 2,
            pc: B32::ONE,
            is_write: false,
        };
        assert_eq!(ram.access_history[0], write_history);
        assert_eq!(ram.access_history[1], read_history);
    }

    #[test]
    fn test_generic_access() {
        let mut ram = Ram::new(MIN_RAM_SIZE);

        ram.write::<u32>(0, 0xAABBCCDD, 1, B32::ONE).unwrap();

        let value: u32 = ram.read(0, 2, B32::ONE).unwrap();
        assert_eq!(value, 0xAABBCCDD);

        ram.write::<u16>(4, 0x1234, 3, B32::ONE).unwrap();
        ram.write::<u8>(6, 0x55, 4, B32::ONE).unwrap();
        ram.write::<u8>(7, 0x66, 5, B32::ONE).unwrap();

        let val1: u16 = ram.read(4, 6, B32::ONE).unwrap();
        let val2: u16 = ram.read(6, 7, B32::ONE).unwrap();
        let val3: u32 = ram.read(4, 8, B32::ONE).unwrap();

        assert_eq!(val1, 0x1234);
        assert_eq!(val2, 0x6655);
        assert_eq!(val3, 0x66551234);
    }

    #[test]
    fn test_power_of_two_sizing() {
        let ram = Ram::new(1000);
        assert_eq!(ram.capacity(), 1024);

        let ram = Ram::new(16);
        assert_eq!(ram.capacity(), MIN_RAM_SIZE);

        let mut ram = Ram::new(MIN_RAM_SIZE);
        ram.write::<u32>(MIN_RAM_SIZE as u32, 0xAABBCCDD, 1, B32::ONE)
            .unwrap();
        assert_eq!(ram.capacity(), MIN_RAM_SIZE * 2);
    }

    #[test]
    fn test_read_out_of_bounds() {
        let mut ram = Ram::new(MIN_RAM_SIZE);

        let result: Result<u32, _> = ram.read(MIN_RAM_SIZE as u32, 1, B32::ONE);

        assert!(result.is_err());
        match result {
            Err(MemoryError::RamAddressOutOfBounds(addr, size)) => {
                assert_eq!(addr, MIN_RAM_SIZE as u32);
                assert_eq!(size, 4);
            }
            _ => panic!("Expected RamAddressOutOfBounds error"),
        }
    }

    #[test]
    fn test_alignment_check() {
        let mut ram = Ram::new(MIN_RAM_SIZE);

        let result = ram.write::<u32>(1, 0x12345678, 1, B32::ONE);
        assert!(result.is_err());

        if let Err(MemoryError::RamMisalignedAccess(addr, size)) = result {
            assert_eq!(addr, 1);
            assert_eq!(size, 4);
        } else {
            panic!("Expected RamMisalignedAccess error");
        }
    }

    #[test]
    fn test_byte_operations() {
        let mut ram = Ram::new(MIN_RAM_SIZE);

        ram.write::<u8>(0, 0x11, 1, B32::ONE).unwrap();
        ram.write::<u8>(1, 0x22, 2, B32::ONE).unwrap();
        ram.write::<u8>(2, 0x33, 3, B32::ONE).unwrap();
        ram.write::<u8>(3, 0x44, 4, B32::ONE).unwrap();

        let b0: u8 = ram.read(0, 5, B32::ONE).unwrap();
        let b1: u8 = ram.read(1, 6, B32::ONE).unwrap();
        let b2: u8 = ram.read(2, 7, B32::ONE).unwrap();
        let b3: u8 = ram.read(3, 8, B32::ONE).unwrap();

        assert_eq!(b0, 0x11);
        assert_eq!(b1, 0x22);
        assert_eq!(b2, 0x33);
        assert_eq!(b3, 0x44);

        let word: u32 = ram.read(0, 9, B32::ONE).unwrap();
        assert_eq!(word, 0x44332211);
    }
}
