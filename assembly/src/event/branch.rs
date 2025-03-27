use binius_field::{BinaryField16b, BinaryField32b};

use super::Event;
use crate::{
    execution::{
        Interpreter, InterpreterChannels, InterpreterError, InterpreterTables, ZCrayTrace,
    },
    fire_non_jump_event,
};

/// Event for BNZ.
///
/// Performs a branching to the target address if the argument is not zero.
///
/// Logic:
///   1. if FP[cond] <> 0, then PC = target
///   2. if FP[cond] == 0, then increment PC
#[derive(Debug, Default, Clone)]
pub struct BnzEvent {
    pub timestamp: u32,
    pub pc: BinaryField32b,
    pub fp: u32,
    pub cond: u16,
    pub con_val: u32,
    pub target: BinaryField32b,
}

impl Event for BnzEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        assert_ne!(self.cond, 0);
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.target, self.fp, self.timestamp));
    }
}

impl BnzEvent {
    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        cond: BinaryField16b,
        target: BinaryField32b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let cond_val = trace.get_vrom_u32(interpreter.fp ^ cond.val() as u32)?;

        if interpreter.pc == 0 {
            return Err(InterpreterError::BadPc);
        }

        let event = BnzEvent {
            timestamp: interpreter.timestamp,
            pc: field_pc,
            fp: interpreter.fp,
            cond: cond.val(),
            con_val: cond_val,
            target,
        };
        interpreter.jump_to(target);
        Ok(event)
    }
}

// TODO: Maybe this could be just a NoopEvent?
#[derive(Debug, Default, Clone)]
pub struct BzEvent {
    pub timestamp: u32,
    pub pc: BinaryField32b,
    pub fp: u32,
    pub cond: u16,
    pub cond_val: u32,
    pub target: BinaryField32b,
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
        trace: &mut ZCrayTrace,
        cond: BinaryField16b,
        target: BinaryField32b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let fp = interpreter.fp;
        let cond_val = trace.get_vrom_u32(fp ^ cond.val() as u32)?;
        let event = BzEvent {
            timestamp: interpreter.timestamp,
            pc: field_pc,
            fp,
            cond: cond.val(),
            cond_val,
            target,
        };
        interpreter.incr_pc();
        Ok(event)
    }
}
