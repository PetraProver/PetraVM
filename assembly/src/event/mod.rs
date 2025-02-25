pub(crate) mod b32;
pub(crate) mod branch;
pub(crate) mod ret;
pub(crate) mod sli;
pub(crate) mod call;

use crate::emulator::StateChannel;

pub trait Event {
    fn fire(&self, state_channel: &mut StateChannel);
}

pub(crate) trait BinaryOperation: Sized {
    fn operation(val: u32, imm: u32) -> u32;
}

// TODO: Add type paraeter for operation over other fields?
pub(crate) trait ImmediateBinaryOperation: BinaryOperation {
    // TODO: Add some trick to implement new only once
    fn new(
        timestamp: u16,
        pc: u16,
        fp: u16,
        dst: u16,
        dst_val: u32,
        src: u16,
        src_val: u32,
        imm: u32,
    ) -> Self;

    fn generate_event(
        interpreter: &mut crate::emulator::Interpreter,
        dst: u16,
        src: u16,
        imm: u32,
    ) -> Self {
        let src_val = interpreter.vrom[interpreter.fp as usize + src as usize];
        let dst_val = Self::operation(src_val, imm);
        let event = Self::new(
            interpreter.timestamp,
            interpreter.pc,
            interpreter.fp,
            dst,
            dst_val,
            src,
            src_val,
            imm,
        );
        interpreter.vrom[interpreter.fp as usize + dst as usize] = dst_val;
        interpreter.pc += 1;
        interpreter.timestamp += 1;
        event
    }
}
