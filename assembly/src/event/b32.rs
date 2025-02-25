use core::time;

use crate::emulator::{Interpreter, InterpreterChannels, InterpreterTables};

use super::{BinaryOperation, Event, ImmediateBinaryOperation};

#[derive(Debug, Default, Clone)]
pub(crate) struct XoriEvent {
    timestamp: u16,
    pc: u16,
    fp: u16,
    dst: u16,
    dst_val: u32,
    src: u16,
    src_val: u32,
    imm: u32,
}

impl Event for XoriEvent {
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables) {
        channels
            .state_channel
            .push((self.pc, self.fp, self.timestamp));
    }
}

impl ImmediateBinaryOperation for XoriEvent {
    fn new(
        timestamp: u16,
        pc: u16,
        fp: u16,
        dst: u16,
        dst_val: u32,
        src: u16,
        src_val: u32,
        imm: u32,
    ) -> Self {
        Self {
            timestamp,
            pc,
            fp,
            dst,
            dst_val,
            src,
            src_val,
            imm,
        }
    }
}

impl BinaryOperation for XoriEvent {
    fn operation(val: u32, imm: u32) -> u32 {
        val ^ imm
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct AndiEvent {
    timestamp: u16,
    pc: u16,
    fp: u16,
    dst: u16,
    dst_val: u32,
    src: u16,
    src_val: u32,
    imm: u32,
}

impl Event for AndiEvent {
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables) {
        unimplemented!()
    }
}

impl ImmediateBinaryOperation for AndiEvent {
    fn new(
        timestamp: u16,
        pc: u16,
        fp: u16,
        dst: u16,
        dst_val: u32,
        src: u16,
        src_val: u32,
        imm: u32,
    ) -> Self {
        Self {
            timestamp,
            pc,
            fp,
            dst,
            dst_val,
            src,
            src_val,
            imm,
        }
    }
}

impl BinaryOperation for AndiEvent {
    fn operation(val: u32, imm: u32) -> u32 {
        val & imm
    }
}
