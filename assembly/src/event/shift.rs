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
    /// The effective shift amount is determined by masking the provided shift
    /// amount to the lower 5 bits (i.e., `shift_amount & 0x1F`). If the
    /// effective shift amount is 0, the original `src_val` is returned.
    /// Otherwise, the shift is performed based on the `op`:
    /// - LogicalLeft: `src_val << effective_shift`
    /// - LogicalRight: `src_val >> effective_shift`
    /// - ArithmeticRight: arithmetic right shift preserving the sign bit.
    pub fn calculate_result(src_val: u32, shift_amount: u32, op: &ShiftOperation) -> u32 {
        let effective_shift = shift_amount & 0x1f;
        if effective_shift == 0 {
            return src_val;
        }
        match op {
            ShiftOperation::LogicalLeft => src_val << effective_shift,
            ShiftOperation::LogicalRight => src_val >> effective_shift,
            ShiftOperation::ArithmeticRight => ((src_val as i32) >> effective_shift) as u32,
        }
    }

    /// Generate a ShiftEvent for immediate shift operations.
    ///
    /// For immediate shifts (like SLLI, SRLI, SRAI), the shift amount comes
    /// directly from the instruction (as a 16-bit immediate) and masked to 5
    /// bits.
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
        event::{
            ret::RetEvent,
            test_utils::{vrom_set_value_at_offset, TestEnv},
        },
        memory::Memory,
        opcodes::Opcode,
        util::code_to_prom,
        ValueRom,
    };

    #[test]
    fn test_shift_event_calculate_comprehensive() {
        // Each tuple is:
        // (src_val, shift_amount, expected_left, expected_right, expected_arith,
        // description)
        let test_cases = [
            (
                0x00000001,
                0,
                0x00000001,
                0x00000001,
                0x00000001,
                "identity shift (0)",
            ),
            (
                0x00000001,
                1,
                0x00000002,
                0x00000000,
                0x00000000,
                "shift by 1",
            ),
            (
                0x00000001,
                31,
                0x80000000,
                0x00000000,
                0x00000000,
                "shift by 31",
            ),
            (
                0x80000000,
                1,
                0x00000000,
                0x40000000,
                0xc0000000,
                "negative value, shift by 1",
            ),
            (
                0x80000000,
                31,
                0x00000000,
                0x00000001,
                0xffffffff,
                "negative value, shift by 31",
            ),
            (
                0x12345678,
                32,
                0x12345678,
                0x12345678,
                0x12345678,
                "shift by 32 (mod 32 => 0)",
            ),
            (
                0x12345678,
                33,
                0x2468acf0,
                0x091a2b3c,
                0x091a2b3c,
                "shift by 33 (effective shift 1)",
            ),
            (
                0x80000000,
                100,
                0x00000000,
                0x08000000,
                0xf8000000,
                "shift by 100 (effective shift 4)",
            ),
        ];

        for (src_val, shift_amount, expected_left, expected_right, expected_arith, desc) in
            test_cases
        {
            let result_left =
                ShiftEvent::calculate_result(src_val, shift_amount, &ShiftOperation::LogicalLeft);
            let result_right =
                ShiftEvent::calculate_result(src_val, shift_amount, &ShiftOperation::LogicalRight);
            let result_arith = ShiftEvent::calculate_result(
                src_val,
                shift_amount,
                &ShiftOperation::ArithmeticRight,
            );

            assert_eq!(
                result_left, expected_left,
                "LogicalLeft failed for {}: expected 0x{:08x}, got 0x{:08x}",
                desc, expected_left, result_left
            );
            assert_eq!(
                result_right, expected_right,
                "LogicalRight failed for {}: expected 0x{:08x}, got 0x{:08x}",
                desc, expected_right, result_right
            );
            assert_eq!(
                result_arith, expected_arith,
                "ArithmeticRight failed for {}: expected 0x{:08x}, got 0x{:08x}",
                desc, expected_arith, result_arith
            );
        }
    }

    #[test]
    fn test_shift_event_integration() {
        let zero = BinaryField16b::zero();

        // Initialize VROM
        let mut vrom = ValueRom::default();
        vrom.set_u32(0, 0).unwrap(); // Return PC
        vrom.set_u32(1, 0).unwrap(); // Return FP

        // Create source value slots
        let src_pos = vrom_set_value_at_offset(&mut vrom, 2, 0x00000003);
        let src_neg = vrom_set_value_at_offset(&mut vrom, 3, 0x80000000);

        // Create shift amount slots
        let shift_zero = vrom_set_value_at_offset(&mut vrom, 4, 0);
        let shift_normal = vrom_set_value_at_offset(&mut vrom, 5, 3);
        let shift_32 = vrom_set_value_at_offset(&mut vrom, 6, 32);

        // Create destination slots
        let slli_result = BinaryField16b::new(10);
        let srli_result = BinaryField16b::new(11);
        let srai_result = BinaryField16b::new(12);
        let slli_zero_result = BinaryField16b::new(13);
        let sll_result = BinaryField16b::new(14);
        let srl_result = BinaryField16b::new(15);
        let sra_result = BinaryField16b::new(16);
        let sll_zero_result = BinaryField16b::new(17);
        let srl_32_result = BinaryField16b::new(18);
        let sra_32_result = BinaryField16b::new(19);

        // Build a sequence of instructions
        let instructions = vec![
            // Immediate shift operations with normal shift amount (3)
            [
                Opcode::Slli.get_field_elt(),
                slli_result,
                src_pos,
                BinaryField16b::new(3),
            ],
            [
                Opcode::Srli.get_field_elt(),
                srli_result,
                src_pos,
                BinaryField16b::new(3),
            ],
            [
                Opcode::Srai.get_field_elt(),
                srai_result,
                src_neg,
                BinaryField16b::new(3),
            ],
            // Edge case: immediate shift by 0
            [
                Opcode::Slli.get_field_elt(),
                slli_zero_result,
                src_pos,
                zero,
            ],
            // VROM-based shift operations with normal shift amount
            [
                Opcode::Sll.get_field_elt(),
                sll_result,
                src_pos,
                shift_normal,
            ],
            [
                Opcode::Srl.get_field_elt(),
                srl_result,
                src_pos,
                shift_normal,
            ],
            [
                Opcode::Sra.get_field_elt(),
                sra_result,
                src_neg,
                shift_normal,
            ],
            // Edge case: VROM-based shift by 0
            [
                Opcode::Sll.get_field_elt(),
                sll_zero_result,
                src_pos,
                shift_zero,
            ],
            // Edge case: VROM-based shift by 32 (mod 32 → 0, so no shift)
            [
                Opcode::Srl.get_field_elt(),
                srl_32_result,
                src_pos,
                shift_32,
            ],
            [
                Opcode::Sra.get_field_elt(),
                sra_32_result,
                src_neg,
                shift_32,
            ],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];

        let frame_size = 20; // Highest used offset + 1

        let mut frames = HashMap::new();
        frames.insert(BinaryField32b::ONE, frame_size);

        let prom = code_to_prom(&instructions);
        let memory = Memory::new(prom, vrom);

        let (trace, _) = ZCrayTrace::generate(memory, frames, HashMap::new())
            .expect("Trace generation should not fail.");

        // Check results for immediate shift operations
        assert_eq!(
            trace.get_vrom_u32(slli_result.val() as u32).unwrap(),
            0x00000018,
            "SLLI: 3 << 3 should be 24 (0x00000018)"
        );

        assert_eq!(
            trace.get_vrom_u32(srli_result.val() as u32).unwrap(),
            0x00000000,
            "SRLI: 3 >> 3 should be 0"
        );

        assert_eq!(
            trace.get_vrom_u32(srai_result.val() as u32).unwrap(),
            0xf0000000,
            "SRAI: 0x80000000 >> 3 (arithmetic) should be 0xF0000000"
        );

        // Check edge case: immediate shift by 0
        assert_eq!(
            trace.get_vrom_u32(slli_zero_result.val() as u32).unwrap(),
            0x00000003,
            "Shift by 0 should return original value"
        );

        // Check results for VROM-based shift operations
        assert_eq!(
            trace.get_vrom_u32(sll_result.val() as u32).unwrap(),
            0x00000018,
            "SLL: 3 << 3 should be 24 (0x00000018)"
        );

        assert_eq!(
            trace.get_vrom_u32(srl_result.val() as u32).unwrap(),
            0x00000000,
            "SRL: 3 >> 3 should be 0"
        );

        assert_eq!(
            trace.get_vrom_u32(sra_result.val() as u32).unwrap(),
            0xf0000000,
            "SRA: 0x80000000 >> 3 (arithmetic) should be 0xF0000000"
        );

        // Check VROM-based edge cases (modular behavior):
        // A shift by 32 is equivalent to a shift by 0.
        assert_eq!(
            trace.get_vrom_u32(sll_zero_result.val() as u32).unwrap(),
            0x00000003,
            "VROM-based shift by 0 should return original value"
        );

        // For shift amount 32, effective shift = 0, so original value is returned.
        assert_eq!(
            trace.get_vrom_u32(srl_32_result.val() as u32).unwrap(),
            0x00000003,
            "SRL by 32 should return original value (mod 32 behavior)"
        );

        assert_eq!(
            trace.get_vrom_u32(sra_32_result.val() as u32).unwrap(),
            0x80000000,
            "SRA by 32 on negative value should return original value (mod 32 behavior)"
        );
    }
}
