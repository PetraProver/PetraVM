use crate::emulator::{Interpreter, StateChannel};

use super::Event;

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
            pc: interpreter.pc,
            fp: interpreter.fp,
            timestamp: interpreter.timestamp,
            fp_0_val: interpreter.vrom[interpreter.fp as usize] as u16,
            fp_1_val: interpreter.vrom[interpreter.fp as usize + 1] as u16,
        }
    }

    pub fn generate_event(interpreter: &mut Interpreter) -> RetEvent {
        if interpreter.fp as usize + 1 > interpreter.vrom_size() {
            interpreter.vrom.extend(&vec![
                0u32;
                interpreter.fp as usize - interpreter.vrom_size() + 2
            ]);
        }
        interpreter.pc = interpreter.vrom[interpreter.fp as usize] as u16;
        interpreter.fp = interpreter.vrom[interpreter.fp as usize + 1] as u16;
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
