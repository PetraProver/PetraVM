use core::time;

use crate::emulator::{Interpreter, StateChannel};

use super::{BinaryOperation, Event, ImmediateBinaryOperation};

#[derive(Debug, Default, Clone)]
pub(crate) struct XorIEvent {
    timestamp: u16,
    pc: u16,
    fp: u16,
    dst: u16,
    dst_val: u32,
    src: u16,
    src_val: u32,
    imm: u32,
}

impl Event for XorIEvent {
    fn fire(&self, prom_chan: &mut StateChannel) {
        prom_chan.push((self.pc, self.fp, self.timestamp));
    }
}

impl ImmediateBinaryOperation for XorIEvent {

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

impl BinaryOperation for XorIEvent {
    fn operation(val: u32, imm: u32) -> u32 {
        val ^ imm
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct AndIEvent {
    timestamp: u16,
    pc: u16,
    fp: u16,
    dst: u16,
    dst_val: u32,
    src: u16,
    src_val: u32,
    imm: u32,
}

impl Event for AndIEvent {
    fn fire(&self, prom_chan: &mut StateChannel) {
        unimplemented!()
    }
}

impl ImmediateBinaryOperation for AndIEvent {
    
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

impl BinaryOperation for AndIEvent {
    fn operation(val: u32, imm: u32) -> u32 {
        val & imm
    }
}
