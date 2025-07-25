use binius_m3::builder::{B16, B32};

use super::{context::EventContext, Event};
use crate::execution::{FramePointer, InterpreterChannels, InterpreterError};

/// Event for RET.
///
/// Performs a return from a function call.
///
/// Logic:
///   1. PC = FP[0]
///   2. FP = FP[1]
#[derive(Debug, PartialEq, Clone)]
pub struct RetEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub pc_next: u32,
    pub fp_next: u32,
}

impl RetEvent {
    pub(crate) fn new(ctx: &EventContext) -> Result<Self, InterpreterError> {
        let (_, field_pc, fp, timestamp) = ctx.program_state();

        let (pc_next, fp_next) = {
            // Perform a single packed read to get both u32 values at once.
            let pack = ctx.vrom_read::<u64>(ctx.addr(0u32))?;
            (pack as u32, (pack >> 32) as u32)
        };

        Ok(Self {
            pc: field_pc,
            fp,
            timestamp,
            pc_next,
            fp_next,
        })
    }
}

impl Event for RetEvent {
    fn generate(
        ctx: &mut EventContext,
        _unused0: B16,
        _unused1: B16,
        _unused2: B16,
    ) -> Result<(), InterpreterError> {
        let ret_event = RetEvent::new(ctx)?;

        let target = ret_event.pc_next;
        ctx.jump_to(B32::new(target));
        ctx.set_fp(ret_event.fp_next);

        ctx.trace.ret.push(ret_event);
        Ok(())
    }

    fn fire(&self, channels: &mut InterpreterChannels) {
        channels
            .state_channel
            .pull((self.pc, *self.fp, self.timestamp));
        channels
            .state_channel
            .push((B32::new(self.pc_next), self.fp_next, self.timestamp));
    }
}
