use binius_field::ExtensionField;
use binius_m3::builder::{B16, B32};

use super::{context::EventContext, Event};
use crate::execution::{FramePointer, InterpreterChannels, InterpreterError};

/// Event for Jumpv.
///
/// Jump to the target address given as an immediate.
///
/// Logic:
/// 1. PC = FP[offset]
#[derive(Debug, Clone)]
pub struct JumpvEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub offset: u16,
    pub target: u32,
}

impl Event for JumpvEvent {
    fn generate(
        ctx: &mut EventContext,
        offset: B16,
        _unused0: B16,
        _unused1: B16,
    ) -> Result<(), InterpreterError> {
        let target = ctx.vrom_read::<u32>(ctx.addr(offset.val()))?;

        let (_pc, field_pc, fp, timestamp) = ctx.program_state();

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

    fn fire(&self, channels: &mut InterpreterChannels) {
        channels
            .state_channel
            .pull((self.pc, *self.fp, self.timestamp));
        channels
            .state_channel
            .push((B32::new(self.target), *self.fp, self.timestamp));
    }
}

/// Event for Jumpi.
///
/// Jump to the target address given as an immediate.
///
/// Logic:
/// 1. PC = target
#[derive(Debug, Clone)]
pub struct JumpiEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub target: B32,
}

impl Event for JumpiEvent {
    fn generate(
        ctx: &mut EventContext,
        target_low: B16,
        target_high: B16,
        _unused: B16,
    ) -> Result<(), InterpreterError> {
        let (_pc, field_pc, fp, timestamp) = ctx.program_state();

        let target = (B32::from_bases([target_low, target_high]))
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

    fn fire(&self, channels: &mut InterpreterChannels) {
        channels
            .state_channel
            .pull((self.pc, *self.fp, self.timestamp));
        channels
            .state_channel
            .push((B32::new(self.target.val()), *self.fp, self.timestamp));
    }
}
