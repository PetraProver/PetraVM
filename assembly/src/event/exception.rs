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
    pub exception_slot: B16,
    pub exception_code: u8,
}

impl Event for TrapEvent {
    fn generate(
        ctx: &mut EventContext,
        exception_slot: B16,
        _unused0: B16,
        _unused1: B16,
    ) -> Result<(), InterpreterError> {
        let (_, field_pc, fp, timestamp) = ctx.program_state();

        // Read exception code from the specified slot.
        let exception_code = ctx.vrom_read::<u32>(ctx.addr(exception_slot.val()))? as u8;

        // Allocate exception frame.
        let exception_fp = ctx.vrom_mut().allocate_new_frame(3);

        // Setup exception frame.
        let packed_vals = field_pc.val() as u64 + ((*fp as u128) << 32) as u64;
        ctx.vrom_write(exception_fp, packed_vals)?;
        ctx.vrom_write(exception_fp ^ 2, exception_code as u32)?;

        // Set FP and PC.
        ctx.set_fp(exception_fp);
        ctx.jump_to(B32::ZERO);

        let trap_event = TrapEvent {
            pc: field_pc,
            fp,
            timestamp,
            exception_fp,
            exception_slot,
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
