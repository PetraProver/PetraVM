use crate::emulator::{Interpreter, StateChannel};

pub trait Event {
    fn fire(&self, state_channel: &mut StateChannel);
}

#[derive(Debug)]
pub(crate) struct RetEvent {
    pc: u16,
    fp: u16,
    timestamp: u16,
    fp_0_val: u16,
    fp_1_val: u16,
}

impl RetEvent {
    pub(crate) fn new(interpreter: &Interpreter) -> Self {
        let fp = interpreter.fp as usize;
        Self {
            pc: interpreter.pc,
            fp: interpreter.fp,
            timestamp: interpreter.timestamp,
            fp_0_val: interpreter.vrom[fp] as u16,
            fp_1_val: interpreter.vrom[fp + 1] as u16,
        }
    }

    pub(crate) fn generate_event(interpreter: &mut Interpreter) -> RetEvent {
        let fp = interpreter.fp as usize;
        interpreter.pc = interpreter.vrom[fp] as u16;
        interpreter.fp = interpreter.vrom[fp + 1] as u16;
        interpreter.timestamp = interpreter.timestamp + 1;
        RetEvent::new(&interpreter)
    }
}

impl Event for RetEvent {
    fn fire(&self, state_channel: &mut StateChannel) {
        state_channel.pull((self.pc, self.fp, self.timestamp));
        state_channel.push((self.fp_0_val, self.fp_1_val, self.timestamp + 1));
    }
}
