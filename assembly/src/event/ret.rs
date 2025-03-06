use binius_field::{BinaryField32b, Field};

use crate::emulator::{Interpreter, InterpreterChannels, InterpreterTables};

use super::Event;

#[derive(Debug, PartialEq)]
pub struct RetEvent {
    pub(crate) pc: BinaryField32b,
    pub(crate) fp: u32,
    pub(crate) timestamp: u32,
    pub(crate) fp_0_val: u32,
    pub(crate) fp_1_val: u32,
}

impl RetEvent {
    pub fn new(interpreter: &Interpreter) -> Self {
        let fp = interpreter.fp;
        Self {
            pc: interpreter.pc,
            fp,
            timestamp: interpreter.timestamp,
            fp_0_val: interpreter.vrom.get_u32(fp),
            fp_1_val: interpreter.vrom.get_u32(fp + 4),
        }
    }

    pub fn generate_event(interpreter: &mut Interpreter) -> RetEvent {
        let fp = interpreter.fp;

        let ret_event = RetEvent::new(&interpreter);
        interpreter.jump_to(BinaryField32b::new(interpreter.vrom.get_u32(fp)));
        interpreter.fp = interpreter.vrom.get_u32(fp + 4);

        ret_event
    }
}

impl Event for RetEvent {
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels.state_channel.push((
            BinaryField32b::new(self.fp_0_val),
            self.fp_1_val,
            self.timestamp + 1,
        ));
    }
}
