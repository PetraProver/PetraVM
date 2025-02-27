use std::cmp::max;

use crate::{
    emulator::{Interpreter, InterpreterChannels, InterpreterTables},
    event::Event,
};

// Struture of an event for MVV.W.
#[derive(Debug, Clone)]
pub(crate) struct MVVWEvent {
    pc: u16,
    fp: u16,
    timestamp: u16,
    dst: u16,
    dst_addr: u16,
    src: u16,
    src_val: u32,
    offset: u16,
}

impl MVVWEvent {
    pub fn new(
        pc: u16,
        fp: u16,
        timestamp: u16,
        dst: u16,
        dst_addr: u16,
        src: u16,
        src_val: u32,
        offset: u16,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_addr,
            src,
            src_val,
            offset,
        }
    }

    pub fn generate_event(interpreter: &mut Interpreter, dst: u16, offset: u16, src: u16) -> Self {
        let fp = interpreter.fp;
        let dst_addr = interpreter.vrom.get(fp as usize + dst as usize) as u16;
        let src_val = interpreter.vrom.get((fp + src) as usize);
        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;

        interpreter.vrom.set((dst_addr + offset) as usize, src_val);
        interpreter.pc += 1;

        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_addr,
            src,
            src_val,
            offset,
        }
    }
}

impl Event for MVVWEvent {
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.pc + 1, self.fp, self.timestamp + 1));
    }
}
