use binius_field::{BinaryField16b, BinaryField32b, ExtensionField};

use super::{context::EventContext, Event};
use crate::{
    execution::{
        FramePointer, Interpreter, InterpreterChannels, InterpreterError, InterpreterTables,
    },
    ZCrayTrace,
};

/// Event for Jumpv.
///
/// Jump to the target address given as an immediate.
///
/// Logic:
/// 1. PC = FP[offset]
#[derive(Debug, Clone)]
pub(crate) struct JumpvEvent {
    pc: BinaryField32b,
    fp: FramePointer,
    timestamp: u32,
    offset: u16,
    target: u32,
}

impl Event for JumpvEvent {
    fn generate(
        ctx: &mut EventContext,
        offset: BinaryField16b,
        _unused0: BinaryField16b,
        _unused1: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let target = ctx.load_vrom_u32(ctx.addr(offset.val()))?;

        let (pc, field_pc, fp, timestamp) = ctx.program_state();

        ctx.jump_to(target.into());

        let event = Self {
            pc: field_pc,
            fp,
            timestamp,
            offset: offset.val(),
            target,
        };

        ctx.trace.jumpv.push(event);
        Ok(())
    }

    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, *self.fp, self.timestamp));
        channels
            .state_channel
            .push((BinaryField32b::new(self.target), *self.fp, self.timestamp));
    }
}

/// Event for Jumpi.
///
/// Jump to the target address given as an immediate.
///
/// Logic:
/// 1. PC = target
#[derive(Debug, Clone)]
pub(crate) struct JumpiEvent {
    pc: BinaryField32b,
    fp: FramePointer,
    timestamp: u32,
    target: BinaryField32b,
}

impl Event for JumpiEvent {
    fn generate(
        ctx: &mut EventContext,
        target_low: BinaryField16b,
        target_high: BinaryField16b,
        _unused: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let (pc, field_pc, fp, timestamp) = ctx.program_state();

        let target = (BinaryField32b::from_bases([target_low, target_high]))
            .map_err(|_| InterpreterError::InvalidInput)?;

        ctx.jump_to(target);

        let event = Self {
            pc: field_pc,
            fp,
            timestamp,
            target,
        };

        ctx.trace.jumpi.push(event);
        Ok(())
    }

    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, *self.fp, self.timestamp));
        channels.state_channel.push((
            BinaryField32b::new(self.target.val()),
            *self.fp,
            self.timestamp,
        ));
    }
}
