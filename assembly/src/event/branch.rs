use crate::emulator::{Interpreter, InterpreterChannels, InterpreterTables};

use super::Event;

#[derive(Debug, Default, Clone)]
pub(crate) struct BnzEvent {
    timestamp: u16,
    pc: u16,
    fp: u16,
    cond: u16,
    con_val: u32,
    target: u16,
}

impl Event for BnzEvent {
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables) {
        unimplemented!();
    }
}

impl BnzEvent {
    pub fn generate_event(interpreter: &mut Interpreter, cond: u16, target: u16) -> BnzEvent {
        let cond_val = interpreter
            .vrom
            .get(interpreter.fp as usize + cond as usize);
        let event = BnzEvent {
            timestamp: interpreter.timestamp,
            pc: interpreter.pc,
            fp: interpreter.fp,
            cond,
            con_val: cond_val,
            target,
        };
        if cond_val != 0 {
            interpreter.pc = target as u16;
        } else {
            interpreter.pc += 1;
        }
        event
    }
}
