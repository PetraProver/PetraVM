use binius_field::{BinaryField128b, BinaryField16b, BinaryField32b};

use super::Event;
use crate::{
    event::BinaryOperation,
    execution::{Interpreter, InterpreterChannels, InterpreterError, InterpreterTables},
    fire_non_jump_event, ZCrayTrace, G,
};

/// Event for B128_ADD.
///
/// Performs a 128-bit binary field addition (XOR) between two target addresses.
///
/// Logic:
///   1. FP[dst] = __b128_add(FP[src1], FP[src2])
#[derive(Debug, Clone)]
pub(crate) struct B128AddEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    dst: u16,
    dst_val: u128,
    src1: u16,
    src1_val: u128,
    src2: u16,
    src2_val: u128,
}

impl B128AddEvent {
    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let fp = interpreter.fp;

        // Calculate addresses
        let dst_addr = fp ^ dst.val() as u32;
        let src1_addr = fp ^ src1.val() as u32;
        let src2_addr = fp ^ src2.val() as u32;

        // Get source values
        let src1_val = interpreter.get_vrom_u128(src1_addr)?;
        let src2_val = interpreter.get_vrom_u128(src2_addr)?;

        // In binary fields, addition is XOR
        let dst_val = src1_val ^ src2_val;

        // Store result
        interpreter.set_vrom_u128(trace, dst_addr, dst_val)?;

        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;
        interpreter.incr_pc();

        Ok(Self {
            timestamp,
            pc: field_pc,
            fp,
            dst: dst.val(),
            dst_val,
            src1: src1.val(),
            src1_val,
            src2: src2.val(),
            src2_val,
        })
    }
}

impl BinaryOperation for B128AddEvent {
    fn operation(val1: BinaryField128b, val2: BinaryField128b) -> BinaryField128b {
        // In binary fields, addition is XOR
        val1 + val2
    }
}

impl super::LeftOp for B128AddEvent {
    type Left = BinaryField128b;

    fn left(&self) -> BinaryField128b {
        BinaryField128b::new(self.src1_val)
    }
}

impl super::RigthOp for B128AddEvent {
    type Right = BinaryField128b;

    fn right(&self) -> BinaryField128b {
        BinaryField128b::new(self.src2_val)
    }
}

impl super::OutputOp for B128AddEvent {
    type Output = BinaryField128b;

    fn output(&self) -> BinaryField128b {
        BinaryField128b::new(self.dst_val)
    }
}

impl Event for B128AddEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        use super::{LeftOp, OutputOp, RigthOp};

        // Verify that the result is correct (XOR of inputs)
        assert_eq!(self.output(), Self::operation(self.left(), self.right()));

        // Update state channel
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.pc * G, self.fp, self.timestamp + 1));
    }
}

/// Event for B128_MUL.
///
/// Performs a 128-bit binary field multiplication between two target addresses.
///
/// Logic:
///   1. FP[dst] = __b128_mul(FP[src1], FP[src2])
#[derive(Debug, Clone)]
pub(crate) struct B128MulEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    dst: u16,
    dst_val: u128,
    src1: u16,
    src1_val: u128,
    src2: u16,
    src2_val: u128,
}

impl B128MulEvent {
    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let fp = interpreter.fp;

        // Calculate addresses
        let dst_addr = fp ^ dst.val() as u32;
        let src1_addr = fp ^ src1.val() as u32;
        let src2_addr = fp ^ src2.val() as u32;

        // Get source values
        let src1_val = interpreter.get_vrom_u128(src1_addr)?;
        let src2_val = interpreter.get_vrom_u128(src2_addr)?;

        // Binary field multiplication
        let src1_bf = BinaryField128b::new(src1_val);
        let src2_bf = BinaryField128b::new(src2_val);
        let dst_bf = src1_bf * src2_bf;
        let dst_val = dst_bf.val();

        // Store result
        interpreter.set_vrom_u128(trace, dst_addr, dst_val)?;

        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;
        interpreter.incr_pc();

        Ok(Self {
            timestamp,
            pc: field_pc,
            fp,
            dst: dst.val(),
            dst_val,
            src1: src1.val(),
            src1_val,
            src2: src2.val(),
            src2_val,
        })
    }
}

impl BinaryOperation for B128MulEvent {
    fn operation(val1: BinaryField128b, val2: BinaryField128b) -> BinaryField128b {
        val1 * val2
    }
}

impl super::LeftOp for B128MulEvent {
    type Left = BinaryField128b;

    fn left(&self) -> BinaryField128b {
        BinaryField128b::new(self.src1_val)
    }
}

impl super::RigthOp for B128MulEvent {
    type Right = BinaryField128b;

    fn right(&self) -> BinaryField128b {
        BinaryField128b::new(self.src2_val)
    }
}

impl super::OutputOp for B128MulEvent {
    type Output = BinaryField128b;

    fn output(&self) -> BinaryField128b {
        BinaryField128b::new(self.dst_val)
    }
}

impl Event for B128MulEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        use super::{LeftOp, OutputOp, RigthOp};

        // Verify that the result is correct
        assert_eq!(self.output(), Self::operation(self.left(), self.right()));

        // Update state channel
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.pc * G, self.fp, self.timestamp + 1));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use binius_field::{Field, PackedField};

    use super::*;
    use crate::{code_to_prom, opcodes::Opcode, ValueRom};

    #[test]
    fn test_b128_add_operation() {
        // Test the basic operation logic directly
        let val1 = 0x1111111122222222u128 | (0x3333333344444444u128 << 64);
        let val2 = 0x5555555566666666u128 | (0x7777777788888888u128 << 64);

        let bf1 = BinaryField128b::new(val1);
        let bf2 = BinaryField128b::new(val2);

        // The operation should be XOR
        let expected = val1 ^ val2;
        let result = B128AddEvent::operation(bf1, bf2);

        assert_eq!(result.val(), expected);
    }

    #[test]
    fn test_program_with_b128_ops() {
        // Test the basic operation logic directly
        let val1 = 0x0000000000000002u128;
        let val2 = 0x0000000000000003u128;

        let bf1 = BinaryField128b::new(val1);
        let bf2 = BinaryField128b::new(val2);

        // Test the multiplication operation
        let result = B128MulEvent::operation(bf1, bf2);
        let expected = bf1 * bf2;

        assert_eq!(result, expected);
    }
    #[test]
    fn test_b128_operations_program() {
        // Define opcodes and test values
        let zero = BinaryField16b::zero();

        // Offsets/addresses in our test program
        let a_offset = 4; // Must be 4-slot aligned
        let b_offset = 8; // Must be 4-slot aligned
        let c_offset = 12; // Must be 4-slot aligned
        let add_result_offset = 16; // Must be 4-slot aligned
        let mul_result_offset = 20; // Must be 4-slot aligned

        // Create binary field slot references
        let a_slot = BinaryField16b::new(a_offset as u16);
        let b_slot = BinaryField16b::new(b_offset as u16);
        let c_slot = BinaryField16b::new(c_offset as u16);
        let add_result_slot = BinaryField16b::new(add_result_offset as u16);
        let mul_result_slot = BinaryField16b::new(mul_result_offset as u16);

        // Construct a simple program with B128_ADD and B128_MUL instructions
        // 1. B128_ADD @add_result, @a, @b
        // 2. B128_MUL @mul_result, @add_result, @c
        // 3. RET
        let instructions = vec![
            [
                Opcode::B128Add.get_field_elt(),
                add_result_slot,
                a_slot,
                b_slot,
            ],
            [
                Opcode::B128Mul.get_field_elt(),
                mul_result_slot,
                add_result_slot,
                c_slot,
            ],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];

        // Create the PROM
        let prom = code_to_prom(&instructions, &vec![false; instructions.len()]);

        // Test values
        let a_val = 0x1111111122222222u128 | (0x3333333344444444u128 << 64);
        let b_val = 0x5555555566666666u128 | (0x7777777788888888u128 << 64);
        let c_val = 0x9999999988888888u128 | (0x7777777766666666u128 << 64);

        // Create a dummy trace only used to populate the initial VROM.
        let mut dummy_zcray = ZCrayTrace::default();

        let mut init_values = vec![
            // Return PC and FP
            0,
            0,
            // Padding to align a_val at offset 4
            0,
            0,
            // a_val broken into 4 u32 chunks (least significant bits first)
            a_val as u32,         // 0x22222222
            (a_val >> 32) as u32, // 0x11111111
            (a_val >> 64) as u32, // 0x44444444
            (a_val >> 96) as u32, // 0x33333333
            // b_val broken into 4 u32 chunks
            b_val as u32,         // 0x66666666
            (b_val >> 32) as u32, // 0x55555555
            (b_val >> 64) as u32, // 0x88888888
            (b_val >> 96) as u32, // 0x77777777
            // c_val broken into 4 u32 chunks
            c_val as u32,         // 0x88888888
            (c_val >> 32) as u32, // 0x99999999
            (c_val >> 64) as u32, // 0x66666666
            (c_val >> 96) as u32, // 0x77777777
            // Space for results (8 more slots for add_result and mul_result)
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];

        let vrom = ValueRom::new_with_init_values(init_values);

        // Set up frame sizes
        let mut frames = HashMap::new();
        frames.insert(BinaryField32b::ONE, 24);

        // Create an interpreter and run the program
        let (trace, boundary_values) =
            ZCrayTrace::generate_with_vrom(prom, vrom, frames, HashMap::new())
                .expect("Trace generation should not fail.");

        // Capture the final PC before boundary_values is moved
        let final_pc = boundary_values.final_pc;

        // Validate the trace (this consumes boundary_values)
        trace.validate(boundary_values);

        // Calculate the expected results
        let expected_add = a_val ^ b_val;
        let a_bf = BinaryField128b::new(a_val);
        let b_bf = BinaryField128b::new(b_val);
        let c_bf = BinaryField128b::new(c_val);
        let add_result_bf = a_bf + b_bf;
        let expected_mul = (add_result_bf * c_bf).val();

        // Verify the results in VROM
        let actual_add = trace.vrom.get_u128(add_result_offset).unwrap();
        let actual_mul = trace.vrom.get_u128(mul_result_offset).unwrap();

        assert_eq!(actual_add, expected_add, "B128_ADD operation failed");
        assert_eq!(actual_mul, expected_mul, "B128_MUL operation failed");

        // Check that the events were created
        assert_eq!(
            trace.b128_add.len(),
            1,
            "Expected exactly one B128_ADD event"
        );
        assert_eq!(
            trace.b128_mul.len(),
            1,
            "Expected exactly one B128_MUL event"
        );

        // The trace should have completed successfully
        assert_eq!(
            final_pc,
            BinaryField32b::ZERO,
            "Program did not end correctly"
        );
    }
}
