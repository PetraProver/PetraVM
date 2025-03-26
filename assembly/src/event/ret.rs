use binius_field::BinaryField32b;

use super::Event;
use crate::execution::{
    Interpreter, InterpreterChannels, InterpreterError, InterpreterTables, ZCrayTrace,
};

/// Event for RET.
///
/// Performs a return from a function call.
///
/// Logic:
///   1. PC = FP[0]
///   2. FP = FP[1]
#[derive(Debug, PartialEq)]
pub struct RetEvent {
    pub(crate) pc: BinaryField32b,
    pub(crate) fp: u32,
    pub(crate) timestamp: u32,
    pub(crate) fp_0_val: u32,
    pub(crate) fp_1_val: u32,
}

impl RetEvent {
    pub fn new(
        interpreter: &Interpreter,
        trace: &ZCrayTrace,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let fp = interpreter.fp;
        Ok(Self {
            pc: field_pc,
            fp,
            timestamp: interpreter.timestamp,
            fp_0_val: trace.get_vrom_u32(fp)?,
            fp_1_val: trace.get_vrom_u32(fp ^ 1)?,
        })
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &ZCrayTrace,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let fp = interpreter.fp;
        let ret_event = RetEvent::new(interpreter, trace, field_pc);
        interpreter.jump_to(BinaryField32b::new(trace.get_vrom_u32(fp)?));
        interpreter.fp = trace.get_vrom_u32(fp ^ 1)?;

        ret_event
    }
}

impl Event for RetEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels.state_channel.push((
            BinaryField32b::new(self.fp_0_val),
            self.fp_1_val,
            self.timestamp,
        ));
    }
}
