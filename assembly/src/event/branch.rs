use binius_field::{BinaryField16b, BinaryField32b, ExtensionField};

use super::{context::EventContext, Event};
use crate::{
    execution::{
        FramePointer, Interpreter, InterpreterChannels, InterpreterError, InterpreterTables,
        ZCrayTrace,
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
    fp: FramePointer,
    cond: u16,
    con_val: u32,
    target: BinaryField32b,
}

impl Event for BnzEvent {
    fn generate(
        ctx: &mut EventContext,
        cond: BinaryField16b,
        target_low: BinaryField16b,
        target_high: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let target = (BinaryField32b::from_bases([target_low, target_high]))
            .map_err(|_| InterpreterError::InvalidInput)?;

        let cond_val = ctx.load_vrom_u32(ctx.addr(cond.val()))?;

        let (pc, field_pc, fp, timestamp) = ctx.execution_state();
        if pc == 0 {
            return Err(InterpreterError::BadPc);
        }

        let event = BnzEvent {
            timestamp,
            pc: field_pc,
            fp,
            cond: cond.val(),
            con_val: cond_val,
            target,
        };
        ctx.jump_to(target);

        ctx.trace.bnz.push(event);
        Ok(())
    }

    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        assert_ne!(self.cond, 0);
        channels
            .state_channel
            .pull((self.pc, *self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.target, *self.fp, self.timestamp));
    }
}

// TODO: Maybe this could be just a NoopEvent?
#[derive(Debug, Default, Clone)]
pub(crate) struct BzEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: FramePointer,
    cond: u16,
    cond_val: u32,
    target: BinaryField32b,
}

impl Event for BzEvent {
    fn generate(
        ctx: &mut EventContext,
        cond: BinaryField16b,
        target_low: BinaryField16b,
        target_high: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let target = (BinaryField32b::from_bases([target_low, target_high]))
            .map_err(|_| InterpreterError::InvalidInput)?;

        let (pc, field_pc, fp, timestamp) = ctx.execution_state();
        let cond_val = ctx.load_vrom_u32(ctx.addr(cond.val()))?;
        let event = BzEvent {
            timestamp,
            pc: field_pc,
            fp,
            cond: cond.val(),
            cond_val,
            target,
        };
        ctx.incr_pc();

        ctx.trace.bz.push(event);
        Ok(())
    }

    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        assert_eq!(self.cond_val, 0);
        fire_non_jump_event!(self, channels);
    }
}
