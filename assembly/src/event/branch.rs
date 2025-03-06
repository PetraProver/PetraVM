use binius_field::{BinaryField16b, BinaryField32b};

use crate::{
    emulator::{Interpreter, InterpreterChannels, InterpreterTables},
    fire_non_jump_event,
};

use super::Event;

#[derive(Debug, Default, Clone)]
pub(crate) struct BnzEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    cond: u16,
    con_val: u32,
    target: BinaryField32b,
}

impl Event for BnzEvent {
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables) {
        assert_ne!(self.cond, 0);
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.target, self.fp, self.timestamp + 1));
    }
}

impl BnzEvent {
    pub fn generate_event(
        interpreter: &mut Interpreter,
        cond: BinaryField16b,
        target: BinaryField32b,
    ) -> BnzEvent {
        let cond_val = interpreter.vrom.get_u32(interpreter.fp ^ cond.val() as u32);
        let event = BnzEvent {
            timestamp: interpreter.timestamp,
            pc: interpreter.pc,
            fp: interpreter.fp,
            cond: cond.val(),
            con_val: cond_val,
            target,
        };
        interpreter.jump_to(target);
        event
    }
}

// TODO: Maybe this could be jus NoopEvent?
#[derive(Debug, Default, Clone)]
pub(crate) struct BzEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    cond: u16,
    cond_val: u32,
    target: BinaryField32b,
}

impl Event for BzEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        assert_eq!(self.cond_val, 0);
        fire_non_jump_event!(self, channels);
    }
}

impl BzEvent {
    pub fn generate_event(
        interpreter: &mut Interpreter,
        cond: BinaryField16b,
        target: BinaryField32b,
    ) -> BzEvent {
        let fp = interpreter.fp;
        let cond_val = interpreter.vrom.get_u32(fp ^ cond.val() as u32);
        let event = BzEvent {
            timestamp: interpreter.timestamp,
            pc: interpreter.pc,
            fp,
            cond: cond.val(),
            cond_val,
            target,
        };
        interpreter.incr_pc();
        event
    }
}
