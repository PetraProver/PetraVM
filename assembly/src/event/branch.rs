use binius_field::{BinaryField16b, BinaryField32b};

use crate::emulator::{Interpreter, InterpreterChannels, InterpreterTables};

use super::Event;

#[derive(Debug, Default, Clone)]
pub(crate) struct BnzEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    cond: u16,
    con_val: u32,
    target: u32,
}

impl Event for BnzEvent {
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables) {
        unimplemented!();
    }
}

impl BnzEvent {
    pub fn generate_event(
        interpreter: &mut Interpreter,
        cond: BinaryField16b,
        target: BinaryField32b,
    ) -> BnzEvent {
        let fp_field = BinaryField32b::new(interpreter.fp);
        let cond_val = interpreter.vrom.get(fp_field + cond);
        let event = BnzEvent {
            timestamp: interpreter.timestamp,
            pc: interpreter.pc,
            fp: interpreter.fp,
            cond: cond.val(),
            con_val: cond_val,
            target: target.val(),
        };
        if cond_val != 0 {
            interpreter.pc = target;
        } else {
            interpreter.incr_pc();
        }
        event
    }
}
