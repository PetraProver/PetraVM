use binius_field::{BinaryField16b, BinaryField32b, Field};

use crate::{
    event::Event,
    execution::{Interpreter, InterpreterChannels, InterpreterError, InterpreterTables},
    fire_non_jump_event, ZCrayTrace,
};

/// Enum to distinguish between the different kinds of shifts.
#[derive(Debug, Clone, PartialEq)]
pub enum ShiftOperation {
    LogicalLeft,
    LogicalRight,
    ArithmeticRight,
}

/// Indicates the source of the shift amount.
#[derive(Debug, Clone, PartialEq)]
pub enum ShiftSource {
    Immediate(u16),       // 16-bit immediate shift amount
    VromOffset(u16, u32), // (16-bit VROM offset, 32-bit VROM value)
}

/// Combined event for both logical and arithmetic shift operations.
/// The type of shift is determined by the `op` field.
#[derive(Debug, Clone, PartialEq)]
pub struct ShiftEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,                // 16-bit destination VROM offset
    dst_val: u32,            // 32-bit result value
    src: u16,                // 16-bit source VROM offset
    pub(crate) src_val: u32, // 32-bit source value
    shift_source: ShiftSource,
    op: ShiftOperation, // Specifies which shift operation to perform
}

impl ShiftEvent {
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
        op: ShiftOperation,
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
            op,
        }
    }

    /// Calculate the result of the shift operation.
    ///
    /// If `shift_amount` is 0, returns the original value.
    /// For shift amounts ≥ 32, returns 0 for logical shifts and
    /// either 0 or 0xFFFFFFFF for arithmetic right shifts depending on the sign
    /// bit. Otherwise, the shift is performed based on the `op`:
    /// - LogicalLeft: `src_val << shift_amount`
    /// - LogicalRight: `src_val >> shift_amount`
    /// - ArithmeticRight: arithmetic right shift preserving the sign.
    pub fn calculate_result(src_val: u32, shift_amount: u32, op: &ShiftOperation) -> u32 {
        if shift_amount == 0 {
            return src_val;
        }
        if shift_amount >= 32 {
            return match op {
                ShiftOperation::ArithmeticRight => ((src_val as i32) >> 31) as u32,
                _ => 0,
            };
        }
        match op {
            ShiftOperation::LogicalLeft => src_val << shift_amount,
            ShiftOperation::LogicalRight => src_val >> shift_amount,
            ShiftOperation::ArithmeticRight => ((src_val as i32) >> shift_amount) as u32,
        }
    }

    /// Generate a ShiftEvent for immediate shift operations.
    ///
    /// For immediate shifts (like SLLI, SRLI, SRAI), the shift amount comes
    /// directly from the instruction (as a 16-bit immediate).
    pub fn generate_immediate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
        op: ShiftOperation,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let src_val = trace.get_vrom_u32(interpreter.fp ^ src.val() as u32)?;
        let imm_val = imm.val();
        let shift_amount = u32::from(imm_val);
        let new_val = Self::calculate_result(src_val, shift_amount, &op);
        let timestamp = interpreter.timestamp;
        trace.set_vrom_u32(interpreter.fp ^ dst.val() as u32, new_val)?;
        interpreter.incr_pc();

        Ok(Self::new(
            field_pc,
            interpreter.fp,
            timestamp,
            dst.val(),
            new_val,
            src.val(),
            src_val,
            ShiftSource::Immediate(imm_val),
            op,
        ))
    }

    /// Generate a ShiftEvent for VROM-based shift operations.
    ///
    /// For VROM-based shifts (like SLL, SRL, SRA), the shift amount is read
    /// from another VROM location and masked to 5 bits.
    pub fn generate_vrom_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
        op: ShiftOperation,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let src_val = trace.get_vrom_u32(interpreter.fp ^ src1.val() as u32)?;
        let shift_amount = trace.get_vrom_u32(interpreter.fp ^ src2.val() as u32)?;
        let src2_offset = src2.val();
        let new_val = Self::calculate_result(src_val, shift_amount, &op);
        let timestamp = interpreter.timestamp;
        trace.set_vrom_u32(interpreter.fp ^ dst.val() as u32, new_val)?;
        interpreter.incr_pc();

        Ok(Self::new(
            field_pc,
            interpreter.fp,
            timestamp,
            dst.val(),
            new_val,
            src1.val(),
            src_val,
            ShiftSource::VromOffset(src2_offset, shift_amount),
            op,
        ))
    }
}

impl Event for ShiftEvent {
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

    // Test the calculation function for logical left shift.
    #[test]
    fn test_shift_event_calculate_logical_left() {
        let src_val = 0x00000001;
        let shift_amount = 4u32;
        let result =
            ShiftEvent::calculate_result(src_val, shift_amount, &ShiftOperation::LogicalLeft);
        assert_eq!(result, 0x00000010);
    }

    // Test the calculation function for logical right shift.
    #[test]
    fn test_shift_event_calculate_logical_right() {
        let src_val = 0x00000010;
        let shift_amount = 4u32;
        let result =
            ShiftEvent::calculate_result(src_val, shift_amount, &ShiftOperation::LogicalRight);
        assert_eq!(result, 0x00000001);
    }

    // Test the calculation function for arithmetic right shift.
    #[test]
    fn test_shift_event_calculate_arithmetic_right() {
        let src_val = 0x80000008; // Negative number (sign bit set)
        let shift_amount = 3u32;
        let result =
            ShiftEvent::calculate_result(src_val, shift_amount, &ShiftOperation::ArithmeticRight);
        assert_eq!(result, 0xF0000001);
    }

    // Test edge cases: shift amount of 0 should return the original value.
    #[test]
    fn test_shift_event_edge_zero_shift() {
        let src_val = 0x12345678;
        for op in [
            ShiftOperation::LogicalLeft,
            ShiftOperation::LogicalRight,
            ShiftOperation::ArithmeticRight,
        ] {
            let result = ShiftEvent::calculate_result(src_val, 0, &op);
            assert_eq!(result, src_val);
        }
    }

    // Test edge cases: shift amount >= 32.
    #[test]
    fn test_shift_event_edge_large_shift() {
        let src_val = 0x12345678;
        let left = ShiftEvent::calculate_result(src_val, 32, &ShiftOperation::LogicalLeft);
        let right = ShiftEvent::calculate_result(src_val, 32, &ShiftOperation::LogicalRight);
        let arith = ShiftEvent::calculate_result(src_val, 32, &ShiftOperation::ArithmeticRight);
        assert_eq!(left, 0);
        assert_eq!(right, 0);
        // For arithmetic shift, if the sign bit is not set, result is 0.
        assert_eq!(arith, 0);

        // With sign bit set.
        let src_val_neg = 0x80000000;
        let arith_neg =
            ShiftEvent::calculate_result(src_val_neg, 32, &ShiftOperation::ArithmeticRight);
        assert_eq!(arith_neg, 0xFFFFFFFF);
    }

    // Integration test for logical shift events using immediate mode.
    #[test]
    fn test_shift_event_logical_integration() {
        // Setup a simple program with two shift events:
        // First a logical left immediate shift (SLLI), then a logical right immediate
        // shift (SRLI).
        let zero = BinaryField16b::zero();
        let dst1 = BinaryField16b::new(3);
        let src1 = BinaryField16b::new(2);
        let imm1 = BinaryField16b::new(4); // shift left by 4

        let dst2 = BinaryField16b::new(4);
        let src2 = BinaryField16b::new(3);
        let imm2 = BinaryField16b::new(2); // shift right by 2

        // SLLI and SRLI instructions (their opcodes drive the selection of the
        // operation).
        let instructions = vec![
            [Opcode::Slli.get_field_elt(), dst1, src1, imm1],
            [Opcode::Srli.get_field_elt(), dst2, dst1, imm2],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];

        let mut frames = HashMap::new();
        frames.insert(BinaryField32b::ONE, 5);

        let prom = code_to_prom(&instructions);
        let mut vrom = ValueRom::default();
        // Initialize VROM values: offsets 0, 1, and source value at offset 2.
        vrom.set_u32(0, 0).unwrap();
        vrom.set_u32(1, 0).unwrap();
        vrom.set_u32(2, 0x00000002).unwrap();

        let memory = Memory::new(prom, vrom);
        let (trace, _) = ZCrayTrace::generate(memory, frames, HashMap::new())
            .expect("Trace generation should not fail.");

        // Check the results in VROM.
        // First event: 0x00000002 << 4 = 0x00000020 at VROM offset 3.
        // Second event: 0x00000020 >> 2 = 0x00000008 at VROM offset 4.
        assert_eq!(trace.get_vrom_u32(3).unwrap(), 0x00000020);
        assert_eq!(trace.get_vrom_u32(4).unwrap(), 0x00000008);
    }

    // Integration test for arithmetic shift event using immediate mode.
    #[test]
    fn test_shift_event_arithmetic_integration() {
        // Setup a simple program with an arithmetic right shift immediate (SRAI).
        let zero = BinaryField16b::zero();
        let dst = BinaryField16b::new(3);
        let src = BinaryField16b::new(2);
        let imm = BinaryField16b::new(2); // shift right arithmetic by 2

        let instructions = vec![
            [Opcode::Srai.get_field_elt(), dst, src, imm],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];

        let mut frames = HashMap::new();
        frames.insert(BinaryField32b::ONE, 4);

        let prom = code_to_prom(&instructions);
        let mut vrom = ValueRom::default();
        // Initialize VROM values: offsets 0, 1, and source value (a negative number) at
        // offset 2.
        vrom.set_u32(0, 0).unwrap();
        vrom.set_u32(1, 0).unwrap();
        vrom.set_u32(2, 0xF0000000).unwrap();

        let memory = Memory::new(prom, vrom);
        let (trace, _) = ZCrayTrace::generate(memory, frames, HashMap::new())
            .expect("Trace generation should not fail.");

        // Expected arithmetic right shift:
        // For 0xF0000000 >> 2 (arithmetic), the sign is preserved, resulting in
        // 0xFC000000 at VROM offset 3.
        assert_eq!(trace.get_vrom_u32(3).unwrap(), 0xFC000000);
    }
}
