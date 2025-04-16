use binius_m3::builder::{B128, B16, B32};

use super::BinaryOperation;
use crate::{
    define_bin128_op_event,
    event::{context::EventContext, Event},
    execution::{FramePointer, InterpreterChannels, InterpreterError, G},
};

define_bin128_op_event!(
    /// Event for B128_ADD.
    ///
    /// Performs a 128-bit binary field addition (XOR) between two target addresses.
    ///
    /// Logic:
    ///   1. FP[dst] = __b128_add(FP[src1], FP[src2])
    B128AddEvent,
    b128_add,
    +
);

define_bin128_op_event!(
    /// Event for B128_MUL.
    ///
    /// Performs a 128-bit binary field multiplication between two target addresses.
    ///
    /// Logic:
    ///   1. FP[dst] = __b128_mul(FP[src1], FP[src2])
    B128MulEvent,
    b128_mul,
    *
);

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use binius_field::{Field, PackedField};

    use super::*;
    use crate::{
        isa::GenericISA,
        memory::{Memory, ValueRom},
        opcodes::Opcode,
        util::code_to_prom,
        ZCrayTrace,
    };

    #[test]
    fn test_b128_add_operation() {
        // Test the basic operation logic directly
        let val1 = 0x1111111122222222u128 | (0x3333333344444444u128 << 64);
        let val2 = 0x5555555566666666u128 | (0x7777777788888888u128 << 64);

        let bf1 = B128::new(val1);
        let bf2 = B128::new(val2);

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

        let bf1 = B128::new(val1);
        let bf2 = B128::new(val2);

        // Test the multiplication operation
        let result = B128MulEvent::operation(bf1, bf2);
        let expected = bf1 * bf2;

        assert_eq!(result, expected);
    }
    #[test]
    fn test_b128_operations_program() {
        // Define opcodes and test values
        let zero = B16::zero();

        // Offsets/addresses in our test program
        let a_offset = 4; // Must be 4-slot aligned
        let b_offset = 8; // Must be 4-slot aligned
        let c_offset = 12; // Must be 4-slot aligned
        let add_result_offset = 16; // Must be 4-slot aligned
        let mul_result_offset = 20; // Must be 4-slot aligned

        // Create binary field slot references
        let a_slot = B16::new(a_offset as u16);
        let b_slot = B16::new(b_offset as u16);
        let c_slot = B16::new(c_offset as u16);
        let add_result_slot = B16::new(add_result_offset as u16);
        let mul_result_slot = B16::new(mul_result_offset as u16);

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
        let prom = code_to_prom(&instructions);

        // Test values
        let a_val = 0x1111111122222222u128 | (0x3333333344444444u128 << 64);
        let b_val = 0x5555555566666666u128 | (0x7777777788888888u128 << 64);
        let c_val = 0x9999999988888888u128 | (0x7777777766666666u128 << 64);

        let init_values = vec![
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
            (c_val >> 96) as u32, /* 0x77777777
                                   * Space for results (8 more slots for add_result and
                                   * mul_result) */
        ];

        let vrom = ValueRom::new_with_init_vals(&init_values);
        let memory = Memory::new(prom, vrom);

        // Set up frame sizes
        let mut frames = HashMap::new();
        frames.insert(B32::ONE, 24);

        // Create an interpreter and run the program
        let (trace, boundary_values) =
            ZCrayTrace::generate(Box::new(GenericISA), memory, frames, HashMap::new())
                .expect("Trace generation should not fail.");

        // Capture the final PC before boundary_values is moved
        let final_pc = boundary_values.final_pc;

        // Validate the trace (this consumes boundary_values)
        trace.validate(boundary_values);

        // Calculate the expected results
        let expected_add = a_val ^ b_val;
        let a_bf = B128::new(a_val);
        let b_bf = B128::new(b_val);
        let c_bf = B128::new(c_val);
        let add_result_bf = a_bf + b_bf;
        let expected_mul = (add_result_bf * c_bf).val();

        // Verify the results in VROM
        let actual_add = trace.vrom().read::<u128>(add_result_offset).unwrap();
        let actual_mul = trace.vrom().read::<u128>(mul_result_offset).unwrap();

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
        assert_eq!(final_pc, B32::ZERO, "Program did not end correctly");
    }
}
