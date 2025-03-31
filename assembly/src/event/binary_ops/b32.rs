use binius_field::{BinaryField16b, BinaryField32b, ExtensionField, Field, PackedField};

use super::BinaryOperation;
use crate::{
    define_bin32_imm_op_event, define_bin32_op_event,
    event::{binary_ops::*, context::EventContext, Event},
    execution::{InterpreterError, ZCrayTrace, G},
    impl_32b_immediate_binary_operation, Opcode,
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
    |a: BinaryField32b, b: BinaryField32b| BinaryField32b::new(a.val() & b.val())
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
    |a: BinaryField32b, imm: BinaryField16b| BinaryField32b::new(a.val() & imm.val() as u32)
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
    |a: BinaryField32b, b: BinaryField32b| BinaryField32b::new(a.val() | b.val())
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
    |a: BinaryField32b, imm: BinaryField16b| BinaryField32b::new(a.val() | imm.val() as u32)
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
    timestamp: u32,
    pc: BinaryField32b,
    fp: FramePointer,
    dst: u16,
    dst_val: u32,
    src: u16,
    src_val: u32,
    imm: u32,
}

impl BinaryOperation for B32MuliEvent {
    #[inline(always)]
    fn operation(val: BinaryField32b, imm: BinaryField32b) -> BinaryField32b {
        val * imm
    }
}

impl Event for B32MuliEvent {
    fn generate(
        ctx: &mut EventContext,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm_low: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        // B32_MULI spans over two rows in the PROM
        let [second_opcode, imm_high, third, fourth] =
            ctx.trace.prom()[ctx.pc as usize].instruction;

        if second_opcode.val() != Opcode::B32Muli.into()
            || third != BinaryField16b::ZERO
            || fourth != BinaryField16b::ZERO
        {
            return Err(InterpreterError::BadPc);
        }
        let imm = BinaryField32b::from_bases([imm_low, imm_high])
            .map_err(|_| InterpreterError::InvalidInput)?;

        let src_val = ctx.load_vrom_u32(ctx.addr(src.val()))?;
        let dst_val = Self::operation(BinaryField32b::new(src_val), imm);
        debug_assert!(ctx.field_pc == G.pow(ctx.pc as u64 - 1));
        let event = Self::new(
            ctx.timestamp,
            ctx.field_pc,
            ctx.fp,
            dst.val(),
            dst_val.val(),
            src.val(),
            src_val,
            imm.val(),
        );
        ctx.store_vrom_u32(ctx.addr(dst.val()), dst_val.val())?;
        // The instruction is over two rows in the PROM.
        ctx.incr_pc();
        ctx.incr_pc();

        ctx.trace.b32_muli.push(event);
        Ok(())
    }

    fn fire(
        &self,
        channels: &mut crate::execution::InterpreterChannels,
        _tables: &crate::execution::InterpreterTables,
    ) {
        assert_eq!(
            self.dst_val,
            Self::operation(BinaryField32b::new(self.src_val), self.imm.into()).into()
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
        let a = BinaryField32b::new(0b1111_1010_0000);
        let b = BinaryField32b::new(0b1010_1111_0011);

        let a_or_b = BinaryField32b::new(0b1111_1111_0011);
        let a_xor_b = BinaryField32b::new(0b0101_0101_0011);
        let a_and_b = BinaryField32b::new(0b1010_1010_0000);

        assert_eq!(OrEvent::operation(a, b), a_or_b);
        assert_eq!(XorEvent::operation(a, b), a_xor_b);
        assert_eq!(AndEvent::operation(a, b), a_and_b);
    }
}
