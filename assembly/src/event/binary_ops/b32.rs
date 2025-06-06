use binius_field::{ExtensionField, Field};
use binius_m3::builder::{B16, B32};

use super::BinaryOperation;
use crate::macros::{
    define_bin32_imm_op_event, define_bin32_op_event, impl_32b_immediate_binary_operation,
};
use crate::{
    event::{binary_ops::*, context::EventContext, Event},
    execution::{InterpreterError, G},
    Opcode,
};

define_bin32_op_event!(
    /// Event for XOR.
    ///
    /// Performs a XOR between two target addresses.
    ///
    /// Logic:
    ///   1. FP[dst] = __b32_xor(FP[src1], FP[src2])
    XorEvent,
    xor,
    |a, b| a + b
);

define_bin32_imm_op_event!(
    /// Event for XORI.
    ///
    /// Performs a XOR between a target address and an immediate.
    ///
    /// Logic:
    ///   1. FP[dst] = __b32_xor(FP[src], imm)
    XoriEvent,
    xori,
    |a, b| a + b
);

define_bin32_op_event!(
    /// Event for AND.
    ///
    /// Performs an AND between two target addresses.
    ///
    /// Logic:
    ///   1. FP[dst] = __b32_and(FP[src], FP[src2])
    AndEvent,
    and,
    |a: B32, b: B32| B32::new(a.val() & b.val())
);

define_bin32_imm_op_event!(
    /// Event for ANDI.
    ///
    /// Performs an AND between a target address and an immediate.
    ///
    /// Logic:
    ///   1. FP[dst] = __b32_and(FP[src], imm)
    AndiEvent,
    andi,
    |a: B32, imm: B16| B32::new(a.val() & imm.val() as u32)
);

define_bin32_op_event!(
    /// Event for OR.
    ///
    /// Performs an OR between two target addresses.
    ///
    /// Logic:
    ///   1. FP[dst] = __b32_or(FP[src], FP[src2])
    OrEvent,
    or,
    |a: B32, b: B32| B32::new(a.val() | b.val())
);

define_bin32_imm_op_event!(
    /// Event for ORI.
    ///
    /// Performs an OR between a target address and an immediate.
    ///
    /// Logic:
    ///   1. FP[dst] = __b32_or(FP[src], imm)
    OriEvent,
    ori,
    |a: B32, imm: B16| B32::new(a.val() | imm.val() as u32)
);

define_bin32_op_event!(
    /// Event for B32_MUL.
    ///
    /// Performs a 32-bit MUL between two target addresses.
    ///
    /// Logic:
    ///   1. FP[dst] = __b32_mul(FP[src1], FP[src2])
    B32MulEvent,
    b32_mul,
    |a, b| a * b
);

/// Event for B32_MULI.
///
/// Performs a 32-bit MUL between a target address and an immediate.
///
/// Logic:
///   1. FP[dst] = __b32_mul(FP[src], imm)
#[derive(Debug, Default, Clone)]
pub struct B32MuliEvent {
    pub timestamp: u32,
    pub pc: B32,
    pub fp: FramePointer,
    pub dst: u16,
    pub dst_val: u32,
    pub src: u16,
    pub src_val: u32,
    pub imm: u32,
}

impl BinaryOperation for B32MuliEvent {
    #[inline(always)]
    fn operation(val: B32, imm: B32) -> B32 {
        val * imm
    }
}

impl Event for B32MuliEvent {
    fn generate(
        ctx: &mut EventContext,
        dst: B16,
        src: B16,
        imm_low: B16,
    ) -> Result<(), InterpreterError> {
        // B32_MULI spans over two rows in the PROM
        let [second_opcode, imm_high, third, fourth] =
            ctx.trace.prom()[ctx.prom_index as usize + 1].instruction;

        if second_opcode.val() != Opcode::B32Muli as u16
            || third != B16::ZERO
            || fourth != B16::ZERO
        {
            return Err(InterpreterError::InvalidInput);
        }
        let imm =
            B32::from_bases([imm_low, imm_high]).map_err(|_| InterpreterError::InvalidInput)?;

        let src_val = ctx.vrom_read::<u32>(ctx.addr(src.val()))?;
        let dst_val = Self::operation(B32::new(src_val), imm);
        ctx.vrom_write(ctx.addr(dst.val()), dst_val.val())?;

        if !ctx.prover_only {
            let (_, field_pc, fp, timestamp) = ctx.program_state();

            let event = Self::new(
                timestamp,
                field_pc,
                fp,
                dst.val(),
                dst_val.val(),
                src.val(),
                src_val,
                imm.val(),
            );

            ctx.trace.b32_muli.push(event);
        }
        // The instruction is over two rows in the PROM.
        ctx.incr_counters();
        ctx.incr_counters();
        Ok(())
    }

    fn fire(&self, channels: &mut crate::execution::InterpreterChannels) {
        assert_eq!(
            self.dst_val,
            Self::operation(B32::new(self.src_val), self.imm.into()).val()
        );

        channels
            .state_channel
            .pull((self.pc, *self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.pc * G * G, *self.fp, self.timestamp));
    }
}

impl_32b_immediate_binary_operation!(B32MuliEvent);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logical_operations() {
        let a = B32::new(0b1111_1010_0000);
        let b = B32::new(0b1010_1111_0011);

        let a_or_b = B32::new(0b1111_1111_0011);
        let a_xor_b = B32::new(0b0101_0101_0011);
        let a_and_b = B32::new(0b1010_1010_0000);

        assert_eq!(OrEvent::operation(a, b), a_or_b);
        assert_eq!(XorEvent::operation(a, b), a_xor_b);
        assert_eq!(AndEvent::operation(a, b), a_and_b);
    }
}
