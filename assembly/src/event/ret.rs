use binius_field::{BinaryField16b, BinaryField32b};

use super::{context::EventContext, Event};
use crate::execution::{
    FramePointer, Interpreter, InterpreterChannels, InterpreterError, InterpreterTables, ZCrayTrace,
};

/// Event for RET.
///
/// Performs a return from a function call.
///
/// Logic:
///   1. PC = FP[0]
///   2. FP = FP[1]
#[derive(Debug, PartialEq, Clone)]
pub struct RetEvent {
    pub pc: BinaryField32b,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub pc_next: u32,
    pub fp_next: u32,
}

impl RetEvent {
    pub(crate) fn new(ctx: &EventContext) -> Result<Self, InterpreterError> {
        let fp = ctx.fp;
        Ok(Self {
            pc: ctx.field_pc,
            fp,
            timestamp: ctx.timestamp,
            pc_next: ctx.load_vrom_u32(ctx.addr(0u32))?,
            fp_next: ctx.load_vrom_u32(ctx.addr(1u32))?,
        })
    }
}

impl Event for RetEvent {
    fn generate(
        ctx: &mut EventContext,
        _unused0: BinaryField16b,
        _unused1: BinaryField16b,
        _unused2: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let ret_event = RetEvent::new(ctx)?;

        let target = ctx.load_vrom_u32(ctx.addr(0u32))?;
        ctx.jump_to(BinaryField32b::new(target));
        ctx.fp = ctx.load_vrom_u32(ctx.addr(1u32))?.into();

        ctx.trace.ret.push(ret_event);
        Ok(())
    }

    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, *self.fp, self.timestamp));
        channels.state_channel.push((
            BinaryField32b::new(self.pc_next),
            self.fp_next,
            self.timestamp,
        ));
    }
}
