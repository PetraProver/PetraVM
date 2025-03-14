use binius_field::{BinaryField16b, BinaryField32b};

use crate::{
    event::Event,
    execution::{Interpreter, InterpreterChannels, InterpreterError, InterpreterTables},
    fire_non_jump_event, ZCrayTrace, G,
};

/// Helper function to compute the effective RAM address.
/// It retrieves the frame pointer (fp) and timestamp from the interpreter,
/// then uses the provided pointer and offset (both as BinaryField16b) to:
///  - Look up the pointer value from VROM using an XOR with fp.
///  - Compute the effective address as: ptr_val.wrapping_add(offset)
fn compute_effective_address(
    interpreter: &Interpreter,
    trace: &mut ZCrayTrace,
    ptr: BinaryField16b,
    offset: BinaryField16b,
) -> Result<(u32, u32, u32, u32), InterpreterError> {
    let fp = interpreter.fp;
    let timestamp = interpreter.timestamp;
    let ptr_val = trace.get_vrom_u32(fp ^ ptr.val() as u32)?;
    let addr = ptr_val.wrapping_add(offset.val() as u32);
    Ok((fp, timestamp, ptr_val, addr))
}

/// Common trait for RAM access events
pub(crate) trait RamAccessEvent: Event {
    fn address(&self) -> u32;
    fn value(&self) -> u32;
    fn is_store(&self) -> bool;
}

// Load Word - Loads a 4-byte word from RAM into a VROM value
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LWEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,        // Destination slot offset from FP
    ptr: u16,        // Pointer slot offset from FP
    offset: u16,     // Offset to add to the pointer value
    ptr_val: u32,    // Value of the pointer
    loaded_val: u32, // Value loaded from RAM
}

impl LWEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        ptr: u16,
        offset: u16,
        ptr_val: u32,
        loaded_val: u32,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            ptr,
            offset,
            ptr_val,
            loaded_val,
        }
    }

    /// Generates an LW event.
    /// This function:
    /// 1. Retrieves the pointer value from VROM using `fp ^ ptr`.
    /// 2. Computes the effective address and reads a 32-bit value from RAM.
    /// 3. Stores the loaded value into VROM at the destination index.
    /// 4. Increments the program counter.
    /// (See design document :contentReference[oaicite:0]{index=0} for details.)
    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        ptr: BinaryField16b,
        offset: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let (fp, timestamp, ptr_val, addr) =
            compute_effective_address(interpreter, trace, ptr, offset)?;
        let loaded_val = trace.read_ram_u32(addr, timestamp, field_pc)?;
        trace.set_vrom_u32(fp ^ dst.val() as u32, loaded_val)?;
        interpreter.incr_pc();
        Ok(Self {
            pc: field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            ptr: ptr.val(),
            offset: offset.val(),
            ptr_val,
            loaded_val,
        })
    }
}

impl Event for LWEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        fire_non_jump_event!(self, channels);
    }
}

impl RamAccessEvent for LWEvent {
    fn address(&self) -> u32 {
        self.ptr_val.wrapping_add(self.offset as u32)
    }

    fn value(&self) -> u32 {
        self.loaded_val
    }

    fn is_store(&self) -> bool {
        false
    }
}

// Store Word - Store a 4-byte word from VROM into RAM
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SWEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    src: u16,        // Source slot offset from FP
    ptr: u16,        // Pointer slot offset from FP
    offset: u16,     // Offset to add to the pointer value
    ptr_val: u32,    // Value of the pointer
    stored_val: u32, // Value stored to RAM
}

impl SWEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        src: u16,
        ptr: u16,
        offset: u16,
        ptr_val: u32,
        stored_val: u32,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            src,
            ptr,
            offset,
            ptr_val,
            stored_val,
        }
    }

    /// Generates an SW event.
    /// It retrieves the pointer value, computes the effective address, reads
    /// the source value from VROM, writes it to RAM, and then increments
    /// the PC.
    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        src: BinaryField16b,
        ptr: BinaryField16b,
        offset: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let (fp, timestamp, ptr_val, addr) =
            compute_effective_address(interpreter, trace, ptr, offset)?;
        let src_val = trace.get_vrom_u32(fp ^ src.val() as u32)?;
        trace.write_ram_u32(addr, src_val, timestamp, field_pc)?;
        interpreter.incr_pc();
        Ok(Self {
            pc: field_pc,
            fp,
            timestamp,
            src: src.val(),
            ptr: ptr.val(),
            offset: offset.val(),
            ptr_val,
            stored_val: src_val,
        })
    }
}

impl Event for SWEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        fire_non_jump_event!(self, channels);
    }
}

impl RamAccessEvent for SWEvent {
    fn address(&self) -> u32 {
        self.ptr_val.wrapping_add(self.offset as u32)
    }

    fn value(&self) -> u32 {
        self.stored_val
    }

    fn is_store(&self) -> bool {
        true
    }
}

// Load Byte - Loads a signed 8-bit byte from RAM into a VROM value with sign
// extension
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LBEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,        // Destination slot offset from FP
    ptr: u16,        // Pointer slot offset from FP
    offset: u16,     // Offset to add to the pointer value
    ptr_val: u32,    // Value of the pointer
    loaded_val: u32, // Value loaded from RAM (sign-extended)
    raw_val: u8,     // Raw byte value loaded
}

impl LBEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        ptr: u16,
        offset: u16,
        ptr_val: u32,
        loaded_val: u32,
        raw_val: u8,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            ptr,
            offset,
            ptr_val,
            loaded_val,
            raw_val,
        }
    }

    /// Generates an LB event.
    /// It computes the effective address, reads an 8-bit value from RAM,
    /// sign-extends it to 32 bits, updates VROM, and advances the PC.
    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        ptr: BinaryField16b,
        offset: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let (fp, timestamp, ptr_val, addr) =
            compute_effective_address(interpreter, trace, ptr, offset)?;
        let raw_val = trace.read_ram_u8(addr, timestamp, field_pc)?;
        let loaded_val = ((raw_val as i8) as i32) as u32;
        trace.set_vrom_u32(fp ^ dst.val() as u32, loaded_val)?;
        interpreter.incr_pc();
        Ok(Self {
            pc: field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            ptr: ptr.val(),
            offset: offset.val(),
            ptr_val,
            loaded_val,
            raw_val,
        })
    }
}

impl Event for LBEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        fire_non_jump_event!(self, channels);
    }
}

impl RamAccessEvent for LBEvent {
    fn address(&self) -> u32 {
        self.ptr_val.wrapping_add(self.offset as u32)
    }

    fn value(&self) -> u32 {
        self.loaded_val
    }

    fn is_store(&self) -> bool {
        false
    }
}

// Load Byte Unsigned - Loads an unsigned 8-bit byte from RAM into a VROM value
// with zero extension
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LBUEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,        // Destination slot offset from FP
    ptr: u16,        // Pointer slot offset from FP
    offset: u16,     // Offset to add to the pointer value
    ptr_val: u32,    // Value of the pointer
    loaded_val: u32, // Value loaded from RAM (zero-extended)
    raw_val: u8,     // Raw byte value loaded
}

impl LBUEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        ptr: u16,
        offset: u16,
        ptr_val: u32,
        loaded_val: u32,
        raw_val: u8,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            ptr,
            offset,
            ptr_val,
            loaded_val,
            raw_val,
        }
    }

    /// Generates an LBU event.
    /// It reads an 8-bit value from RAM, zero-extends it, updates VROM,
    /// and advances the PC.
    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        ptr: BinaryField16b,
        offset: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let (fp, timestamp, ptr_val, addr) =
            compute_effective_address(interpreter, trace, ptr, offset)?;
        let raw_val = trace.read_ram_u8(addr, timestamp, field_pc)?;
        let loaded_val = raw_val as u32;
        trace.set_vrom_u32(fp ^ dst.val() as u32, loaded_val)?;
        interpreter.incr_pc();
        Ok(Self {
            pc: field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            ptr: ptr.val(),
            offset: offset.val(),
            ptr_val,
            loaded_val,
            raw_val,
        })
    }
}

impl Event for LBUEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        fire_non_jump_event!(self, channels);
    }
}

impl RamAccessEvent for LBUEvent {
    fn address(&self) -> u32 {
        self.ptr_val.wrapping_add(self.offset as u32)
    }

    fn value(&self) -> u32 {
        self.loaded_val
    }

    fn is_store(&self) -> bool {
        false
    }
}

// Load Half-word - Loads a signed 16-bit half-word from RAM into a VROM value
// with sign extension
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LHEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,        // Destination slot offset from FP
    ptr: u16,        // Pointer slot offset from FP
    offset: u16,     // Offset to add to the pointer value
    ptr_val: u32,    // Value of the pointer
    loaded_val: u32, // Value loaded from RAM (sign-extended)
    raw_val: u16,    // Raw half-word value loaded
}

impl LHEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        ptr: u16,
        offset: u16,
        ptr_val: u32,
        loaded_val: u32,
        raw_val: u16,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            ptr,
            offset,
            ptr_val,
            loaded_val,
            raw_val,
        }
    }

    /// Generates an LH event.
    /// Reads a 16-bit value from RAM, sign-extends it, updates VROM, and
    /// increments the PC.
    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        ptr: BinaryField16b,
        offset: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let (fp, timestamp, ptr_val, addr) =
            compute_effective_address(interpreter, trace, ptr, offset)?;
        let raw_val = trace.read_ram_u16(addr, timestamp, field_pc)?;
        let loaded_val = ((raw_val as i16) as i32) as u32;
        trace.set_vrom_u32(fp ^ dst.val() as u32, loaded_val)?;
        interpreter.incr_pc();
        Ok(Self {
            pc: field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            ptr: ptr.val(),
            offset: offset.val(),
            ptr_val,
            loaded_val,
            raw_val,
        })
    }
}

impl Event for LHEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        fire_non_jump_event!(self, channels);
    }
}

impl RamAccessEvent for LHEvent {
    fn address(&self) -> u32 {
        self.ptr_val.wrapping_add(self.offset as u32)
    }

    fn value(&self) -> u32 {
        self.loaded_val
    }

    fn is_store(&self) -> bool {
        false
    }
}

// Load Half-word Unsigned - Loads an unsigned 16-bit half-word from RAM into a
// VROM value with zero extension
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LHUEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,        // Destination slot offset from FP
    ptr: u16,        // Pointer slot offset from FP
    offset: u16,     // Offset to add to the pointer value
    ptr_val: u32,    // Value of the pointer
    loaded_val: u32, // Value loaded from RAM (zero-extended)
    raw_val: u16,    // Raw half-word value loaded
}

impl LHUEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        ptr: u16,
        offset: u16,
        ptr_val: u32,
        loaded_val: u32,
        raw_val: u16,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            ptr,
            offset,
            ptr_val,
            loaded_val,
            raw_val,
        }
    }

    /// Generates an LHU event.
    /// Reads a 16-bit value from RAM, zero-extends it, updates VROM, and
    /// advances the PC.
    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        ptr: BinaryField16b,
        offset: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let (fp, timestamp, ptr_val, addr) =
            compute_effective_address(interpreter, trace, ptr, offset)?;
        let raw_val = trace.read_ram_u16(addr, timestamp, field_pc)?;
        let loaded_val = raw_val as u32;
        trace.set_vrom_u32(fp ^ dst.val() as u32, loaded_val)?;
        interpreter.incr_pc();
        Ok(Self {
            pc: field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            ptr: ptr.val(),
            offset: offset.val(),
            ptr_val,
            loaded_val,
            raw_val,
        })
    }
}

impl Event for LHUEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        fire_non_jump_event!(self, channels);
    }
}

impl RamAccessEvent for LHUEvent {
    fn address(&self) -> u32 {
        self.ptr_val.wrapping_add(self.offset as u32)
    }

    fn value(&self) -> u32 {
        self.loaded_val
    }

    fn is_store(&self) -> bool {
        false
    }
}

// Store Byte - Store a byte from VROM into RAM
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SBEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    src: u16,       // Source slot offset from FP
    ptr: u16,       // Pointer slot offset from FP
    offset: u16,    // Offset to add to the pointer value
    ptr_val: u32,   // Value of the pointer
    stored_val: u8, // Value stored to RAM (only lowest 8 bits used)
    src_val: u32,   // Original 32-bit value from VROM
}

impl SBEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        src: u16,
        ptr: u16,
        offset: u16,
        ptr_val: u32,
        stored_val: u8,
        src_val: u32,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            src,
            ptr,
            offset,
            ptr_val,
            stored_val,
            src_val,
        }
    }

    /// Generates an SB event.
    /// It retrieves the source value from VROM, extracts the lowest byte,
    /// writes it to RAM, and then increments the PC.
    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        src: BinaryField16b,
        ptr: BinaryField16b,
        offset: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let (fp, timestamp, ptr_val, addr) =
            compute_effective_address(interpreter, trace, ptr, offset)?;
        let src_val = trace.get_vrom_u32(fp ^ src.val() as u32)?;
        let stored_val = src_val as u8;
        trace.write_ram_u8(addr, stored_val, timestamp, field_pc)?;
        interpreter.incr_pc();
        Ok(Self {
            pc: field_pc,
            fp,
            timestamp,
            src: src.val(),
            ptr: ptr.val(),
            offset: offset.val(),
            ptr_val,
            stored_val,
            src_val,
        })
    }
}

impl Event for SBEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        fire_non_jump_event!(self, channels);
    }
}

impl RamAccessEvent for SBEvent {
    fn address(&self) -> u32 {
        self.ptr_val.wrapping_add(self.offset as u32)
    }

    fn value(&self) -> u32 {
        self.stored_val as u32
    }

    fn is_store(&self) -> bool {
        true
    }
}

// Store Half-word - Store a half-word from VROM into RAM
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SHEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    src: u16,        // Source slot offset from FP
    ptr: u16,        // Pointer slot offset from FP
    offset: u16,     // Offset to add to the pointer value
    ptr_val: u32,    // Value of the pointer
    stored_val: u16, // Value stored to RAM (only lowest 16 bits used)
    src_val: u32,    // Original 32-bit value from VROM
}

impl SHEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        src: u16,
        ptr: u16,
        offset: u16,
        ptr_val: u32,
        stored_val: u16,
        src_val: u32,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            src,
            ptr,
            offset,
            ptr_val,
            stored_val,
            src_val,
        }
    }

    /// Generates an SH event.
    /// It retrieves the source value from VROM, extracts the lowest half-word,
    /// writes it to RAM, and increments the PC.
    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        src: BinaryField16b,
        ptr: BinaryField16b,
        offset: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let (fp, timestamp, ptr_val, addr) =
            compute_effective_address(interpreter, trace, ptr, offset)?;
        let src_val = trace.get_vrom_u32(fp ^ src.val() as u32)?;
        let stored_val = src_val as u16;
        trace.write_ram_u16(addr, stored_val, timestamp, field_pc)?;
        interpreter.incr_pc();
        Ok(Self {
            pc: field_pc,
            fp,
            timestamp,
            src: src.val(),
            ptr: ptr.val(),
            offset: offset.val(),
            ptr_val,
            stored_val,
            src_val,
        })
    }
}

impl Event for SHEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        fire_non_jump_event!(self, channels);
    }
}

impl RamAccessEvent for SHEvent {
    fn address(&self) -> u32 {
        self.ptr_val.wrapping_add(self.offset as u32)
    }

    fn value(&self) -> u32 {
        self.stored_val as u32
    }

    fn is_store(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use binius_field::{Field, PackedField};

    use super::*;
    use crate::{
        code_to_prom,
        memory::{Memory, ProgramRom, Ram},
        opcodes::Opcode,
        ValueRom,
    };

    // Helper function to set up a memory environment for testing
    fn setup_memory() -> Memory {
        let vrom = ValueRom::new_with_init_vals(&[0, 0, 0x1000, 0x42, 0x7FFF, 0xFFFF, 0x12345678]);
        // Index 0: Return PC
        // Index 1: Return FP
        // Index 2: Pointer value (0x1000)
        // Index 3: Test value (0x42)
        // Index 4: Signed value (0x7FFF)
        // Index 5: Negative value (0xFFFF)
        // Index 6: Full word (0x12345678)

        Memory::new(ProgramRom::default(), vrom)
    }

    #[test]
    fn test_lw_event() {
        let mut memory = setup_memory();

        // Initialize RAM with some data at address 0x1000
        let test_data: u32 = 0xABCDEF01;
        memory
            .write_ram_u32(0x1000, test_data, 0, BinaryField32b::ONE)
            .unwrap();

        let mut trace = ZCrayTrace::new(memory);
        let mut interpreter = Interpreter::default();

        // Create an LW event
        let dst = BinaryField16b::new(7); // Destination slot
        let ptr = BinaryField16b::new(2); // Pointer slot with value 0x1000
        let offset = BinaryField16b::zero(); // No offset

        let event = LWEvent::generate_event(
            &mut interpreter,
            &mut trace,
            dst,
            ptr,
            offset,
            BinaryField32b::ONE,
        )
        .unwrap();

        // Verify the event
        assert_eq!(event.ptr_val, 0x1000);
        assert_eq!(event.loaded_val, test_data);

        // Verify that the value was loaded into VROM
        let loaded = trace.get_vrom_u32(7).unwrap();
        assert_eq!(loaded, test_data);
    }

    #[test]
    fn test_sw_event() {
        let memory = setup_memory();
        let mut trace = ZCrayTrace::new(memory);
        let mut interpreter = Interpreter::default();

        // Create an SW event
        let src = BinaryField16b::new(6); // Source slot with value 0x12345678
        let ptr = BinaryField16b::new(2); // Pointer slot with value 0x1000
        let offset = BinaryField16b::new(4); // Offset 4

        let event = SWEvent::generate_event(
            &mut interpreter,
            &mut trace,
            src,
            ptr,
            offset,
            BinaryField32b::ONE,
        )
        .unwrap();

        // Verify the event
        assert_eq!(event.ptr_val, 0x1000);
        assert_eq!(event.stored_val, 0x12345678);

        // Verify that the value was stored in RAM
        let stored = trace.read_ram_u32(0x1004, 1, BinaryField32b::ONE).unwrap();
        assert_eq!(stored, 0x12345678);
    }

    #[test]
    fn test_lb_event() {
        let mut memory = setup_memory();

        // Initialize RAM with some data at address 0x1000
        // 0x80 = -128 as signed byte
        memory
            .write_ram_u8(0x1000, 0x80, 0, BinaryField32b::ONE)
            .unwrap();

        let mut trace = ZCrayTrace::new(memory);
        let mut interpreter = Interpreter::default();

        // Create an LB event (sign extension should occur)
        let dst = BinaryField16b::new(7); // Destination slot
        let ptr = BinaryField16b::new(2); // Pointer slot with value 0x1000
        let offset = BinaryField16b::zero(); // No offset

        let event = LBEvent::generate_event(
            &mut interpreter,
            &mut trace,
            dst,
            ptr,
            offset,
            BinaryField32b::ONE,
        )
        .unwrap();

        // Verify the event
        assert_eq!(event.ptr_val, 0x1000);
        assert_eq!(event.raw_val, 0x80);
        // Sign extension should make this 0xFFFF_FF80
        assert_eq!(event.loaded_val, 0xFFFFFF80);

        // Verify that the sign-extended value was loaded into VROM
        let loaded = trace.get_vrom_u32(7).unwrap();
        assert_eq!(loaded, 0xFFFFFF80);
    }

    #[test]
    fn test_lbu_event() {
        let mut memory = setup_memory();

        // Initialize RAM with some data at address 0x1000
        memory
            .write_ram_u8(0x1000, 0x80, 0, BinaryField32b::ONE)
            .unwrap();

        let mut trace = ZCrayTrace::new(memory);
        let mut interpreter = Interpreter::default();

        // Create an LBU event (zero extension should occur)
        let dst = BinaryField16b::new(7); // Destination slot
        let ptr = BinaryField16b::new(2); // Pointer slot with value 0x1000
        let offset = BinaryField16b::zero(); // No offset

        let event = LBUEvent::generate_event(
            &mut interpreter,
            &mut trace,
            dst,
            ptr,
            offset,
            BinaryField32b::ONE,
        )
        .unwrap();

        // Verify the event
        assert_eq!(event.ptr_val, 0x1000);
        assert_eq!(event.raw_val, 0x80);
        // Zero extension should make this 0x0000_0080
        assert_eq!(event.loaded_val, 0x80);

        // Verify that the zero-extended value was loaded into VROM
        let loaded = trace.get_vrom_u32(7).unwrap();
        assert_eq!(loaded, 0x80);
    }

    #[test]
    fn test_lh_event() {
        let mut memory = setup_memory();

        // Initialize RAM with some data at address 0x1000
        // 0x8000 = -32768 as signed half-word
        memory
            .write_ram_u16(0x1000, 0x8000, 0, BinaryField32b::ONE)
            .unwrap();

        let mut trace = ZCrayTrace::new(memory);
        let mut interpreter = Interpreter::default();

        // Create an LH event (sign extension should occur)
        let dst = BinaryField16b::new(7); // Destination slot
        let ptr = BinaryField16b::new(2); // Pointer slot with value 0x1000
        let offset = BinaryField16b::zero(); // No offset

        let event = LHEvent::generate_event(
            &mut interpreter,
            &mut trace,
            dst,
            ptr,
            offset,
            BinaryField32b::ONE,
        )
        .unwrap();

        // Verify the event
        assert_eq!(event.ptr_val, 0x1000);
        assert_eq!(event.raw_val, 0x8000);
        // Sign extension should make this 0xFFFF_8000
        assert_eq!(event.loaded_val, 0xFFFF8000);

        // Verify that the sign-extended value was loaded into VROM
        let loaded = trace.get_vrom_u32(7).unwrap();
        assert_eq!(loaded, 0xFFFF8000);
    }

    #[test]
    fn test_lhu_event() {
        let mut memory = setup_memory();

        // Initialize RAM with some data at address 0x1000
        memory
            .write_ram_u16(0x1000, 0x8000, 0, BinaryField32b::ONE)
            .unwrap();

        let mut trace = ZCrayTrace::new(memory);
        let mut interpreter = Interpreter::default();

        // Create an LHU event (zero extension should occur)
        let dst = BinaryField16b::new(7); // Destination slot
        let ptr = BinaryField16b::new(2); // Pointer slot with value 0x1000
        let offset = BinaryField16b::zero(); // No offset

        let event = LHUEvent::generate_event(
            &mut interpreter,
            &mut trace,
            dst,
            ptr,
            offset,
            BinaryField32b::ONE,
        )
        .unwrap();

        // Verify the event
        assert_eq!(event.ptr_val, 0x1000);
        assert_eq!(event.raw_val, 0x8000);
        // Zero extension should make this 0x0000_8000
        assert_eq!(event.loaded_val, 0x8000);

        // Verify that the zero-extended value was loaded into VROM
        let loaded = trace.get_vrom_u32(7).unwrap();
        assert_eq!(loaded, 0x8000);
    }

    #[test]
    fn test_sb_event() {
        let memory = setup_memory();
        let mut trace = ZCrayTrace::new(memory);
        let mut interpreter = Interpreter::default();

        // Create an SB event
        let src = BinaryField16b::new(6); // Source slot with value 0x12345678
        let ptr = BinaryField16b::new(2); // Pointer slot with value 0x1000
        let offset = BinaryField16b::new(8); // Offset 8

        let event = SBEvent::generate_event(
            &mut interpreter,
            &mut trace,
            src,
            ptr,
            offset,
            BinaryField32b::ONE,
        )
        .unwrap();

        // Verify the event
        assert_eq!(event.ptr_val, 0x1000);
        assert_eq!(event.stored_val, 0x78); // Lowest byte of 0x12345678

        // Verify that the byte was stored in RAM
        let stored = trace.read_ram_u8(0x1008, 1, BinaryField32b::ONE).unwrap();
        assert_eq!(stored, 0x78);
    }

    #[test]
    fn test_sh_event() {
        let memory = setup_memory();
        let mut trace = ZCrayTrace::new(memory);
        let mut interpreter = Interpreter::default();

        // Create an SH event
        let src = BinaryField16b::new(6); // Source slot with value 0x12345678
        let ptr = BinaryField16b::new(2); // Pointer slot with value 0x1000
        let offset = BinaryField16b::new(12); // Offset 12

        let event = SHEvent::generate_event(
            &mut interpreter,
            &mut trace,
            src,
            ptr,
            offset,
            BinaryField32b::ONE,
        )
        .unwrap();

        // Verify the event
        assert_eq!(event.ptr_val, 0x1000);
        assert_eq!(event.stored_val, 0x5678); // Lowest half-word of 0x12345678

        // Verify that the half-word was stored in RAM
        let stored = trace.read_ram_u16(0x100C, 1, BinaryField32b::ONE).unwrap();
        assert_eq!(stored, 0x5678);
    }

    #[test]
    fn test_ram_access_error_handling() {
        let memory = setup_memory();
        let mut trace = ZCrayTrace::new(memory);
        let mut interpreter = Interpreter::default();

        // Try to access misaligned address
        let dst = BinaryField16b::new(7);
        let ptr = BinaryField16b::new(3); // Points to 0x42
        let offset = BinaryField16b::new(1); // Makes it 0x43, which is misaligned for u16

        // LH requires 2-byte alignment
        let result = LHEvent::generate_event(
            &mut interpreter,
            &mut trace,
            dst,
            ptr,
            offset,
            BinaryField32b::ONE,
        );

        // Should return a RAM misaligned access error
        assert!(result.is_err());
    }

    #[test]
    fn test_ram_access_events_trait() {
        // Test that all RAM access events implement the RamAccessEvent trait
        let lw_event = LWEvent::new(BinaryField32b::ONE, 0, 1, 7, 2, 0, 0x1000, 0x12345678);

        let sw_event = SWEvent::new(BinaryField32b::ONE, 0, 1, 6, 2, 0, 0x1000, 0x12345678);

        // Check address and value methods
        assert_eq!(lw_event.address(), 0x1000);
        assert_eq!(lw_event.value(), 0x12345678);
        assert_eq!(lw_event.is_store(), false);

        assert_eq!(sw_event.address(), 0x1000);
        assert_eq!(sw_event.value(), 0x12345678);
        assert_eq!(sw_event.is_store(), true);
    }

    #[test]
    fn test_multiple_memory_operations() {
        let memory = setup_memory();
        let mut trace = ZCrayTrace::new(memory);
        let mut interpreter = Interpreter::default();

        // First, store a word
        let src = BinaryField16b::new(6); // Source slot with value 0x12345678
        let ptr = BinaryField16b::new(2); // Pointer slot with value 0x1000
        let offset = BinaryField16b::zero();

        let _ = SWEvent::generate_event(
            &mut interpreter,
            &mut trace,
            src,
            ptr,
            offset,
            BinaryField32b::ONE,
        )
        .unwrap();

        // Now read it back as individual bytes and verify
        // Use different destination slots for each byte to avoid VROM rewrite errors

        // Read first byte (should be 0x78 in little-endian)
        let dst1 = BinaryField16b::new(7);
        let offset_b0 = BinaryField16b::zero();
        let lb_event =
            LBUEvent::generate_event(&mut interpreter, &mut trace, dst1, ptr, offset_b0, G)
                .unwrap();
        assert_eq!(lb_event.raw_val, 0x78);

        // Read second byte (should be 0x56 in little-endian)
        let dst2 = BinaryField16b::new(8);
        let offset_b1 = BinaryField16b::new(1);
        let lb_event = LBUEvent::generate_event(
            &mut interpreter,
            &mut trace,
            dst2,
            ptr,
            offset_b1,
            G.square(),
        )
        .unwrap();
        assert_eq!(lb_event.raw_val, 0x56);

        // Read third byte (should be 0x34 in little-endian)
        let dst3 = BinaryField16b::new(9);
        let offset_b2 = BinaryField16b::new(2);
        let lb_event =
            LBUEvent::generate_event(&mut interpreter, &mut trace, dst3, ptr, offset_b2, G.pow(3))
                .unwrap();
        assert_eq!(lb_event.raw_val, 0x34);

        // Read fourth byte (should be 0x12 in little-endian)
        let dst4 = BinaryField16b::new(10);
        let offset_b3 = BinaryField16b::new(3);
        let lb_event =
            LBUEvent::generate_event(&mut interpreter, &mut trace, dst4, ptr, offset_b3, G.pow(4))
                .unwrap();
        assert_eq!(lb_event.raw_val, 0x12);
    }
}
