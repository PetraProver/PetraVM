use binius_field::{BinaryField16b, BinaryField32b, ExtensionField};

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
        _: BinaryField16b,
        _: BinaryField16b,
    ) -> Result<Self, InterpreterError> {
        let target = ctx.load_vrom_u32(ctx.addr(offset.val()))?;

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
    fn generate(
        &self,
        ctx: &mut EventContext,
        offset: BinaryField16b,
        _unused0: BinaryField16b,
        _unused1: BinaryField16b,
    ) {
        let _ = Self::generate_event(ctx, offset, _unused0, _unused1);
    }

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
        target_low: BinaryField16b,
        target_high: BinaryField16b,
        _: BinaryField16b,
    ) -> Result<Self, InterpreterError> {
        let pc = ctx.pc;
        let fp = ctx.fp;
        let timestamp = ctx.timestamp;

        let target = (BinaryField32b::from_bases([target_low, target_high]))
            .map_err(|_| InterpreterError::InvalidInput)?;

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
    fn generate(
        &self,
        ctx: &mut EventContext,
        target_low: BinaryField16b,
        target_high: BinaryField16b,
        _unused: BinaryField16b,
    ) {
        let _ = Self::generate_event(ctx, target_low, target_high, _unused);
    }

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
