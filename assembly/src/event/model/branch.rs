use binius_field::{BinaryField16b, BinaryField32b, ExtensionField};

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
pub(crate) struct BnzEvent {
    pub(crate) timestamp: u32,
    pub(crate) pc: BinaryField32b,
    pub(crate) fp: u32,
    pub(crate) cond: u16,
    pub(crate) cond_val: u32,
    pub(crate) target_low: BinaryField16b,
    pub(crate) target_high: BinaryField16b,
}

impl Event for BnzEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        assert_ne!(self.cond_val, 0);
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels.state_channel.push((
            BinaryField32b::from_bases([self.target_low, self.target_high]).unwrap(),
            self.fp,
            self.timestamp + 1,
        ));
    }
}

impl BnzEvent {
    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        cond: BinaryField16b,
        target_low: BinaryField16b,
        target_high: BinaryField16b,
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
            cond_val,
            target_low,
            target_high,
        };
        interpreter.jump_to(BinaryField32b::from_bases([target_low, target_high]).unwrap());
        Ok(event)
    }
}

// TODO: Maybe this could be just a NoopEvent?
#[derive(Debug, Default, Clone)]
pub(crate) struct BzEvent {
    pub(crate) timestamp: u32,
    pub(crate) pc: BinaryField32b,
    pub(crate) fp: u32,
    pub(crate) cond: u16,
    pub(crate) cond_val: u32,
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
        };
        interpreter.incr_pc();
        Ok(event)
    }
}
