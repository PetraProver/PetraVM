use core::fmt::Debug;
use std::marker::PhantomData;

use binius_m3::builder::{B16, B32};

use super::context::EventContext;
use super::gadgets::right_logic_shift::RightLogicShiftExtension;
use crate::{
    event::Event,
    execution::{FramePointer, InterpreterChannels, InterpreterError},
    macros::fire_non_jump_event,
};

/// Marker trait to specify the kind of shift used by a [`ShiftEvent`].
pub trait ShiftOperation<S: ShiftSource>: Debug + Clone + PartialEq {
    fn shift_op(val: u32, shift: u32) -> u32;
}

#[derive(Debug, Clone, PartialEq)]
pub struct LogicalLeft;
impl ShiftOperation<ImmediateShift> for LogicalLeft {
    fn shift_op(val: u32, shift: u32) -> u32 {
        val << shift
    }
}

impl ShiftOperation<VromOffsetShift> for LogicalLeft {
    fn shift_op(val: u32, shift: u32) -> u32 {
        val << shift
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LogicalRight;
impl ShiftOperation<ImmediateShift> for LogicalRight {
    fn shift_op(val: u32, shift: u32) -> u32 {
        val >> shift
    }
}

impl ShiftOperation<VromOffsetShift> for LogicalRight {
    fn shift_op(val: u32, shift: u32) -> u32 {
        val >> shift
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArithmeticRight;
impl ShiftOperation<ImmediateShift> for ArithmeticRight {
    fn shift_op(val: u32, shift: u32) -> u32 {
        ((val as i32) >> shift) as u32
    }
}

impl ShiftOperation<VromOffsetShift> for ArithmeticRight {
    fn shift_op(val: u32, shift: u32) -> u32 {
        ((val as i32) >> shift) as u32
    }
}

/// Indicates the source of the shift amount.
pub trait ShiftSource: Debug + Clone + PartialEq {
    fn is_immediate() -> bool;
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImmediateShift(u16);
impl ShiftSource for ImmediateShift {
    fn is_immediate() -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VromOffsetShift((u16, u32));
impl ShiftSource for VromOffsetShift {
    fn is_immediate() -> bool {
        false
    }
}

/// Combined event for both logical and arithmetic shift operations.
/// The type of shift is determined by the `shift_op` field.
#[derive(Debug, Clone, PartialEq)]
pub struct ShiftEvent<S, O>
where
    S: ShiftSource,
    O: ShiftOperation<S>,
{
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub dst: u16,          // 16-bit destination VROM offset
    pub dst_val: u32,      // 32-bit destination value
    pub src: u16,          // 16-bit source VROM offset
    pub src_val: u32,      // 32-bit source value
    pub shift: u16,        // 16-bit shift amount offset
    pub shift_amount: u32, // 32-bit amount to shift source value

    _phantom: PhantomData<(S, O)>,
}

impl<S, O> ShiftEvent<S, O>
where
    S: ShiftSource,
    O: ShiftOperation<S>,
{
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: B32,
        fp: FramePointer,
        timestamp: u32,
        dst: u16,
        dst_val: u32,
        src: u16,
        src_val: u32,
        shift: u16,
        shift_amount: u32,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_val,
            src,
            src_val,
            shift,
            shift_amount,
            _phantom: PhantomData,
        }
    }

    /// Calculate the result of the shift operation.
    ///
    /// The effective shift amount is determined by masking the provided shift
    /// amount to the lower 5 bits (i.e., `shift_amount & 0x1F`). If the
    /// effective shift amount is 0, the original `src_val` is returned.
    /// Otherwise, the shift is performed based on the `shift_op`:
    /// - LogicalLeft: `src_val << effective_shift`
    /// - LogicalRight: `src_val >> effective_shift`
    /// - ArithmeticRight: arithmetic right shift preserving the sign bit.
    pub fn calculate_result(src_val: u32, shift_amount: u32) -> u32 {
        let effective_shift = shift_amount & 0x1f;
        if effective_shift == 0 {
            return src_val;
        }

        O::shift_op(src_val, effective_shift)
    }

    /// Generate a ShiftEvent for immediate shift operations.
    ///
    /// For immediate shifts (like SLLI, SRLI, SRAI), the shift amount comes
    /// directly from the instruction (as a 16-bit immediate) and masked to 5
    /// bits.
    pub(crate) fn generate_immediate_event(
        ctx: &mut EventContext,
        dst: B16,
        src: B16,
        imm: B16,
    ) -> Result<Self, InterpreterError> {
        let src_val = ctx.vrom_read::<u32>(ctx.addr(src.val()))?;
        let imm_val = imm.val();
        let shift_amount = u32::from(imm_val);
        let dst_val = Self::calculate_result(src_val, shift_amount);

        let (_, field_pc, fp, timestamp) = ctx.program_state();

        ctx.vrom_write(ctx.addr(dst.val()), dst_val)?;
        ctx.incr_pc();

        Ok(Self::new(
            field_pc,
            fp,
            timestamp,
            dst.val(),
            dst_val,
            src.val(),
            src_val,
            0, // No shift amount offset for immediate shifts
            shift_amount,
        ))
    }

    /// Generate a ShiftEvent for VROM-based shift operations.
    ///
    /// For VROM-based shifts (like SLL, SRL, SRA), the shift amount is read
    /// from another VROM location and masked to 5 bits.
    pub(crate) fn generate_vrom_event(
        ctx: &mut EventContext,
        dst: B16,
        src1: B16,
        src2: B16,
    ) -> Result<Self, InterpreterError> {
        let src_val = ctx.vrom_read::<u32>(ctx.addr(src1.val()))?;
        let shift_amount = ctx.vrom_read::<u32>(ctx.addr(src2.val()))?;
        let dst_val = Self::calculate_result(src_val, shift_amount);

        let (_, field_pc, fp, timestamp) = ctx.program_state();

        ctx.vrom_write(ctx.addr(dst.val()), dst_val)?;
        ctx.incr_pc();

        Ok(Self::new(
            field_pc,
            fp,
            timestamp,
            dst.val(),
            dst_val,
            src1.val(),
            src_val,
            src2.val(),
            shift_amount,
        ))
    }
}

/// Convenience macro to implement the [`Event`] trait for shift events.
///
/// It takes as argument the field name of the instruction within the
/// [`PetraTrace`](crate::execution::PetraTrace) object, and the corresponding
/// instruction's [`Event`].
///
/// # Example
///
/// ```ignore
/// impl_shift_event!(sll, SllEvent);
/// ```
macro_rules! impl_shift_event {
    ($variant:ident, $ty:ty, $source:ty) => {
        impl Event for $ty {
            fn generate(
                ctx: &mut EventContext,
                dst: B16,
                src1: B16,
                src2: B16,
            ) -> Result<(), InterpreterError> {
                let event = if <$source>::is_immediate() {
                    Self::generate_immediate_event(ctx, dst, src1, src2)?
                } else {
                    Self::generate_vrom_event(ctx, dst, src1, src2)?
                };

                // For right shift operations, create a RightLogicShiftGadgetEvent
                // This needs to handle both logical and arithmetic right shifts
                match stringify!($variant) {
                    "srli" | "srl" => {
                        // For logical right shifts, just use the values directly
                        ctx.trace.add_right_shift_event(
                            event.src_val,
                            event.shift_amount,
                            event.dst_val,
                        );
                    }
                    "srai" | "sra" => {
                        // For arithmetic right shifts, handle sign bit appropriately
                        let sign = (event.src_val >> 31) & 1 == 1;
                        let input = if sign { !event.src_val } else { event.src_val };
                        let output = input >> (event.shift_amount & 0x1F);
                        ctx.trace
                            .add_right_shift_event(input, event.shift_amount, output);
                    }
                    _ => {}
                }

                ctx.trace.$variant.push(event);
                Ok(())
            }

            fn fire(&self, channels: &mut InterpreterChannels) {
                fire_non_jump_event!(self, channels);
            }
        }
    };
}

pub type SlliEvent = ShiftEvent<ImmediateShift, LogicalLeft>;
pub type SrliEvent = ShiftEvent<ImmediateShift, LogicalRight>;
pub type SraiEvent = ShiftEvent<ImmediateShift, ArithmeticRight>;
pub type SllEvent = ShiftEvent<VromOffsetShift, LogicalLeft>;
pub type SrlEvent = ShiftEvent<VromOffsetShift, LogicalRight>;
pub type SraEvent = ShiftEvent<VromOffsetShift, ArithmeticRight>;

impl_shift_event!(slli, SlliEvent, ImmediateShift);
impl_shift_event!(srli, SrliEvent, ImmediateShift);
impl_shift_event!(srai, SraiEvent, ImmediateShift);
impl_shift_event!(sll, SllEvent, VromOffsetShift);
impl_shift_event!(srl, SrlEvent, VromOffsetShift);
impl_shift_event!(sra, SraEvent, VromOffsetShift);

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use binius_field::{Field, PackedField};

    use super::*;
    use crate::{
        isa::GenericISA, memory::Memory, opcodes::Opcode, test_util::code_to_prom, PetraTrace,
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
                ShiftEvent::<ImmediateShift, LogicalLeft>::calculate_result(src_val, shift_amount);
            let result_right =
                ShiftEvent::<ImmediateShift, LogicalRight>::calculate_result(src_val, shift_amount);
            let result_arith = ShiftEvent::<ImmediateShift, ArithmeticRight>::calculate_result(
                src_val,
                shift_amount,
            );

            assert_eq!(
                result_left, expected_left,
                "LogicalLeft failed for {desc}: expected 0x{expected_left:08x}, got 0x{result_left:08x}"
            );
            assert_eq!(
                result_right, expected_right,
                "LogicalRight failed for {desc}: expected 0x{expected_right:08x}, got 0x{result_right:08x}"
            );
            assert_eq!(
                result_arith, expected_arith,
                "ArithmeticRight failed for {desc}: expected 0x{expected_arith:08x}, got 0x{result_arith:08x}"
            );
        }
    }

    #[test]
    fn test_shift_event_integration() {
        let zero = B16::zero();

        // Initialize VROM
        let mut vrom = ValueRom::default();
        vrom.write(0, 0u32).unwrap(); // Return PC
        vrom.write(1, 0u32).unwrap(); // Return FP

        // Create source value slots
        let src_pos = vrom.set_value_at_offset(2, 0x00000003);
        let src_neg = vrom.set_value_at_offset(3, 0x80000000);

        // Create shift amount slots
        let shift_zero = vrom.set_value_at_offset(4, 0);
        let shift_normal = vrom.set_value_at_offset(5, 3);
        let shift_32 = vrom.set_value_at_offset(6, 32);

        // Create destination slots
        let slli_result = B16::new(10);
        let srli_result = B16::new(11);
        let srai_result = B16::new(12);
        let slli_zero_result = B16::new(13);
        let sll_result = B16::new(14);
        let srl_result = B16::new(15);
        let sra_result = B16::new(16);
        let sll_zero_result = B16::new(17);
        let srl_32_result = B16::new(18);
        let sra_32_result = B16::new(19);

        // Build a sequence of instructions
        let instructions = vec![
            // Immediate shift operations with normal shift amount (3)
            [
                Opcode::Slli.get_field_elt(),
                slli_result,
                src_pos,
                B16::new(3),
            ],
            [
                Opcode::Srli.get_field_elt(),
                srli_result,
                src_pos,
                B16::new(3),
            ],
            [
                Opcode::Srai.get_field_elt(),
                srai_result,
                src_neg,
                B16::new(3),
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
        frames.insert(B32::ONE, frame_size);

        let prom = code_to_prom(&instructions);
        let memory = Memory::new(prom, vrom);

        let (trace, _) = PetraTrace::generate(Box::new(GenericISA), memory, frames, HashMap::new())
            .expect("Trace generation should not fail.");

        // Check results for immediate shift operations
        assert_eq!(
            trace.vrom().read::<u32>(slli_result.val() as u32).unwrap(),
            0x00000018,
            "SLLI: 3 << 3 should be 24 (0x00000018)"
        );

        assert_eq!(
            trace.vrom().read::<u32>(srli_result.val() as u32).unwrap(),
            0x00000000,
            "SRLI: 3 >> 3 should be 0"
        );

        assert_eq!(
            trace.vrom().read::<u32>(srai_result.val() as u32).unwrap(),
            0xf0000000,
            "SRAI: 0x80000000 >> 3 (arithmetic) should be 0xF0000000"
        );

        // Check edge case: immediate shift by 0
        assert_eq!(
            trace
                .vrom()
                .read::<u32>(slli_zero_result.val() as u32)
                .unwrap(),
            0x00000003,
            "Shift by 0 should return original value"
        );

        // Check results for VROM-based shift operations
        assert_eq!(
            trace.vrom().read::<u32>(sll_result.val() as u32).unwrap(),
            0x00000018,
            "SLL: 3 << 3 should be 24 (0x00000018)"
        );

        assert_eq!(
            trace.vrom().read::<u32>(srl_result.val() as u32).unwrap(),
            0x00000000,
            "SRL: 3 >> 3 should be 0"
        );

        assert_eq!(
            trace.vrom().read::<u32>(sra_result.val() as u32).unwrap(),
            0xf0000000,
            "SRA: 0x80000000 >> 3 (arithmetic) should be 0xF0000000"
        );

        // Check VROM-based edge cases (modular behavior):
        // A shift by 32 is equivalent to a shift by 0.
        assert_eq!(
            trace
                .vrom()
                .read::<u32>(sll_zero_result.val() as u32)
                .unwrap(),
            0x00000003,
            "VROM-based shift by 0 should return original value"
        );

        // For shift amount 32, effective shift = 0, so original value is returned.
        assert_eq!(
            trace
                .vrom()
                .read::<u32>(srl_32_result.val() as u32)
                .unwrap(),
            0x00000003,
            "SRL by 32 should return original value (mod 32 behavior)"
        );

        assert_eq!(
            trace
                .vrom()
                .read::<u32>(sra_32_result.val() as u32)
                .unwrap(),
            0x80000000,
            "SRA by 32 on negative value should return original value (mod 32 behavior)"
        );
    }
}
