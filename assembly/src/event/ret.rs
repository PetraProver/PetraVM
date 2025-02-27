use crate::emulator::{Interpreter, InterpreterChannels, InterpreterTables};

use super::Event;

#[derive(Debug, PartialEq)]
pub struct RetEvent {
    pub(crate) pc: u16,
    pub(crate) fp: u16,
    pub(crate) timestamp: u16,
    pub(crate) fp_0_val: u16,
    pub(crate) fp_1_val: u16,
}

impl RetEvent {
    pub fn new(interpreter: &Interpreter) -> Self {
        Self {
            pc: interpreter.pc,
            fp: interpreter.fp,
            timestamp: interpreter.timestamp,
            fp_0_val: interpreter.vrom.get(interpreter.fp as usize) as u16,
            fp_1_val: interpreter.vrom.get(interpreter.fp as usize + 1) as u16,
        }
    }

    pub fn generate_event(interpreter: &mut Interpreter) -> RetEvent {
        if interpreter.fp as usize + 1 > interpreter.vrom_size() {
            interpreter.vrom.extend(&vec![
                0u32;
                interpreter.fp as usize - interpreter.vrom_size() + 2
            ]);
        }
        let ret_event = RetEvent::new(&interpreter);
        interpreter.pc = interpreter.vrom.get(interpreter.fp as usize) as u16;
        interpreter.fp = interpreter.vrom.get(interpreter.fp as usize + 1) as u16;

        ret_event
    }
}

impl Event for RetEvent {
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.fp_0_val, self.fp_1_val, self.timestamp + 1));
    }
}
