use binius_m3::builder::{B16, B32};

use super::context::EventContext;
use crate::{
    define_bin32_imm_op_event, define_bin32_op_event,
    event::binary_ops::*,
    execution::{FramePointer, InterpreterError},
};

// Note: The addition is checked thanks to the ADD32 table.
define_bin32_op_event!(
    /// Event for SLTU.
    ///
    /// Performs an SLTU between two target addresses.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src1] < FP[src2]
    SltuEvent,
    sltu,
    // LT is checked using a SUB gadget.
    |a: B32, b: B32| B32::new((a.val() < b.val()) as u32)
);

define_bin32_imm_op_event!(
    /// Event for SLTIU.
    ///
    /// Performs an SLTIU between an unsigned target address and immediate.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src1] < FP[src2]
    SltiuEvent,
    sltiu,
    // LT is checked using a SUB gadget.
    |a: B32, imm: B16| B32::new((a.val() < imm.val() as u32) as u32)
);

// Note: The addition is checked thanks to the ADD32 table.
define_bin32_op_event!(
    /// Event for SLT.
    ///
    /// Performs an SLT between two signed target addresses.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src1] < FP[src2]
    SltEvent,
    slt,
    // LT is checked using a SUB gadget.
    |a: B32, b: B32| B32::new(((a.val() as i32) < (b.val() as i32)) as u32)
);

define_bin32_imm_op_event!(
    /// Event for SLTI.
    ///
    /// Performs an SLTI between a target address and an immediate.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src1] < imm
    SltiEvent,
    slti,
    // LT is checked using a SUB gadget.
    |a: B32, imm: B16| B32::new(((a.val() as i32) < (imm.val() as i16 as i32)) as u32)
);

// Note: The addition is checked thanks to the ADD32 table.
define_bin32_op_event!(
    /// Event for SLE.
    ///
    /// Performs an SLE between two signed target addresses.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src1] <= FP[src2]
    SleEvent,
    sle,
    // LT is checked using a SUB gadget.
    |a: B32, b: B32| B32::new(((a.val() as i32) <= (b.val() as i32)) as u32)
);

define_bin32_imm_op_event!(
    /// Event for SLEI.
    ///
    /// Performs an SLEI between a target address and an immediate.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src1] <= imm
    SleiEvent,
    slei,
    // LT is checked using a SUB gadget.
    |a: B32, imm: B16| B32::new(((a.val() as i32) <= (imm.val() as i16 as i32)) as u32)
);

// Note: The addition is checked thanks to the ADD32 table.
define_bin32_op_event!(
    /// Event for SLEU.
    ///
    /// Performs an SLEU between two signed target addresses.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src1] <= FP[src2]
    SleuEvent,
    sleu,
    // LT is checked using a SUB gadget.
    |a: B32, b: B32| B32::new((a.val() <= b.val()) as u32)
);

define_bin32_imm_op_event!(
    /// Event for SLEUI.
    ///
    /// Performs an SLEUI between a target address and an immediate.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src1] <= imm
    SleiuEvent,
    sleiu,
    // LT is checked using a SUB gadget.
    |a: B32, imm: B16| B32::new((a.val() <= imm.val() as u32) as u32)
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{execution::Interpreter, get_last_event, Event, ZCrayTrace};

    /// Tests for Comparison operations (without immediate)
    #[test]
    fn test_comparison_operations() {
        // Test cases for SLT, SLTU, SLE, SLEU
        let test_cases = [
            // (src1_val, src2_val, slt_expected, sltu_expected, sle_expected, sleu_expected,
            // description)
            (5, 10, 1, 1, 1, 1, "simple less than"),
            (10, 5, 0, 0, 0, 0, "simple greater than"),
            (5, 5, 0, 0, 1, 1, "equal values"),
            (0, 0, 0, 0, 1, 1, "zero comparison"),
            (0xFFFFFFFF, 5, 1, 0, 1, 0, "signed -1 < 5, unsigned MAX > 5"),
            (5, 0xFFFFFFFF, 0, 1, 0, 1, "signed 5 > -1, unsigned 5 < MAX"),
            (
                0x80000000,
                0x7FFFFFFF,
                1,
                0,
                1,
                0,
                "signed MIN < MAX, unsigned MIN > MAX",
            ),
            (
                0x7FFFFFFF,
                0x80000000,
                0,
                1,
                0,
                1,
                "signed MAX > MIN, unsigned MAX < MIN",
            ),
            (
                0x80000000,
                0x80000000,
                0,
                0,
                1,
                1,
                "0x80000000 vs 0x80000000 (equal values)",
            ),
            (
                0,
                0x80000000,
                0,
                1,
                0,
                1,
                "signed 0 > MIN, unsigned 0 < MIN",
            ),
            (
                0x80000000,
                0,
                1,
                0,
                1,
                0,
                "signed MIN < 0, unsigned MIN > 0",
            ),
        ];

        for (src1_val, src2_val, slt_expected, sltu_expected, sle_expected, sleu_expected, desc) in
            test_cases
        {
            // Test SLT (Signed Less Than)
            let mut interpreter = Interpreter::default();
            let mut trace = ZCrayTrace::default();
            let mut ctx = EventContext::new(&mut interpreter, &mut trace);
            let src1_offset = B16::new(2);
            let src2_offset = B16::new(3);
            let dst_offset = B16::new(4);

            // Set values in VROM at the computed addresses (FP ^ offset)
            ctx.set_vrom(src1_offset.val(), src1_val);
            ctx.set_vrom(src2_offset.val(), src2_val);

            SltEvent::generate(&mut ctx, dst_offset, src1_offset, src2_offset).unwrap();
            let event = get_last_event!(ctx, slt);

            assert_eq!(
                event.dst_val, slt_expected,
                "SLT failed for {}: expected {} got {} (src1=0x{:x}, src2=0x{:x})",
                desc, slt_expected, event.dst_val, src1_val, src2_val
            );

            // Test SLTU (Unsigned Less Than)
            let mut interpreter = Interpreter::default();
            let mut trace = ZCrayTrace::default();
            let mut ctx = EventContext::new(&mut interpreter, &mut trace);
            // Set values in VROM at the computed addresses (FP ^ offset)
            ctx.set_vrom(src1_offset.val(), src1_val);
            ctx.set_vrom(src2_offset.val(), src2_val);

            SltuEvent::generate(&mut ctx, dst_offset, src1_offset, src2_offset).unwrap();
            let event = get_last_event!(ctx, sltu);

            assert_eq!(
                event.dst_val, sltu_expected,
                "SLTU failed for {}: expected {} got {} (src1=0x{:x}, src2=0x{:x})",
                desc, sltu_expected, event.dst_val, src1_val, src2_val
            );

            // Test SLE (Signed Less Than Or Equal)
            let mut interpreter = Interpreter::default();
            let mut trace = ZCrayTrace::default();
            let mut ctx = EventContext::new(&mut interpreter, &mut trace);
            // Set values in VROM at the computed addresses (FP ^ offset)
            ctx.set_vrom(src1_offset.val(), src1_val);
            ctx.set_vrom(src2_offset.val(), src2_val);

            SleEvent::generate(&mut ctx, dst_offset, src1_offset, src2_offset).unwrap();
            let event = get_last_event!(ctx, sle);

            assert_eq!(
                event.dst_val, sle_expected,
                "SLE failed for {}: expected {} got {} (src1=0x{:x}, src2=0x{:x})",
                desc, sle_expected, event.dst_val, src1_val, src2_val
            );

            // Test SLEU (Unsigned Less Than Or Equal)
            let mut interpreter = Interpreter::default();
            let mut trace = ZCrayTrace::default();
            let mut ctx = EventContext::new(&mut interpreter, &mut trace);
            // Set values in VROM at the computed addresses (FP ^ offset)
            ctx.set_vrom(src1_offset.val(), src1_val);
            ctx.set_vrom(src2_offset.val(), src2_val);

            SleuEvent::generate(&mut ctx, dst_offset, src1_offset, src2_offset).unwrap();
            let event = get_last_event!(ctx, sleu);

            assert_eq!(
                event.dst_val, sleu_expected,
                "SLEU failed for {}: expected {} got {} (src1=0x{:x}, src2=0x{:x})",
                desc, sleu_expected, event.dst_val, src1_val, src2_val
            );
        }
    }

    /// Tests for Comparison operations (with immediate)
    #[test]
    fn test_comparison_immediate_operations() {
        // Test cases for SLTI, SLTIU
        let test_cases = [
            // (src_val, imm_val, slti_expected, sltiu_expected, slei_expected, sleiu_expected,
            // description)
            (5, 10, 1, 1, 1, 1, "simple less than"),
            (10, 5, 0, 0, 0, 0, "simple greater than"),
            (5, 5, 0, 0, 1, 1, "equal values"),
            (0, 0, 0, 0, 1, 1, "zero comparison"),
            // Tests with max positive and min negative 16-bit immediates
            (
                0x7FFF,
                0x7FFF,
                0,
                0,
                1,
                1,
                "equal to max positive immediate",
            ),
            (
                0x7FFE,
                0x7FFF,
                1,
                1,
                1,
                1,
                "just below max positive immediate",
            ),
            (
                0x8000,
                0x7FFF,
                0,
                0,
                0,
                0,
                "just above max positive immediate",
            ),
            (
                0x0000,
                0x7FFF,
                1,
                1,
                1,
                1,
                "0 vs 0x7FFF (max positive immediate)",
            ),
            // Sign extension tests for immediates
            (
                0x0000,
                0x8000,
                0,
                1,
                0,
                1,
                "0 vs 0x8000 (sign extended to -32768)",
            ),
            (
                0x7FFF,
                0x8000,
                0,
                1,
                0,
                1,
                "0x7FFF vs 0x8000 (sign extended to -32768)",
            ),
            (
                0x8000,
                0x8000,
                0,
                0,
                0,
                1,
                "0x8000 vs 0x8000 (equal values)",
            ),
            (
                0x8001,
                0x8000,
                0,
                0,
                0,
                0,
                "0x8001 vs 0x8000 (both negative, first more negative)",
            ),
            // Tests with 0xFFFF (-1 signed, max unsigned)
            (
                0x0000,
                0xFFFF,
                0,
                1,
                0,
                1,
                "0 vs 0xFFFF (sign extended to -1)",
            ),
            (
                0xFFFE,
                0xFFFF,
                0,
                1,
                0,
                1,
                "0xFFFE vs 0xFFFF (test just below)",
            ),
            (
                0xFFFF,
                0xFFFF,
                0,
                0,
                0,
                1,
                "0xFFFF vs 0xFFFF (equal values)",
            ),
            (
                0x10000,
                0xFFFF,
                0,
                0,
                0,
                0,
                "0x10000 vs 0xFFFF (test just above)",
            ),
            (
                0xFFFFFFFF,
                0xFFFF,
                0,
                0,
                1,
                0,
                "-1 vs -1 (both sign extended to -1)",
            ),
            // Additional tests with signed vs unsigned interpretation
            (
                0xFFFFFFFF,
                0x0005,
                1,
                0,
                1,
                0,
                "-1 vs 5 (signed vs unsigned)",
            ),
            (
                0x00000005,
                0xFFFF,
                0,
                1,
                0,
                1,
                "5 vs -1 (signed vs unsigned)",
            ),
        ];

        for (
            src_val,
            imm_val,
            slti_expected,
            sltiu_expected,
            slei_expected,
            sleiu_expected,
            desc,
        ) in test_cases
        {
            // Test SLTI (Signed Less Than Immediate)
            let mut interpreter = Interpreter::default();
            let mut trace = ZCrayTrace::default();
            let mut ctx = EventContext::new(&mut interpreter, &mut trace);
            let src_offset = B16::new(2);
            let dst_offset = B16::new(4);
            let imm = B16::new(imm_val);

            // Set value in VROM at the computed address (FP ^ offset)
            ctx.set_vrom(src_offset.val(), src_val);

            SltiEvent::generate(&mut ctx, dst_offset, src_offset, imm).unwrap();
            let event = get_last_event!(ctx, slti);

            assert_eq!(
                event.dst_val, slti_expected,
                "SLTI failed for {}: expected {} got {} (src=0x{:x}, imm=0x{:x})",
                desc, slti_expected, event.dst_val, src_val, imm_val
            );

            // Test SLTIU (Unsigned Less Than Immediate)
            let mut interpreter = Interpreter::default();
            let mut trace = ZCrayTrace::default();
            let mut ctx = EventContext::new(&mut interpreter, &mut trace);
            // Set value in VROM at the computed address (FP ^ offset)
            ctx.set_vrom(src_offset.val(), src_val);

            SltiuEvent::generate(&mut ctx, dst_offset, src_offset, imm).unwrap();
            let event = get_last_event!(ctx, sltiu);

            assert_eq!(
                event.dst_val, sltiu_expected,
                "SLTIU failed for {}: expected {} got {} (src=0x{:x}, imm=0x{:x})",
                desc, sltiu_expected, event.dst_val, src_val, imm_val
            );

            // Test SLEI (Signed Less Than Or Equal Immediate)
            let mut interpreter = Interpreter::default();
            let mut trace = ZCrayTrace::default();
            let mut ctx = EventContext::new(&mut interpreter, &mut trace);
            // Set value in VROM at the computed address (FP ^ offset)
            ctx.set_vrom(src_offset.val(), src_val);

            SleiEvent::generate(&mut ctx, dst_offset, src_offset, imm).unwrap();
            let event = get_last_event!(ctx, slei);

            assert_eq!(
                event.dst_val, slei_expected,
                "SLEI failed for {}: expected {} got {} (src=0x{:x}, imm=0x{:x})",
                desc, slei_expected, event.dst_val, src_val, imm_val
            );

            // Test SLEUI (Unsigned Less Than Or Equal Immediate)
            let mut interpreter = Interpreter::default();
            let mut trace = ZCrayTrace::default();
            let mut ctx = EventContext::new(&mut interpreter, &mut trace);
            // Set value in VROM at the computed address (FP ^ offset)
            ctx.set_vrom(src_offset.val(), src_val);

            SleiuEvent::generate(&mut ctx, dst_offset, src_offset, imm).unwrap();
            let event = get_last_event!(ctx, sleiu);

            assert_eq!(
                event.dst_val, sleiu_expected,
                "SLEUI failed for {}: expected {} got {} (src=0x{:x}, imm=0x{:x})",
                desc, sleiu_expected, event.dst_val, src_val, imm_val
            );
        }
    }
}
