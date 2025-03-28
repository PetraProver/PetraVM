use binius_field::{BinaryField16b as B16, BinaryField32b as B32, ExtensionField};

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
    pub pc: B32,
    pub fp: u32,
    pub cond: u16,
    pub cond_val: u32,
    pub target_low: B16,
    pub target_high: B16,
}

impl Event for BnzEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        assert_ne!(self.cond_val, 0);
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels.state_channel.push((
            B32::from_bases([self.target_low, self.target_high]).unwrap(),
            self.fp,
            self.timestamp,
        ));
    }
}

impl BnzEvent {
    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        cond: B16,
        target_low: B16,
        target_high: B16,
        field_pc: B32,
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
            cond_val,
            target_low,
            target_high,
        };
        interpreter.jump_to(B32::from_bases([target_low, target_high]).unwrap());
        Ok(event)
    }
}

// TODO: Maybe this could be just a NoopEvent?
#[derive(Debug, Default, Clone)]
pub struct BzEvent {
    pub timestamp: u32,
    pub pc: B32,
    pub fp: u32,
    pub cond: u16,
    pub target_low: B16,
    pub target_high: B16,
}

impl Event for BzEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        fire_non_jump_event!(self, channels);
    }
}

impl BzEvent {
    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        cond: B16,
        target_low: B16,
        target_high: B16,
        field_pc: B32,
    ) -> Result<Self, InterpreterError> {
        let fp = interpreter.fp;
        let cond_val = trace.get_vrom_u32(fp ^ cond.val() as u32)?;
        let event = BzEvent {
            timestamp: interpreter.timestamp,
            pc: field_pc,
            fp,
            cond: cond.val(),
            target_low,
            target_high,
        };
        interpreter.incr_pc();
        Ok(event)
    }
}
