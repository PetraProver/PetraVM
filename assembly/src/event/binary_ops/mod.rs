use core::fmt::Debug;

use binius_field::{BinaryField16b, BinaryField32b};

use crate::{execution::InterpreterError, ZCrayTrace};

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
// TODO: Add type parameter for operation over other fields?
pub(crate) trait ImmediateBinaryOperation:
    BinaryOperation<Left = BinaryField32b, Right = BinaryField16b, Output = BinaryField32b>
{
    // TODO: Add some trick to implement new only once
    #[allow(clippy::too_many_arguments)]
    fn new(
        timestamp: u32,
        pc: BinaryField32b,
        fp: u32,
        dst: u16,
        dst_val: u32,
        src: u16,
        src_val: u32,
        imm: u16,
    ) -> Self;

    fn generate_event(
        interpreter: &mut crate::execution::Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let src_val = trace.get_vrom_u32(interpreter.fp ^ src.val() as u32)?;
        let dst_val = Self::operation(BinaryField32b::new(src_val), imm);
        let event = Self::new(
            interpreter.timestamp,
            field_pc,
            interpreter.fp,
            dst.val(),
            dst_val.val(),
            src.val(),
            src_val,
            imm.into(),
        );
        trace.set_vrom_u32(interpreter.fp ^ dst.val() as u32, dst_val.val())?;
        interpreter.incr_pc();
        Ok(event)
    }
}

pub(crate) trait NonImmediateBinaryOperation:
    BinaryOperation<Left = BinaryField32b, Right = BinaryField32b, Output = BinaryField32b>
{
    #[allow(clippy::too_many_arguments)]
    fn new(
        timestamp: u32,
        pc: BinaryField32b,
        fp: u32,
        dst: u16,
        dst_val: u32,
        src1: u16,
        src1_val: u32,
        src2: u16,
        src2_val: u32,
    ) -> Self;

    fn generate_event(
        interpreter: &mut crate::execution::Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let src1_val = trace.get_vrom_u32(interpreter.fp ^ src1.val() as u32)?;
        let src2_val = trace.get_vrom_u32(interpreter.fp ^ src2.val() as u32)?;
        let dst_val = Self::operation(BinaryField32b::new(src1_val), BinaryField32b::new(src2_val));
        let event = Self::new(
            interpreter.timestamp,
            field_pc,
            interpreter.fp,
            dst.val(),
            dst_val.val(),
            src1.val(),
            src1_val,
            src2.val(),
            src2_val,
        );
        trace.set_vrom_u32(interpreter.fp ^ dst.val() as u32, dst_val.val())?;
        interpreter.incr_pc();
        Ok(event)
    }
}
