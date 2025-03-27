use binius_field::{BinaryField16b, BinaryField32b};

use super::{context::EventContext, Event};
use crate::execution::{
    Interpreter, InterpreterChannels, InterpreterError, InterpreterTables, ZCrayTrace,
};

/// Event for RET.
///
/// Performs a return from a function call.
///
/// Logic:
///   1. PC = FP[0]
///   2. FP = FP[1]
#[derive(Debug, PartialEq)]
pub struct RetEvent {
    pub(crate) pc: BinaryField32b,
    pub(crate) fp: u32,
    pub(crate) timestamp: u32,
    pub(crate) fp_0_val: u32,
    pub(crate) fp_1_val: u32,
}

impl RetEvent {
    pub fn new(ctx: &EventContext) -> Result<Self, InterpreterError> {
        let fp = ctx.fp;
        Ok(Self {
            pc: ctx.field_pc,
            fp,
            timestamp: ctx.timestamp,
            fp_0_val: ctx.load_vrom_u32(ctx.addr(0u32))?,
            fp_1_val: ctx.load_vrom_u32(ctx.addr(1u32))?,
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
        ctx.fp = ctx.load_vrom_u32(ctx.addr(1u32))?;

        ctx.trace.ret.push(ret_event);
        Ok(())
    }

    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels.state_channel.push((
            BinaryField32b::new(self.fp_0_val),
            self.fp_1_val,
            self.timestamp + 1,
        ));
    }
}
