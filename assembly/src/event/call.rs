use std::cmp::max;

use crate::{
    emulator::{Interpreter, InterpreterChannels, InterpreterTables},
    event::Event,
};

// Struture of an event for TAILI.
#[derive(Debug, Clone)]
pub(crate) struct TailiEvent {
    pc: u16,
    fp: u16,
    timestamp: u16,
    target: u16,
    next_fp: u16,
    next_fp_val: u16,
    return_addr: u32,
    old_fp_val: u16,
}

impl TailiEvent {
    pub fn new(
        pc: u16,
        fp: u16,
        timestamp: u16,
        target: u16,
        next_fp: u16,
        next_fp_val: u16,
        return_addr: u32,
        old_fp_val: u16,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            target,
            next_fp,
            next_fp_val,
            return_addr,
            old_fp_val,
        }
    }

    pub fn generate_event(interpreter: &mut Interpreter, target: u16, next_fp: u16) -> Self {
        let fp = interpreter.fp;
        let return_addr = interpreter.vrom.get(fp as usize);
        let old_fp_val = interpreter.vrom.get(fp as usize + 1);
        let next_fp_val = interpreter.vrom.get((fp + next_fp) as usize) as u16;
        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;
        interpreter.fp = next_fp_val as u16;
        interpreter.pc = target;

        interpreter.vrom.set(next_fp_val as usize, return_addr);
        interpreter.vrom.set(next_fp_val as usize + 1, old_fp_val);

        Self {
            pc,
            fp,
            timestamp,
            target,
            next_fp,
            next_fp_val,
            return_addr,
            old_fp_val: old_fp_val as u16,
        }
    }
}

impl Event for TailiEvent {
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.target, self.next_fp_val, self.timestamp + 1));
    }
}
