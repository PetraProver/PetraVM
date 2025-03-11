use binius_field::{BinaryField16b, BinaryField32b, Field, PackedField};

use super::{BinaryOperation, Event};
use crate::{
    emulator::InterpreterError, fire_non_jump_event, impl_32b_immediate_binary_operation,
    impl_binary_operation, impl_event_for_binary_operation, impl_immediate_binary_operation,
    ZCrayTrace, G,
};

/// Event for XOR.
///
/// Performs a XOR between two target addresses.
///
/// Logic:
///   1. FP[dst] = __b32_xor(FP[src1], FP[src2])
#[derive(Debug, Default, Clone)]
pub(crate) struct XorEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    dst: u16,
    dst_val: u32,
    src1: u16,
    src1_val: u32,
    src2: u16,
    src2_val: u32,
}

impl BinaryOperation for XorEvent {
    fn operation(val1: BinaryField32b, val2: BinaryField32b) -> BinaryField32b {
        val1 + val2
    }
}

impl_binary_operation!(XorEvent);
impl_event_for_binary_operation!(XorEvent);

/// Event for XORI.
///
/// Performs a XOR between a target address and an immediate.
///
/// Logic:
///   1. FP[dst] = __b32_xor(FP[src], imm)
#[derive(Debug, Default, Clone)]
pub(crate) struct XoriEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    dst: u16,
    dst_val: u32,
    src: u16,
    src_val: u32,
    imm: u16,
}

impl BinaryOperation for XoriEvent {
    #[inline(always)]
    fn operation(val: BinaryField32b, imm: BinaryField16b) -> BinaryField32b {
        val + imm
    }
}

impl_immediate_binary_operation!(XoriEvent);
impl_event_for_binary_operation!(XoriEvent);

/// Event for AND.
///
/// Performs an AND between two target addresses.
///
/// Logic:
///   1. FP[dst] = __b32_and(FP[src], FP[src2])
#[derive(Debug, Default, Clone)]
pub(crate) struct AndEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    dst: u16,
    dst_val: u32,
    src1: u16,
    src1_val: u32,
    src2: u16,
    src2_val: u32,
}

impl BinaryOperation for AndEvent {
    #[inline(always)]
    fn operation(val1: BinaryField32b, val2: BinaryField32b) -> BinaryField32b {
        BinaryField32b::new(val1.val() & val2.val())
    }
}

impl_binary_operation!(AndEvent);
impl_event_for_binary_operation!(AndEvent);

/// Event for ANDI.
///
/// Performs an AND between a target address and an immediate.
///
/// Logic:
///   1. FP[dst] = __b32_and(FP[src], imm)
#[derive(Debug, Default, Clone)]
pub(crate) struct AndiEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    dst: u16,
    dst_val: u32,
    src: u16,
    src_val: u32,
    imm: u16,
}

impl BinaryOperation for AndiEvent {
    #[inline(always)]
    fn operation(val: BinaryField32b, imm: BinaryField16b) -> BinaryField32b {
        BinaryField32b::new(val.val() & imm.val() as u32)
    }
}

impl_immediate_binary_operation!(AndiEvent);
impl_event_for_binary_operation!(AndiEvent);

/// Event for OR.
///
/// Performs an OR between two target addresses.
///
/// Logic:
///   1. FP[dst] = __b32_or(FP[src], FP[src2])
#[derive(Debug, Default, Clone)]
pub(crate) struct OrEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    dst: u16,
    dst_val: u32,
    src1: u16,
    src1_val: u32,
    src2: u16,
    src2_val: u32,
}

impl BinaryOperation for OrEvent {
    fn operation(val1: BinaryField32b, val2: BinaryField32b) -> BinaryField32b {
        BinaryField32b::new(val1.val() | val2.val())
    }
}

impl_binary_operation!(OrEvent);
impl_event_for_binary_operation!(OrEvent);

/// Event for ORI.
///
/// Performs an OR between a target address and an immediate.
///
/// Logic:
///   1. FP[dst] = __b32_or(FP[src], imm)
#[derive(Debug, Default, Clone)]
pub(crate) struct OriEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    dst: u16,
    dst_val: u32,
    src: u16,
    src_val: u32,
    imm: u16,
}

impl BinaryOperation for OriEvent {
    fn operation(val: BinaryField32b, imm: BinaryField16b) -> BinaryField32b {
        BinaryField32b::new(val.val() | imm.val() as u32)
    }
}

impl_immediate_binary_operation!(OriEvent);
impl_event_for_binary_operation!(OriEvent);

/// Event for B32_MUL.
///
/// Performs a 32-bit MUL between two target addresses.
///
/// Logic:
///   1. FP[dst] = __b32_mul(FP[src1], FP[src2])
#[derive(Debug, Default, Clone)]
pub(crate) struct B32MulEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    dst: u16,
    dst_val: u32,
    src1: u16,
    src1_val: u32,
    src2: u16,
    src2_val: u32,
}

impl BinaryOperation for B32MulEvent {
    #[inline(always)]
    fn operation(val1: BinaryField32b, val2: BinaryField32b) -> BinaryField32b {
        val1 * val2
    }
}

impl_binary_operation!(B32MulEvent);
impl_event_for_binary_operation!(B32MulEvent);

/// Event for B32_MULI.
///
/// Performs a 32-bit MUL between a target address and an immediate.
///
/// Logic:
///   1. FP[dst] = __b32_mul(FP[src], imm)
#[derive(Debug, Default, Clone)]
pub(crate) struct B32MuliEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    dst: u16,
    dst_val: u32,
    src: u16,
    src_val: u32,
    imm: u32,
}

impl B32MuliEvent {
    pub fn generate_event(
        interpreter: &mut crate::emulator::Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField32b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let src_val = interpreter
            .vrom
            .get_u32(interpreter.fp ^ src.val() as u32)?;
        let dst_val = Self::operation(BinaryField32b::new(src_val), imm);
        debug_assert!(field_pc == G.pow(interpreter.pc as u64 - 1));
        let event = Self::new(
            interpreter.timestamp,
            field_pc,
            interpreter.fp,
            dst.val(),
            dst_val.val(),
            src.val(),
            src_val,
            imm.val(),
        );
        interpreter
            .vrom
            .set_u32(trace, interpreter.fp ^ dst.val() as u32, dst_val.val())?;
        // The instruction is over two rows in the PROM.
        interpreter.incr_pc();
        interpreter.incr_pc();
        Ok(event)
    }
}

impl BinaryOperation for B32MuliEvent {
    #[inline(always)]
    fn operation(val: BinaryField32b, imm: BinaryField32b) -> BinaryField32b {
        val * imm
    }
}

impl Event for B32MuliEvent {
    fn fire(
        &self,
        channels: &mut crate::emulator::InterpreterChannels,
        _tables: &crate::emulator::InterpreterTables,
    ) {
        assert_eq!(
            self.dst_val,
            Self::operation(BinaryField32b::new(self.src_val), self.imm.into()).into()
        );

        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.pc * G * G, self.fp, self.timestamp + 1));
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
