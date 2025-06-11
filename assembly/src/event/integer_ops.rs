use core::fmt::Debug;
use std::marker::PhantomData;

use binius_m3::builder::{B16, B32};

use super::context::EventContext;
use crate::macros::{define_bin32_imm_op_event, define_bin32_op_event, fire_non_jump_event};
use crate::{
    event::{binary_ops::*, Event},
    execution::{FramePointer, InterpreterChannels, InterpreterError},
};

define_bin32_imm_op_event!(
    /// Event for ADDI.
    ///
    /// Performs an ADD between a target address and an immediate.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src] + imm
    AddiEvent,
    addi,
    |a: B32, imm: B16| B32::new((a.val() as i32).wrapping_add(imm.val() as i16 as i32) as u32)
);

// Note: The addition is checked thanks to the ADD32 table.
define_bin32_op_event!(
    /// Event for ADD.
    ///
    /// Performs an ADD between two target addresses.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src1] + FP[src2]
    AddEvent,
    add,
    |a: B32, b: B32| B32::new((a.val() as i32).wrapping_add(b.val() as i32) as u32)
);

/// Event for MULI.
///
/// Performs a MUL between a signed 32-bit integer and a 16-bit immediate.
#[derive(Debug, Clone)]
pub struct MuliEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub dst: u16,
    pub dst_val: u64,
    pub src: u16,
    pub src_val: u32,
    pub imm: u16,
}

impl Event for MuliEvent {
    fn generate(
        ctx: &mut EventContext,
        dst: B16,
        src: B16,
        imm: B16,
    ) -> Result<(), InterpreterError> {
        let src_val = ctx.vrom_read::<u32>(ctx.addr(src.val()))?;

        let imm_val = imm.val();
        let dst_val = (src_val as i32 as i64).wrapping_mul(imm_val as i16 as i64) as u64;
        ctx.vrom_write(ctx.addr(dst.val()), dst_val)?;

        if !ctx.prover_only {
            let (_pc, field_pc, fp, timestamp) = ctx.program_state();

            let event = Self {
                pc: field_pc,
                fp,
                timestamp,
                dst: dst.val(),
                dst_val,
                src: src.val(),
                src_val,
                imm: imm_val,
            };

            ctx.trace.muli.push(event);
        }
        ctx.incr_counters();
        Ok(())
    }

    fn fire(&self, channels: &mut InterpreterChannels) {
        assert_eq!(
            self.dst_val,
            (self.src_val as i32 as i64).wrapping_mul(self.imm as i16 as i64) as u64
        );
        fire_non_jump_event!(self, channels);
    }
}

/// Event for MULU.
///
/// Performs a MULU between two unsigned 32-bit integers. Returns a 64-bit
/// result.
#[derive(Debug, Clone)]
pub struct MuluEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub dst: u16,
    pub dst_val: u64,
    pub src1: u16,
    pub src1_val: u32,
    pub src2: u16,
    pub src2_val: u32,
}

impl Event for MuluEvent {
    fn generate(
        ctx: &mut EventContext,
        dst: B16,
        src1: B16,
        src2: B16,
    ) -> Result<(), InterpreterError> {
        let src1_val = ctx.vrom_read::<u32>(ctx.addr(src1.val()))?;
        let src2_val = ctx.vrom_read::<u32>(ctx.addr(src2.val()))?;

        let dst_val = (src1_val as u64).wrapping_mul(src2_val as u64);
        ctx.vrom_write(ctx.addr(dst.val()), dst_val)?;

        if !ctx.prover_only {
            let (_pc, field_pc, fp, timestamp) = ctx.program_state();

            let mulu_event = Self {
                pc: field_pc,
                fp,
                timestamp,
                dst: dst.val(),
                dst_val,
                src1: src1.val(),
                src1_val,
                src2: src2.val(),
                src2_val,
            };

            ctx.trace.mulu.push(mulu_event);
        }
        ctx.incr_counters();
        Ok(())
    }

    fn fire(&self, channels: &mut InterpreterChannels) {
        assert_eq!(
            self.dst_val,
            (self.src1_val as u64).wrapping_mul(self.src2_val as u64)
        );
        fire_non_jump_event!(self, channels);
    }
}

pub trait SignedMulOperation: Debug + Clone {
    fn mul_op(input1: u32, input2: u32) -> u64;
}

#[derive(Debug, Clone)]
pub struct MulsuOp;
impl SignedMulOperation for MulsuOp {
    fn mul_op(input1: u32, input2: u32) -> u64 {
        // If the value is signed, first turn into an i32 to get the sign, then into an
        // i64 to get the 64-bit value. Otherwise, directly cast as an i64 for
        // the multiplication.
        (input1 as i32 as i64).wrapping_mul(input2 as i64) as u64
    }
}

#[derive(Debug, Clone)]
pub struct MulOp;
impl SignedMulOperation for MulOp {
    fn mul_op(input1: u32, input2: u32) -> u64 {
        // If the value is signed, first turn into an i32 to get the sign, then into an
        // i64 to get the 64-bit value. Otherwise, directly cast as an i64 for
        // the multiplication.
        (input1 as i32 as i64).wrapping_mul(input2 as i32 as i64) as u64
    }
}

/// Convenience macro to implement the [`Event`] trait for signed mul events.
///
/// It takes as argument the field name of the instruction within the
/// [`PetraTrace`](crate::execution::PetraTrace) object, and the corresponding
/// instruction's [`Event`].
///
/// # Example
///
/// ```ignore
/// impl_signed_mul_event!(mul, MulEvent);
macro_rules! impl_signed_mul_event {
    ($variant:ident, $ty:ty, $op:ty) => {
        impl Event for $ty {
            fn generate(
                ctx: &mut EventContext,
                dst: B16,
                src1: B16,
                src2: B16,
            ) -> Result<(), InterpreterError> {
                let src1_val = ctx.vrom_read::<u32>(ctx.addr(src1.val()))?;
                let src2_val = ctx.vrom_read::<u32>(ctx.addr(src2.val()))?;

                let dst_val = <$op>::mul_op(src1_val, src2_val);
                ctx.vrom_write(ctx.addr(dst.val()), dst_val)?;

                if !ctx.prover_only {
                    let (_pc, field_pc, fp, timestamp) = ctx.program_state();

                    let event = Self {
                        pc: field_pc,
                        fp,
                        timestamp,
                        dst: dst.val(),
                        dst_val,
                        src1: src1.val(),
                        src1_val,
                        src2: src2.val(),
                        src2_val,
                        _phantom: PhantomData,
                    };

                    ctx.trace.$variant.push(event);
                }
                ctx.incr_counters();
                Ok(())
            }

            fn fire(&self, channels: &mut InterpreterChannels) {
                assert_eq!(self.dst_val, <$op>::mul_op(self.src1_val, self.src2_val));
                fire_non_jump_event!(self, channels);
            }
        }
    };
}

impl_signed_mul_event!(mul, MulEvent, MulOp);
impl_signed_mul_event!(mulsu, MulsuEvent, MulsuOp);

/// Event for MUL or MULSU.
///
/// Performs a MUL between two signed 32-bit integers.
#[derive(Debug, Clone)]
pub struct SignedMulEvent<SignedMulOperation> {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub dst: u16,
    pub dst_val: u64,
    pub src1: u16,
    pub src1_val: u32,
    pub src2: u16,
    pub src2_val: u32,

    _phantom: PhantomData<SignedMulOperation>,
}

pub type MulEvent = SignedMulEvent<MulOp>;
pub type MulsuEvent = SignedMulEvent<MulsuOp>;

define_bin32_op_event!(
    // Event for SUB.
    ///
    /// Performs a SUB between two target addresses.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src1] - FP[src2]
    SubEvent,
    sub,
    // SUB is checked using a specific gadget, similarly to ADD.
    |a: B32, b: B32| B32::new(((a.val() as i32).wrapping_sub(b.val() as i32)) as u32)
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::get_last_event;
    use crate::{execution::Interpreter, PetraTrace};

    /// Tests for Add operations (without immediate)
    #[test]
    fn test_add_operations() {
        // Test cases for ADD
        let test_cases = [
            // (src1_val, src2_val, expected_result, description)
            (10, 20, 30, "simple addition"),
            (0, 0, 0, "zero addition"),
            (u32::MAX, 1, 0, "overflow"),
            (0x7FFFFFFF, 1, 0x80000000, "positive to negative overflow"),
            (
                0x80000000,
                0xFFFFFFFF,
                0x7FFFFFFF,
                "negative to positive underflow",
            ),
            (0x1000, 0x7FFF, 0x8FFF, "add values"),
            (
                0x1000,
                0xFFFF8000,
                0xFFFF9000,
                "add sign-extended negative value",
            ),
            (0x1000, 0xFFFFFFFF, 0xFFF, "add -1 value"),
        ];

        for (src1_val, src2_val, expected, desc) in test_cases {
            let mut interpreter = Interpreter::default();
            let mut trace = PetraTrace::default();
            let mut ctx = EventContext::new(&mut interpreter, &mut trace);
            let src1_offset = B16::new(2);
            let src2_offset = B16::new(3);
            let dst_offset = B16::new(4);

            // Set values in VROM at the computed addresses (FP ^ offset)
            ctx.set_vrom(src1_offset.val(), src1_val);
            ctx.set_vrom(src2_offset.val(), src2_val);

            AddEvent::generate(&mut ctx, dst_offset, src1_offset, src2_offset).unwrap();
            let event = get_last_event!(ctx, add);

            assert_eq!(
                event.dst_val, expected,
                "ADD failed for {}: expected 0x{:x} got 0x{:x} (src1=0x{:x}, src2=0x{:x})",
                desc, expected, event.dst_val, src1_val, src2_val
            );
        }
    }

    /// Tests for Sub operations
    #[test]
    fn test_sub_operations() {
        // Test cases for SUB
        let test_cases = [
            // (src1_val, src2_val, expected_result, description)
            (30, 20, 10, "simple subtraction"),
            (
                20,
                30,
                0xFFFFFFF6,
                "negative result (-10 in two's complement)",
            ),
            (0, 0, 0, "zero subtraction"),
            (0, 1, 0xFFFFFFFF, "0 - 1 = -1 (underflow to max u32)"),
            (
                0x80000000,
                1,
                0x7FFFFFFF,
                "MIN_INT - 1 = MAX_INT (underflow)",
            ),
            (
                0x7FFFFFFF,
                0xFFFFFFFF,
                0x80000000,
                "MAX_INT - (-1) = MIN_INT (overflow)",
            ),
            (0x7FFFFFFF, 0x7FFFFFFF, 0, "MAX_INT - MAX_INT = 0"),
            (0x80000000, 0x80000000, 0, "MIN_INT - MIN_INT = 0"),
            (
                0,
                0x80000000,
                0x80000000,
                "0 - MIN_INT = MIN_INT (positive overflow)",
            ),
            (0xFFFFFFFF, 0x7FFFFFFF, 0x80000000, "-1 - MAX_INT = MIN_INT"),
            (0x80000000, 0x7FFFFFFF, 0x00000001, "MIN_INT - MAX_INT = 1"),
            (0xFFFFFFFF, 0xFFFFFFFF, 0, "-1 - (-1) = 0"),
            (0x12345678, 0x12345678, 0, "arbitrary value - itself = 0"),
        ];

        for (src1_val, src2_val, expected, desc) in test_cases {
            let mut interpreter = Interpreter::default();
            let mut trace = PetraTrace::default();
            let mut ctx = EventContext::new(&mut interpreter, &mut trace);
            let src1_offset = B16::new(2);
            let src2_offset = B16::new(3);
            let dst_offset = B16::new(4);

            // Set values in VROM at the computed addresses (FP ^ offset)
            ctx.set_vrom(src1_offset.val(), src1_val);
            ctx.set_vrom(src2_offset.val(), src2_val);

            SubEvent::generate(&mut ctx, dst_offset, src1_offset, src2_offset).unwrap();
            let event = get_last_event!(ctx, sub);

            assert_eq!(
                event.dst_val, expected,
                "SUB failed for {}: expected 0x{:x} got 0x{:x} (src1=0x{:x}, src2=0x{:x})",
                desc, expected, event.dst_val, src1_val, src2_val
            );
        }
    }

    /// Tests for Addi operations
    #[test]
    fn test_addi_operations() {
        // Test cases for ADDI with sign extension
        let test_cases = [
            // (src_val, imm_val, expected_result, description)
            (10, 20, 30, "simple addition"),
            (0, 0, 0, "zero addition"),
            (u32::MAX, 1, 0, "overflow"),
            (0x7FFFFFFF, 1, 0x80000000, "positive to negative overflow"),
            (0x1000, 0x7FFF, 0x8FFF, "add max positive immediate"),
            // Sign extension tests
            (
                0x1000,
                0x8000,
                0xFFFF9000,
                "add min negative immediate (0x8000 -> -32768)",
            ),
            (0x1000, 0xFFFF, 0xFFF, "add -1 immediate (0xFFFF -> -1)"),
            (0xFFFFFFFF, 0xFFFF, 0xFFFFFFFE, "add -1 to -1 (wrapped)"),
            (
                0x80000000,
                0x8000,
                0x7FFF8000,
                "add min negative immediate to min int (overflow)",
            ),
        ];

        for (src_val, imm_val, expected, desc) in test_cases {
            let mut interpreter = Interpreter::default();
            let mut trace = PetraTrace::default();
            let mut ctx = EventContext::new(&mut interpreter, &mut trace);
            let src_offset = B16::new(2);
            let dst_offset = B16::new(4);

            // Set value in VROM at the computed address (FP ^ offset)
            ctx.set_vrom(src_offset.val(), src_val);
            let imm = B16::new(imm_val);

            AddiEvent::generate(&mut ctx, dst_offset, src_offset, imm).unwrap();
            let event = get_last_event!(ctx, addi);

            assert_eq!(
                event.dst_val, expected,
                "ADDI failed for {}: expected 0x{:x} got 0x{:x} (src=0x{:x}, imm=0x{:x})",
                desc, expected, event.dst_val, src_val, imm_val
            );
        }
    }

    /// Tests for Mul operations (without immediate)
    #[test]
    fn test_mul_operations() {
        // Test cases for MUL, MULU, MULSU
        let test_cases = [
            // (src1_val, src2_val, mul_expected, mulu_expected, mulsu_expected, description)
            (5, 7, 35, 35, 35, "simple multiplication"),
            (0, 0, 0, 0, 0, "zero multiplication"),
            (1, 0, 0, 0, 0, "identity with zero"),
            (0, 1, 0, 0, 0, "zero with identity"),
            (1, 1, 1, 1, 1, "identity"),
            // Negative source values - different behavior between signed and unsigned
            (
                0xFFFFFFFF,
                2,
                (-2i64 as u64),
                0x1FFFFFFFE,
                (-2i64 as u64),
                "negative * positive",
            ),
            (
                0xFFFFFFFF,
                0xFFFFFFFF,
                1,
                0xFFFFFFFE00000001,
                0xFFFFFFFF00000001,
                "negative * negative",
            ),
            (
                0xFFFFFFFF,
                0x80000000,
                0x80000000,
                0x7fffffff80000000,
                0xffffffff80000000,
                "negative * min negative",
            ),
            // Large value edge cases
            (
                0x7FFFFFFF,
                2,
                0xFFFFFFFE,
                0xFFFFFFFE,
                0xFFFFFFFE,
                "max positive * 2",
            ),
            (
                0x7FFFFFFF,
                0x7FFFFFFF,
                0x3FFFFFFF00000001,
                0x3FFFFFFF00000001,
                0x3FFFFFFF00000001,
                "max positive * max positive",
            ),
            (
                0x80000000,
                2,
                0xffffffff00000000,
                0x100000000,
                0xffffffff00000000,
                "min negative * 2",
            ),
            (
                0x80000000,
                0x80000000,
                0x4000000000000000,
                0x4000000000000000,
                0xc000000000000000,
                "min negative * min negative",
            ),
        ];

        for (src1_val, src2_val, mul_expected, mulu_expected, mulsu_expected, desc) in test_cases {
            // Test MUL (sign * sign)
            let mut interpreter = Interpreter::default();
            let mut trace = PetraTrace::default();
            let mut ctx = EventContext::new(&mut interpreter, &mut trace);
            let src1_offset = B16::new(2);
            let src2_offset = B16::new(3);
            let dst_offset = B16::new(4);

            // Set values in VROM at the computed addresses (FP ^ offset)
            ctx.set_vrom(src1_offset.val(), src1_val);
            ctx.set_vrom(src2_offset.val(), src2_val);

            SignedMulEvent::<MulOp>::generate(&mut ctx, dst_offset, src1_offset, src2_offset)
                .unwrap();

            // Extract the event
            let event = get_last_event!(ctx, mul);

            assert_eq!(
                event.dst_val, mul_expected,
                "MUL failed for {}: expected 0x{:x} got 0x{:x} (src1=0x{:x}, src2=0x{:x})",
                desc, mul_expected, event.dst_val, src1_val, src2_val
            );

            // Test MULU (unsigned * unsigned)

            let mut interpreter = Interpreter::default();
            interpreter.timestamp = 0;
            interpreter.pc = 1;

            let mut interpreter = Interpreter::default();
            let mut trace = PetraTrace::default();
            let mut ctx = EventContext::new(&mut interpreter, &mut trace);
            ctx.set_vrom(src1_offset.val(), src1_val);
            ctx.set_vrom(src2_offset.val(), src2_val);

            MuluEvent::generate(&mut ctx, dst_offset, src1_offset, src2_offset).unwrap();
            let event = get_last_event!(ctx, mulu);

            assert_eq!(
                event.dst_val, mulu_expected,
                "MULU failed for {}: expected 0x{:x} got 0x{:x} (src1=0x{:x}, src2=0x{:x})",
                desc, mulu_expected, event.dst_val, src1_val, src2_val
            );

            // Test MULSU (sign * unsigned)
            let mut interpreter = Interpreter::default();
            let mut trace = PetraTrace::default();
            let mut ctx = EventContext::new(&mut interpreter, &mut trace);
            ctx.set_vrom(src1_offset.val(), src1_val);
            ctx.set_vrom(src2_offset.val(), src2_val);

            SignedMulEvent::<MulsuOp>::generate(&mut ctx, dst_offset, src1_offset, src2_offset)
                .unwrap();

            // Extract the event
            let event = get_last_event!(ctx, mulsu);

            assert_eq!(
                event.dst_val, mulsu_expected,
                "MULSU failed for {}: expected 0x{:x} got 0x{:x} (src1=0x{:x}, src2=0x{:x})",
                desc, mulsu_expected, event.dst_val, src1_val, src2_val
            );
        }
    }

    /// Tests for Mul operations (with immediate)
    #[test]
    fn test_muli_operations() {
        // Test cases for MULI (sign-extends the immediate)
        let test_cases = [
            // (src_val, imm_val, expected_result, description)
            (5, 7, 35, "simple multiplication"),
            (0, 0, 0, "zero multiplication"),
            (1, 0, 0, "identity with zero"),
            (0, 1, 0, "zero with identity"),
            (1, 1, 1, "identity"),
            // Immediate sign extension tests
            (5, 0x7FFF, 163835, "multiply by max positive 16-bit"),
            (
                5,
                0x8000,
                0xfffffffffffd8000,
                "multiply by 0x8000 (sign extends to -32768)",
            ),
            (
                5,
                0xFFFF,
                (-5i64 as u64),
                "multiply by 0xFFFF (sign extends to -1)",
            ),
            // Tests with negative source values
            (0xFFFFFFFF, 2, (-2i64 as u64), "negative * positive"),
            (0x80000000, 2, 0xffffffff00000000, "min negative * 2"),
            // Edge cases with 16-bit immediate
            (
                0x10000,
                0x7FFF,
                0x7FFF0000,
                "multiply by max positive 16-bit",
            ),
            (
                0x10000,
                0x8000,
                0xffffffff80000000,
                "multiply by min negative 16-bit",
            ),
            (0x10000, 0xFFFF, 0xffffffffffff0000, "multiply by -1"),
        ];

        for (src_val, imm_val, expected, desc) in test_cases {
            let mut interpreter = Interpreter::default();
            let mut trace = PetraTrace::default();
            let mut ctx = EventContext::new(&mut interpreter, &mut trace);
            let src_offset = B16::new(2);
            let dst_offset = B16::new(4);

            // Set value in VROM at the computed address (FP ^ offset)
            ctx.set_vrom(src_offset.val(), src_val);
            let imm = B16::new(imm_val);

            MuliEvent::generate(&mut ctx, dst_offset, src_offset, imm).unwrap();

            // Extract the event
            let event = get_last_event!(ctx, muli);

            assert_eq!(
                event.dst_val, expected,
                "MULI failed for {}: expected 0x{:x} got 0x{:x} (src=0x{:x}, imm=0x{:x})",
                desc, expected, event.dst_val, src_val, imm_val
            );
        }
    }
}
