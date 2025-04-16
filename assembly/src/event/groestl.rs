use binius_hash::{
    compression,
    groestl::{Groestl256ByteCompression, GroestlShortImpl, GroestlShortInternal},
    PseudoCompressionFunction,
};
use binius_m3::builder::{B16, B32};
use bytemuck::{cast_slice, Pod};
use generic_array::GenericArray;

use super::{context::EventContext, Event};
use crate::{
    execution::{FramePointer, InterpreterChannels, InterpreterError},
    fire_non_jump_event,
};

/// Event for GROESTL256_COMPRESS.
///
/// Performs a Groestl compression between two 512-bit inputs.
#[derive(Debug, Clone)]
pub struct GroestlCompressEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub dst: u16,
    pub dst_val: [u64; 8],
    pub src1: u16,
    pub src1_val: [u8; 64],
    pub src2: u16,
    pub src2_val: [u8; 64],
}

impl Event for GroestlCompressEvent {
    fn generate(
        ctx: &mut EventContext,
        dst: B16,
        src1: B16,
        src2: B16,
    ) -> Result<(), InterpreterError> {
        let mut src1_val = Vec::with_capacity(16);
        for i in 0..16 {
            src1_val.push(ctx.vrom_read::<u32>(ctx.addr(src1.val() + i))?);
        }
        let src1_val = cast_slice::<u32, u8>(&src1_val);
        let mut src2_val = Vec::with_capacity(16);
        for i in 0..16 {
            src2_val.push(ctx.vrom_read::<u32>(ctx.addr(src2.val() + i))?);
        }
        let src2_val = cast_slice::<u32, u8>(&src2_val);

        let mut dst_val = GroestlShortImpl::state_from_bytes(src1_val.try_into().unwrap());
        <GroestlShortImpl as GroestlShortInternal>::compress(
            &mut dst_val,
            src2_val.try_into().unwrap(),
        );

        for i in 0..8 {
            ctx.vrom_write::<u64>(ctx.addr(dst.val() + 2 * i), dst_val[i as usize])?;
        }

        let (_pc, field_pc, fp, timestamp) = ctx.program_state();
        ctx.incr_pc();

        let event = Self {
            pc: field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            dst_val,
            src1: src1.val(),
            src1_val: src1_val.try_into().unwrap(),
            src2: src2.val(),
            src2_val: src2_val.try_into().unwrap(),
        };

        ctx.trace.groestl_compress.push(event);
        Ok(())
    }

    fn fire(&self, channels: &mut InterpreterChannels) {
        fire_non_jump_event!(self, channels);
    }
}

/// Event for GROESTL_OUTPUT.
///
/// Performs a Groestl compression between two 512-bit inputs.
#[derive(Debug, Clone)]
pub struct GroestlOutputEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub dst: u16,
    pub dst_val: [u32; 8],
    pub src1: u16,
    pub src1_val: [u8; 32],
    pub src2: u16,
    pub src2_val: [u8; 32],
}

impl Event for GroestlOutputEvent {
    #[allow(clippy::default_constructed_unit_structs)]
    fn generate(
        ctx: &mut EventContext,
        dst: B16,
        src1: B16,
        src2: B16,
    ) -> Result<(), InterpreterError> {
        let mut src1_val = Vec::with_capacity(16);
        for i in 0..8 {
            src1_val.push(ctx.vrom_read::<u32>(ctx.addr(src1.val() + i))?);
        }
        let src1_val = cast_slice::<u32, u8>(&src1_val);
        let mut src2_val = Vec::with_capacity(16);
        for i in 0..8 {
            src2_val.push(ctx.vrom_read::<u32>(ctx.addr(src2.val() + i))?);
        }
        let src2_val = cast_slice::<u32, u8>(&src2_val);

        let compression = Groestl256ByteCompression::default();
        let src1_array = GenericArray::from_slice(src1_val);
        let src2_array = GenericArray::from_slice(src2_val);
        let dst_val = compression.compress([*src1_array, *src2_array]);
        let dst_val = cast_slice::<u8, u32>(&dst_val);
        for i in 0..8 {
            ctx.vrom_write(ctx.addr(dst.val() + i), dst_val[i as usize])?;
        }

        let (_pc, field_pc, fp, timestamp) = ctx.program_state();
        ctx.incr_pc();

        let event = Self {
            pc: field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            dst_val: dst_val.try_into().unwrap(),
            src1: src1.val(),
            src1_val: src1_val.try_into().unwrap(),
            src2: src2.val(),
            src2_val: src2_val.try_into().unwrap(),
        };

        ctx.trace.groestl_output.push(event);
        Ok(())
    }

    fn fire(&self, channels: &mut InterpreterChannels) {
        fire_non_jump_event!(self, channels);
    }
}
