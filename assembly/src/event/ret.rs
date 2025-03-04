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
        let fp_field = BinaryField32b::new(fp);
        Self {
            pc: interpreter.pc,
            fp,
            timestamp: interpreter.timestamp,
            fp_0_val: interpreter.vrom.get(fp_field),
            fp_1_val: interpreter.vrom.get(fp_field + BinaryField32b::ONE),
        }
    }

    pub fn generate_event(interpreter: &mut Interpreter) -> RetEvent {
        let fp = interpreter.fp;
        let fp_field = BinaryField32b::new(fp);
        if fp as usize + 1 > interpreter.vrom_size() {
            interpreter
                .vrom
                .extend(&vec![0u32; fp as usize - interpreter.vrom_size() + 2]);
        }
        let ret_event = RetEvent::new(&interpreter);
        interpreter.pc = BinaryField32b::new(interpreter.vrom.get(fp_field));
        interpreter.fp = interpreter.vrom.get(fp_field + BinaryField32b::ONE);

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
