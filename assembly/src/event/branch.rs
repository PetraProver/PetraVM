use binius_field::{BinaryField16b, BinaryField32b};

use super::{context::EventContext, Event};
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
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    cond: u16,
    con_val: u32,
    target: BinaryField32b,
}

impl Event for BnzEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        assert_ne!(self.cond, 0);
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.target, self.fp, self.timestamp + 1));
    }
}

impl BnzEvent {
    pub fn generate_event(
        ctx: &mut EventContext,
        cond: BinaryField16b,
        target: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let cond_val = ctx.load_vrom_u32(cond.val())?;

        if ctx.pc == 0 {
            return Err(InterpreterError::BadPc);
        }

        let event = BnzEvent {
            timestamp: ctx.timestamp,
            pc: ctx.field_pc,
            fp: ctx.fp,
            cond: cond.val(),
            con_val: cond_val,
            target,
        };
        ctx.jump_to(target);
        Ok(event)
    }
}

// TODO: Maybe this could be just a NoopEvent?
#[derive(Debug, Default, Clone)]
pub(crate) struct BzEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    cond: u16,
    cond_val: u32,
    target: BinaryField32b,
}

impl Event for BzEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        assert_eq!(self.cond_val, 0);
        fire_non_jump_event!(self, channels);
    }
}

impl BzEvent {
    pub fn generate_event(
        ctx: &mut EventContext,
        cond: BinaryField16b,
        target: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let fp = ctx.fp;
        let cond_val = ctx.load_vrom_u32(cond.val())?;
        let event = BzEvent {
            timestamp: ctx.timestamp,
            pc: ctx.field_pc,
            fp,
            cond: cond.val(),
            cond_val,
            target,
        };
        ctx.incr_pc();
        Ok(event)
    }
}
