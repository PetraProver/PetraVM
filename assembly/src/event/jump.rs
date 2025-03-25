use binius_field::{BinaryField16b, BinaryField32b};

use super::{context::EventContext, Event};
use crate::{
    execution::{Interpreter, InterpreterChannels, InterpreterError, InterpreterTables},
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
    fp: u32,
    timestamp: u32,
    offset: u16,
    target: u32,
}

impl JumpvEvent {
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        offset: u16,
        target: u32,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            offset,
            target,
        }
    }

    pub fn generate_event(
        ctx: &mut EventContext,
        offset: BinaryField16b,
    ) -> Result<Self, InterpreterError> {
        let target = ctx.load_vrom_u32(offset.val())?;

        let pc = ctx.pc;
        let fp = ctx.fp;
        let timestamp = ctx.timestamp;

        ctx.jump_to(target.into());

        Ok(Self {
            pc: ctx.field_pc,
            fp,
            timestamp,
            offset: offset.val(),
            target,
        })
    }
}

impl Event for JumpvEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels.state_channel.push((
            BinaryField32b::new(self.target),
            self.fp,
            self.timestamp + 1,
        ));
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
    fp: u32,
    timestamp: u32,
    target: BinaryField32b,
}

impl JumpiEvent {
    pub const fn new(pc: BinaryField32b, fp: u32, timestamp: u32, target: BinaryField32b) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            target,
        }
    }

    pub fn generate_event(
        ctx: &mut EventContext,
        target: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let pc = ctx.pc;
        let fp = ctx.fp;
        let timestamp = ctx.timestamp;

        ctx.jump_to(target);

        Ok(Self {
            pc: ctx.field_pc,
            fp,
            timestamp,
            target,
        })
    }
}

impl Event for JumpiEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels.state_channel.push((
            BinaryField32b::new(self.target.val()),
            self.fp,
            self.timestamp + 1,
        ));
    }
}
