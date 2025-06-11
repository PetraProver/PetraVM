use binius_m3::builder::{B16, B32};

use super::context::EventContext;
use crate::{
    event::Event,
    execution::{FramePointer, InterpreterChannels, InterpreterError},
    macros::fire_non_jump_event,
};

/// Event for FP.
///
/// Stores FP + immediate at a destination.
///
/// Logic:
///   1. FP[dst] = FP + imm
#[derive(Debug, Clone)]
pub struct FpEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub dst: u16,
    pub imm: u16,
}

impl Event for FpEvent {
    fn generate(
        ctx: &mut EventContext,
        dst: B16,
        imm: B16,
        _unused: B16,
    ) -> Result<(), InterpreterError> {
        let imm_val = imm.val();
        let dst_addr = ctx.addr(dst.val());
        ctx.vrom_write(dst_addr, ctx.addr(imm_val))?;

        if !ctx.prover_only {
            let (_pc, field_pc, fp, timestamp) = ctx.program_state();

            let event = Self {
                pc: field_pc,
                fp,
                timestamp,
                dst: dst.val(),
                imm: imm_val,
            };

            ctx.trace.fp.push(event);
        }

        ctx.incr_counters();
        Ok(())
    }

    fn fire(&self, channels: &mut InterpreterChannels) {
        fire_non_jump_event!(self, channels);
    }
}
