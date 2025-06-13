use binius_field::Field;
use binius_m3::builder::{B16, B32};

use super::context::EventContext;
use crate::{
    event::Event,
    execution::{FramePointer, InterpreterChannels, InterpreterError},
};

#[derive(Debug, Clone)]
pub struct TrapEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub exception_fp: u32,
    pub exception_code: B16,
}

impl Event for TrapEvent {
    fn generate(
        ctx: &mut EventContext,
        exception_code: B16,
        _unused0: B16,
        _unused1: B16,
    ) -> Result<(), InterpreterError> {
        let (_, field_pc, fp, timestamp) = ctx.program_state();

        // Allocate exception frame.
        let exception_fp = ctx.vrom_mut().allocate_new_frame(3);

        // Setup exception frame.
        ctx.vrom_write(exception_fp, field_pc.val())?;
        ctx.vrom_write(exception_fp ^ 1, *fp)?;
        ctx.vrom_write(exception_fp ^ 2, exception_code.val() as u32)?;

        // Set FP and PC.
        ctx.set_fp(exception_fp);
        ctx.jump_to(B32::ZERO);

        let trap_event = TrapEvent {
            pc: field_pc,
            fp,
            timestamp,
            exception_fp,
            exception_code,
        };
        ctx.trace.trap.push(trap_event);
        Ok(())
    }

    fn fire(&self, channels: &mut InterpreterChannels) {
        channels
            .state_channel
            .pull((self.pc, *self.fp, self.timestamp));
        channels
            .state_channel
            .push((B32::ZERO, self.exception_fp, self.timestamp));
    }
}
