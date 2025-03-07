use binius_field::{BinaryField16b, BinaryField32b};

use super::{BinaryOperation, Event};
use crate::{
    fire_non_jump_event, impl_32b_immediate_binary_operation, impl_binary_operation,
    impl_event_for_binary_operation, impl_immediate_binary_operation, G,
};

/// Event for XORI.
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

/// Event for ANDI.
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

impl_binary_operation!(XorEvent);
impl_event_for_binary_operation!(XorEvent);

impl BinaryOperation for XorEvent {
    fn operation(val: BinaryField32b, imm: BinaryField32b) -> BinaryField32b {
        val + imm
    }
}

impl BinaryOperation for AndiEvent {
    #[inline(always)]
    fn operation(val: BinaryField32b, imm: BinaryField16b) -> BinaryField32b {
        // TODO: can't we simplify it like for Xori?
        BinaryField32b::new(val.val() & imm.val() as u32)
    }
}

impl_immediate_binary_operation!(AndiEvent);
impl_event_for_binary_operation!(AndiEvent);

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
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField32b,
    ) -> Self {
        let src_val = interpreter.vrom.get_u32(interpreter.fp ^ src.val() as u32);
        let dst_val = Self::operation(BinaryField32b::new(src_val), imm);
        let event = Self::new(
            interpreter.timestamp,
            interpreter.pc,
            interpreter.fp,
            dst.val(),
            dst_val.val(),
            src.val(),
            src_val,
            imm.val(),
        );
        interpreter
            .vrom
            .set_u32(interpreter.fp ^ dst.val() as u32, dst_val.val());
        // The instruction is over two rows in the PROM.
        interpreter.incr_pc();
        interpreter.incr_pc();
        event
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
        tables: &crate::emulator::InterpreterTables,
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
