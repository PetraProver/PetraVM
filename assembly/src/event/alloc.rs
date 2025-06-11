use binius_m3::builder::B16;

use super::context::EventContext;
use crate::{
    event::Event,
    execution::{InterpreterChannels, InterpreterError},
};

#[derive(Debug, Clone)]
pub struct AllociEvent {}

impl Event for AllociEvent {
    fn generate(
        ctx: &mut EventContext,
        dst: B16,
        imm: B16,
        _unused: B16,
    ) -> Result<(), InterpreterError> {
        let dst_addr = ctx.addr(dst.val());
        let ptr = ctx.vrom_mut().allocate_new_frame(imm.val() as u32);
        ctx.vrom_write(dst_addr, ptr)?;
        ctx.incr_counters();
        Ok(())
    }

    fn fire(&self, _channels: &mut InterpreterChannels) {}
}

#[derive(Debug, Clone)]
pub struct AllocvEvent {}

impl Event for AllocvEvent {
    fn generate(
        ctx: &mut EventContext,
        dst: B16,
        src: B16,
        _unused: B16,
    ) -> Result<(), InterpreterError> {
        let dst_addr = ctx.addr(dst.val());
        let src_val = ctx.vrom_read::<u32>(ctx.addr(src.val()))?;
        let ptr = ctx.vrom_mut().allocate_new_frame(src_val);
        ctx.vrom_write(dst_addr, ptr)?;
        ctx.incr_counters();
        Ok(())
    }

    fn fire(&self, _channels: &mut InterpreterChannels) {}
}
