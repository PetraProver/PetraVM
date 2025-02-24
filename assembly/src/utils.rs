use crate::emulator::{Interpreter, StateChannel};

pub trait Event {
    fn fire(&self, state_channel: &mut StateChannel);
}

#[derive(Debug)]
pub struct RetEvent {
    pc: u16,
    fp: u16,
    timestamp: u16,
    fp_0_val: u16,
    fp_1_val: u16,
}

impl RetEvent {
    pub fn new(interpreter: &Interpreter) -> Self {
        Self {
            pc: interpreter.get_pc(),
            fp: interpreter.get_fp(),
            timestamp: interpreter.get_timestamp(),
            fp_0_val: interpreter.get_vrom_index(interpreter.get_fp() as usize) as u16,
            fp_1_val: interpreter.get_vrom_index(interpreter.get_fp() as usize + 1) as u16,
        }
    }

    pub fn generate_event(interpreter: &mut Interpreter) -> RetEvent {
        interpreter.set_pc(interpreter.get_vrom_index(interpreter.get_fp() as usize) as u16);
        interpreter.set_fp(interpreter.get_vrom_index(interpreter.get_fp() as usize + 1) as u16);
        interpreter.set_timestamp(interpreter.get_timestamp() + 1);
        RetEvent::new(&interpreter)
    }
}

impl Event for RetEvent {
    fn fire(&self, state_channel: &mut StateChannel) {
        state_channel.pull((self.pc, self.fp, self.timestamp));
        state_channel.push((self.fp_0_val, self.fp_1_val, self.timestamp + 1));
    }
}
