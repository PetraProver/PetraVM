//! Defines execution events for the zCray VM.
//!
//! Each event represents an instruction executed by the VM, such as arithmetic
//! operations, branching, or function calls.

use std::fmt::Debug;

use binius_field::{BinaryField16b, BinaryField32b};

use crate::emulator::{InterpreterChannels, InterpreterTables};

pub(crate) mod b32;
pub(crate) mod branch;
pub(crate) mod call;
pub(crate) mod integer_ops;
pub(crate) mod mv;
pub(crate) mod ret;
pub(crate) mod sli;

/// An `Event` represents an instruction that can be executed by the VM.
pub trait Event {
    /// Executes the flushing rules associated to this `Event`, pushing to /
    /// pulling from their target channels.
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables);
}
pub(crate) trait BinaryOperation: Sized + LeftOp + RigthOp + OutputOp {
    fn operation(left: Self::Left, right: Self::Right) -> Self::Output;
}

pub(crate) trait LeftOp {
    type Left;

    fn left(&self) -> Self::Left;
}

pub(crate) trait RigthOp {
    type Right;

    fn right(&self) -> Self::Right;
}

pub(crate) trait OutputOp {
    type Output: PartialEq + Debug;
    fn output(&self) -> Self::Output;
}

// TODO: Add type paraeter for operation over other fields?
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
        interpreter: &mut crate::emulator::Interpreter,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
    ) -> Self {
        let src_val = interpreter.vrom.get_u32(interpreter.fp ^ src.val() as u32);
        let dst_val = Self::operation(BinaryField32b::new(src_val), imm);
        let event = Self::new(
            interpreter.timestamp,
            interpreter.pc,
            interpreter.fp,
            dst.val(),
            dst_val.val(),
            src.val(),
            src_val,
            imm.into(),
        );
        interpreter
            .vrom
            .set_u32(interpreter.fp ^ dst.val() as u32, dst_val.val());
        interpreter.incr_pc();
        event
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
        interpreter: &mut crate::emulator::Interpreter,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
    ) -> Self {
        let src1_val = interpreter.vrom.get_u32(interpreter.fp ^ src1.val() as u32);
        let src2_val = interpreter.vrom.get_u32(interpreter.fp ^ src2.val() as u32);
        let dst_val = Self::operation(BinaryField32b::new(src1_val), BinaryField32b::new(src2_val));
        let event = Self::new(
            interpreter.timestamp,
            interpreter.pc,
            interpreter.fp,
            dst.val(),
            dst_val.val(),
            src1.val(),
            src1_val,
            src2.val(),
            src2_val,
        );
        interpreter
            .vrom
            .set_u32(interpreter.fp ^ dst.val() as u32, dst_val.val());
        interpreter.incr_pc();
        event
    }
}

#[macro_export]
macro_rules! impl_immediate_binary_operation {
    ($t:ty) => {
        $crate::impl_left_right_output_for_imm_bin_op!($t);
        impl $crate::event::ImmediateBinaryOperation for $t {
            fn new(
                timestamp: u32,
                pc: BinaryField32b,
                fp: u32,
                dst: u16,
                dst_val: u32,
                src: u16,
                src_val: u32,
                imm: u16,
            ) -> Self {
                Self {
                    timestamp,
                    pc,
                    fp,
                    dst,
                    dst_val,
                    src,
                    src_val,
                    imm,
                }
            }
        }
    };
}

#[macro_export]
macro_rules! impl_32b_immediate_binary_operation {
    ($t:ty) => {
        $crate::impl_left_right_output_for_b32imm_bin_op!($t);
        #[allow(clippy::too_many_arguments)]
        impl $t {
            const fn new(
                timestamp: u32,
                pc: BinaryField32b,
                fp: u32,
                dst: u16,
                dst_val: u32,
                src: u16,
                src_val: u32,
                imm: u32,
            ) -> Self {
                Self {
                    timestamp,
                    pc,
                    fp,
                    dst,
                    dst_val,
                    src,
                    src_val,
                    imm,
                }
            }
        }
    };
}

#[macro_export]
macro_rules! impl_binary_operation {
    ($t:ty) => {
        $crate::impl_left_right_output_for_bin_op!($t);
        impl $crate::event::NonImmediateBinaryOperation for $t {
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
            ) -> Self {
                Self {
                    timestamp,
                    pc,
                    fp,
                    dst,
                    dst_val,
                    src1,
                    src1_val,
                    src2,
                    src2_val,
                }
            }
        }
    };
}

#[macro_export]
macro_rules! impl_left_right_output_for_imm_bin_op {
    ($t:ty) => {
        impl $crate::event::LeftOp for $t {
            type Left = BinaryField32b;
            fn left(&self) -> BinaryField32b {
                BinaryField32b::new(self.src_val)
            }
        }
        impl $crate::event::RigthOp for $t {
            type Right = BinaryField16b;

            fn right(&self) -> BinaryField16b {
                BinaryField16b::new(self.imm)
            }
        }
        impl $crate::event::OutputOp for $t {
            type Output = BinaryField32b;

            fn output(&self) -> BinaryField32b {
                BinaryField32b::new(self.dst_val)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_left_right_output_for_b32imm_bin_op {
    ($t:ty) => {
        impl $crate::event::LeftOp for $t {
            type Left = BinaryField32b;
            fn left(&self) -> BinaryField32b {
                BinaryField32b::new(self.src_val)
            }
        }
        impl $crate::event::RigthOp for $t {
            type Right = BinaryField32b;

            fn right(&self) -> BinaryField32b {
                BinaryField32b::new(self.imm)
            }
        }
        impl $crate::event::OutputOp for $t {
            type Output = BinaryField32b;

            fn output(&self) -> BinaryField32b {
                BinaryField32b::new(self.dst_val)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_left_right_output_for_bin_op {
    ($t:ty) => {
        impl $crate::event::LeftOp for $t {
            type Left = BinaryField32b;
            fn left(&self) -> BinaryField32b {
                BinaryField32b::new(self.src1_val)
            }
        }
        impl $crate::event::RigthOp for $t {
            type Right = BinaryField32b;

            fn right(&self) -> BinaryField32b {
                BinaryField32b::new(self.src2_val)
            }
        }
        impl $crate::event::OutputOp for $t {
            type Output = BinaryField32b;

            fn output(&self) -> BinaryField32b {
                BinaryField32b::new(self.dst_val)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_event_for_binary_operation {
    ($ty:ty) => {
        impl $crate::event::Event for $ty {
            fn fire(
                &self,
                channels: &mut $crate::emulator::InterpreterChannels,
                _tables: &$crate::emulator::InterpreterTables,
            ) {
                use $crate::event::{LeftOp, OutputOp, RigthOp};
                assert_eq!(self.output(), Self::operation(self.left(), self.right()));
                fire_non_jump_event!(self, channels);
            }
        }
    };
}

#[macro_export]
macro_rules! fire_non_jump_event {
    ($intrp:ident, $channels:ident) => {
        $channels
            .state_channel
            .pull(($intrp.pc, $intrp.fp, $intrp.timestamp));
        $channels.state_channel.push((
            $intrp.pc * $crate::emulator::G,
            $intrp.fp,
            $intrp.timestamp + 1,
        ));
    };
}

#[macro_export]
macro_rules! impl_event_no_interaction_with_state_channel {
    ($t:ty) => {
        impl Event for $t {
            fn fire(&self, _channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
                // No interaction with the state channel.
            }
        }
    };
}
