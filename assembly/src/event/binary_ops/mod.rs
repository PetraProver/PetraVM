use core::fmt::Debug;

use binius_m3::builder::{B16, B32};

use super::context::EventContext;
use crate::execution::{FramePointer, InterpreterError};

pub(crate) mod b128;
pub(crate) mod b32;

pub(crate) trait BinaryOperation: Sized + LeftOp + RightOp + OutputOp {
    fn operation(left: Self::Left, right: Self::Right) -> Self::Output;
}

pub(crate) trait LeftOp {
    type Left;

    fn left(&self) -> Self::Left;
}

pub(crate) trait RightOp {
    type Right;

    fn right(&self) -> Self::Right;
}

pub(crate) trait OutputOp {
    type Output: PartialEq + Debug;
    fn output(&self) -> Self::Output;
}
pub(crate) trait ImmediateBinaryOperation:
    BinaryOperation<Left = B32, Right = B16, Output = B32>
{
    #[allow(clippy::too_many_arguments)]
    fn new(
        timestamp: u32,
        pc: B32,
        fp: FramePointer,
        dst: u16,
        dst_val: u32,
        src: u16,
        src_val: u32,
        imm: u16,
    ) -> Self;

    fn generate_event(
        ctx: &mut EventContext,
        dst: B16,
        src: B16,
        imm: B16,
    ) -> Result<Option<Self>, InterpreterError> {
        let src_val = ctx.vrom_read::<u32>(ctx.addr(src.val()))?;
        let dst_val = Self::operation(B32::new(src_val), imm);

        ctx.vrom_write(ctx.addr(dst.val()), dst_val.val())?;
        ctx.incr_counters();
        if ctx.prover_only {
            Ok(None)
        } else {
            let (_, field_pc, fp, timestamp) = ctx.program_state();

            let event = Self::new(
                timestamp,
                field_pc,
                fp,
                dst.val(),
                dst_val.val(),
                src.val(),
                src_val,
                imm.into(),
            );

            Ok(Some(event))
        }
    }
}

pub(crate) trait NonImmediateBinaryOperation:
    BinaryOperation<Left = B32, Right = B32, Output = B32>
{
    #[allow(clippy::too_many_arguments)]
    fn new(
        timestamp: u32,
        pc: B32,
        fp: FramePointer,
        dst: u16,
        dst_val: u32,
        src1: u16,
        src1_val: u32,
        src2: u16,
        src2_val: u32,
    ) -> Self;

    fn generate_event(
        ctx: &mut EventContext,
        dst: B16,
        src1: B16,
        src2: B16,
    ) -> Result<Option<Self>, InterpreterError> {
        let src1_val = ctx.vrom_read::<u32>(ctx.addr(src1.val()))?;
        let src2_val = ctx.vrom_read::<u32>(ctx.addr(src2.val()))?;
        let dst_val = Self::operation(B32::new(src1_val), B32::new(src2_val));

        ctx.vrom_write(ctx.addr(dst.val()), dst_val.val())?;
        ctx.incr_counters();
        if ctx.prover_only {
            Ok(None)
        } else {
            let (_, field_pc, fp, timestamp) = ctx.program_state();

            let event = Self::new(
                timestamp,
                field_pc,
                fp,
                dst.val(),
                dst_val.val(),
                src1.val(),
                src1_val,
                src2.val(),
                src2_val,
            );

            Ok(Some(event))
        }
    }
}
