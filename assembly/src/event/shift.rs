use binius_field::{BinaryField16b, BinaryField32b, Field};

use crate::{
    event::Event,
    execution::{Interpreter, InterpreterChannels, InterpreterError, InterpreterTables},
    fire_non_jump_event, ZCrayTrace,
};

#[derive(Debug, Clone, PartialEq)]
pub enum LogicalShiftKind {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShiftSource {
    Immediate(u16),       // 16-bit immediate shift amount
    VromOffset(u16, u32), // (16-bit VROM offset, 32-bit VROM value)
}

/// Event for logical shift operations: SLLI, SRLI, SLL, SRL
///
/// Performs a logical shift operation (left or right) on a source value.
#[derive(Debug, Clone, PartialEq)]
pub struct LogicalShiftEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,                  // 16-bit destination VROM offset
    dst_val: u32,              // 32-bit result value
    src: u16,                  // 16-bit source VROM offset
    pub(crate) src_val: u32,   // 32-bit source value
    shift_source: ShiftSource, // Where the shift amount comes from
    kind: LogicalShiftKind,    // Left or Right shift
}

/// Event for arithmetic shift operations: SRAI, SRA
///
/// Performs an arithmetic right shift preserving the sign bit.
#[derive(Debug, Clone, PartialEq)]
pub struct ArithmeticShiftEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,                  // 16-bit destination VROM offset
    dst_val: u32,              // 32-bit result value
    src: u16,                  // 16-bit source VROM offset
    pub(crate) src_val: u32,   // 32-bit source value
    shift_source: ShiftSource, // Where the shift amount comes from
}

impl LogicalShiftEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        dst_val: u32,
        src: u16,
        src_val: u32,
        shift_source: ShiftSource,
        kind: LogicalShiftKind,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_val,
            src,
            src_val,
            shift_source,
            kind,
        }
    }

    /// Calculate the result of a logical shift.
    ///
    /// Returns the original value if `shift_amount` is 0, or 0 if the amount is
    /// ≥ 32. Otherwise, performs a left or right shift based on `kind`.
    pub fn calculate_result(src_val: u32, shift_amount: u32, kind: &LogicalShiftKind) -> u32 {
        if shift_amount == 0 {
            return src_val;
        }
        if shift_amount >= 32 {
            return 0;
        }
        match kind {
            LogicalShiftKind::Left => src_val << shift_amount,
            LogicalShiftKind::Right => src_val >> shift_amount,
        }
    }

    /// Generate a LogicalShiftEvent for immediate shift operations (SLLI, SRLI)
    pub fn generate_immediate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
        kind: LogicalShiftKind,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        // Using XOR for address calculation is by design for mapping the FP and VROM
        // offset.
        let src_val = trace.get_vrom_u32(interpreter.fp ^ src.val() as u32)?;
        let imm_val = imm.val(); // u16 immediate value from instruction

        let shift_amount = u32::from(imm_val);
        let new_val = Self::calculate_result(src_val, shift_amount, &kind);

        let timestamp = interpreter.timestamp;
        trace.set_vrom_u32(interpreter.fp ^ dst.val() as u32, new_val)?;
        interpreter.incr_pc();

        Ok(LogicalShiftEvent::new(
            field_pc,
            interpreter.fp,
            timestamp,
            dst.val(),
            new_val,
            src.val(),
            src_val,
            ShiftSource::Immediate(imm_val),
            kind,
        ))
    }

    /// Generate a LogicalShiftEvent for VROM-based shift operations (SLL, SRL)
    pub fn generate_vrom_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
        kind: LogicalShiftKind,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        // The XOR between FP and VROM offset derives a unique VROM address.
        let src_val = trace.get_vrom_u32(interpreter.fp ^ src1.val() as u32)?;
        let shift_vrom_val = trace.get_vrom_u32(interpreter.fp ^ src2.val() as u32)?;
        let src2_offset = src2.val();

        let shift_amount = shift_vrom_val & 0x1F;
        let new_val = Self::calculate_result(src_val, shift_amount, &kind);

        let timestamp = interpreter.timestamp;
        trace.set_vrom_u32(interpreter.fp ^ dst.val() as u32, new_val)?;
        interpreter.incr_pc();

        Ok(LogicalShiftEvent::new(
            field_pc,
            interpreter.fp,
            timestamp,
            dst.val(),
            new_val,
            src1.val(),
            src_val,
            ShiftSource::VromOffset(src2_offset, shift_vrom_val),
            kind,
        ))
    }
}

impl ArithmeticShiftEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        dst_val: u32,
        src: u16,
        src_val: u32,
        shift_source: ShiftSource,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_val,
            src,
            src_val,
            shift_source,
        }
    }

    /// Calculate the result of an arithmetic right shift.
    ///
    /// Returns the original value if `shift_amount` is 0, or all ones/zeros if
    /// ≥ 32, depending on the sign bit. Otherwise, performs an arithmetic
    /// right shift.
    pub fn calculate_result(src_val: u32, shift_amount: u32) -> u32 {
        if shift_amount == 0 {
            return src_val;
        }
        if shift_amount >= 32 {
            return if (src_val & 0x80000000) != 0 {
                0xFFFFFFFF
            } else {
                0
            };
        }
        // Cast to i32 for arithmetic shift (to preserve sign) then back to u32.
        ((src_val as i32) >> shift_amount) as u32
    }

    /// Generate an ArithmeticShiftEvent for immediate shift operations (SRAI)
    pub fn generate_immediate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let src_val = trace.get_vrom_u32(interpreter.fp ^ src.val() as u32)?;
        let imm_val = imm.val();

        let shift_amount = u32::from(imm_val);
        let new_val = Self::calculate_result(src_val, shift_amount);

        let timestamp = interpreter.timestamp;
        trace.set_vrom_u32(interpreter.fp ^ dst.val() as u32, new_val)?;
        interpreter.incr_pc();

        Ok(ArithmeticShiftEvent::new(
            field_pc,
            interpreter.fp,
            timestamp,
            dst.val(),
            new_val,
            src.val(),
            src_val,
            ShiftSource::Immediate(imm_val),
        ))
    }

    /// Generate an ArithmeticShiftEvent for VROM-based shift operations (SRA)
    pub fn generate_vrom_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let src_val = trace.get_vrom_u32(interpreter.fp ^ src1.val() as u32)?;
        let shift_vrom_val = trace.get_vrom_u32(interpreter.fp ^ src2.val() as u32)?;
        let src2_offset = src2.val();

        let shift_amount = shift_vrom_val & 0x1F;
        let new_val = Self::calculate_result(src_val, shift_amount);

        let timestamp = interpreter.timestamp;
        trace.set_vrom_u32(interpreter.fp ^ dst.val() as u32, new_val)?;
        interpreter.incr_pc();

        Ok(ArithmeticShiftEvent::new(
            field_pc,
            interpreter.fp,
            timestamp,
            dst.val(),
            new_val,
            src1.val(),
            src_val,
            ShiftSource::VromOffset(src2_offset, shift_vrom_val),
        ))
    }
}

// Using the fire_non_jump_event macro to simplify the Event implementation for
// both types
impl Event for LogicalShiftEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        fire_non_jump_event!(self, channels);
    }
}

impl Event for ArithmeticShiftEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        fire_non_jump_event!(self, channels);
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use binius_field::PackedField;

    use super::*;
    use crate::{
        event::ret::RetEvent, memory::Memory, opcodes::Opcode, util::code_to_prom, ValueRom,
    };

    #[test]
    fn test_logical_shift_events() {
        // Test left shift
        let src_val = 0x00000001;
        let shift_amount = 4u32;
        let left_result =
            LogicalShiftEvent::calculate_result(src_val, shift_amount, &LogicalShiftKind::Left);
        assert_eq!(left_result, 0x00000010);

        // Test right shift
        let src_val = 0x00000010;
        let right_result =
            LogicalShiftEvent::calculate_result(src_val, shift_amount, &LogicalShiftKind::Right);
        assert_eq!(right_result, 0x00000001);

        // Edge cases: shift by 0 and shift by 32 or more
        assert_eq!(
            LogicalShiftEvent::calculate_result(src_val, 0, &LogicalShiftKind::Left),
            src_val
        );
        assert_eq!(
            LogicalShiftEvent::calculate_result(src_val, 32, &LogicalShiftKind::Left),
            0
        );
    }

    #[test]
    fn test_arithmetic_shift_events() {
        // Test arithmetic right shift with sign bit set
        let src_val = 0x80000008; // Negative number
        let shift_amount = 3u32;
        let result = ArithmeticShiftEvent::calculate_result(src_val, shift_amount);
        assert_eq!(result, 0xF0000001);

        // Test arithmetic right shift with sign bit clear
        let src_val = 0x00000008; // Positive number
        let result = ArithmeticShiftEvent::calculate_result(src_val, shift_amount);
        assert_eq!(result, 0x00000001);

        // Edge cases
        assert_eq!(ArithmeticShiftEvent::calculate_result(src_val, 0), src_val);
        assert_eq!(ArithmeticShiftEvent::calculate_result(0x70000000, 32), 0);
        assert_eq!(
            ArithmeticShiftEvent::calculate_result(0x80000000, 32),
            0xFFFFFFFF
        );
    }

    #[test]
    fn test_logical_shift_integration() {
        // Setup a simple program with logical shifts.
        let zero = BinaryField16b::zero();
        let dst1 = BinaryField16b::new(3);
        let src1 = BinaryField16b::new(2);
        let imm1 = BinaryField16b::new(4);

        let dst2 = BinaryField16b::new(4);
        let src2 = BinaryField16b::new(3);
        let imm2 = BinaryField16b::new(2);

        // SLLI and SRLI instructions
        let instructions = vec![
            [Opcode::Slli.get_field_elt(), dst1, src1, imm1],
            [Opcode::Srli.get_field_elt(), dst2, dst1, imm2],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];

        let mut frames = HashMap::new();
        frames.insert(BinaryField32b::ONE, 5);

        let prom = code_to_prom(&instructions, &vec![false; instructions.len()]);
        let mut vrom = ValueRom::default();
        vrom.set_u32(0, 0);
        vrom.set_u32(1, 0);
        vrom.set_u32(2, 0x00000002); // Initial value

        let memory = Memory::new(prom, vrom);
        let (trace, _) = ZCrayTrace::generate(memory, frames, HashMap::new())
            .expect("Trace generation should not fail.");

        // Check the results: first shift left, then shift right.
        assert_eq!(trace.logical_shifts.len(), 2);
        assert_eq!(trace.logical_shifts[0].dst_val, 0x00000020);
        assert_eq!(trace.logical_shifts[1].dst_val, 0x00000008);
        assert_eq!(trace.get_vrom_u32(3).unwrap(), 0x00000020);
        assert_eq!(trace.get_vrom_u32(4).unwrap(), 0x00000008);
    }

    #[test]
    fn test_arithmetic_shift_integration() {
        // Setup a simple program with arithmetic shift.
        let zero = BinaryField16b::zero();
        let dst = BinaryField16b::new(3);
        let src = BinaryField16b::new(2);
        let imm = BinaryField16b::new(2);

        // SRAI instruction
        let instructions = vec![
            [Opcode::Srai.get_field_elt(), dst, src, imm],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];

        let mut frames = HashMap::new();
        frames.insert(BinaryField32b::ONE, 4);

        let prom = code_to_prom(&instructions, &vec![false; instructions.len()]);
        let mut vrom = ValueRom::default();
        vrom.set_u32(0, 0);
        vrom.set_u32(1, 0);
        vrom.set_u32(2, 0xF0000000); // Negative number

        let memory = Memory::new(prom, vrom);
        let (trace, _) = ZCrayTrace::generate(memory, frames, HashMap::new())
            .expect("Trace generation should not fail.");

        // Check the arithmetic shift result.
        assert_eq!(trace.arithmetic_shifts.len(), 1);
        assert_eq!(trace.arithmetic_shifts[0].dst_val, 0xFC000000);
        assert_eq!(trace.get_vrom_u32(3).unwrap(), 0xFC000000);
    }
}
