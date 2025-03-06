use binius_field::{BinaryField16b, BinaryField32b, Field};

use crate::{
    emulator::{Interpreter, InterpreterChannels, InterpreterTables},
    event::Event,
};

// Struture of an event for TAILI.
#[derive(Debug, Clone)]
pub(crate) struct TailiEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    target: u32,
    next_fp: u16,
    next_fp_val: u32,
    return_addr: u32,
    old_fp_val: u16,
}

impl TailiEvent {
    pub fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        target: u32,
        next_fp: u16,
        next_fp_val: u32,
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

    pub fn generate_event(
        interpreter: &mut Interpreter,
        target: BinaryField32b,
        next_fp: BinaryField16b,
    ) -> Self {
        let return_addr = interpreter.vrom.get_u32(interpreter.fp);
        let old_fp_val = interpreter.vrom.get_u32(interpreter.fp ^ 1);
        let next_fp_val = interpreter
            .vrom
            .get_u32(interpreter.fp ^ next_fp.val() as u32);
        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;
        interpreter.fp = next_fp_val;
        interpreter.jump_to(target);

        interpreter.vrom.set_u32(next_fp_val, return_addr);
        interpreter.vrom.set_u32(next_fp_val + 1, old_fp_val);

        Self {
            pc,
            fp: interpreter.fp,
            timestamp,
            target: target.val(),
            next_fp: next_fp.val(),
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
        channels.state_channel.push((
            BinaryField32b::new(self.target),
            self.next_fp_val,
            self.timestamp + 1,
        ));
    }
}
