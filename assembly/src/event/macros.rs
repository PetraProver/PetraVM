//! Helper macros for [`Event`] definitions.

#[macro_export]
macro_rules! impl_binary_operation {
    ($t:ty) => {
        $crate::impl_left_right_output_for_bin_op!($t, B32);
        impl $crate::event::binary_ops::NonImmediateBinaryOperation for $t {
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
    ($t:ty, $imm_field_ty:ty) => {
        impl $crate::event::binary_ops::LeftOp for $t {
            type Left = B32;
            fn left(&self) -> B32 {
                B32::new(self.src_val)
            }
        }
        impl $crate::event::binary_ops::RightOp for $t {
            type Right = $imm_field_ty;

            fn right(&self) -> $imm_field_ty {
                <$imm_field_ty>::new(self.imm)
            }
        }
        impl $crate::event::binary_ops::OutputOp for $t {
            type Output = B32;

            fn output(&self) -> B32 {
                B32::new(self.dst_val)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_left_right_output_for_bin_op {
    ($t:ty, $field_ty:ty) => {
        impl $crate::event::binary_ops::LeftOp for $t {
            type Left = $field_ty;
            fn left(&self) -> $field_ty {
                <$field_ty>::new(self.src1_val)
            }
        }
        impl $crate::event::binary_ops::RightOp for $t {
            type Right = $field_ty;
            fn right(&self) -> $field_ty {
                <$field_ty>::new(self.src2_val)
            }
        }
        impl $crate::event::binary_ops::OutputOp for $t {
            type Output = $field_ty;
            fn output(&self) -> $field_ty {
                <$field_ty>::new(self.dst_val)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_event_for_binary_operation {
    ($ty:ty, $trace_field:ident) => {
        impl $crate::event::Event for $ty {
            fn generate(
                ctx: &mut EventContext,
                arg0: B16,
                arg1: B16,
                arg2: B16,
            ) -> Result<(), InterpreterError> {
                // TODO: push to trace
                let event = Self::generate_event(ctx, arg0, arg1, arg2)?;
                ctx.trace.$trace_field.push(event);
                Ok(())
            }

            fn fire(
                &self,
                channels: &mut $crate::execution::InterpreterChannels,
                _tables: &$crate::execution::InterpreterTables,
            ) {
                use $crate::event::binary_ops::{LeftOp, OutputOp, RightOp};
                assert_eq!(self.output(), Self::operation(self.left(), self.right()));
                $crate::fire_non_jump_event!(self, channels);
            }
        }
    };
}

#[macro_export]
macro_rules! fire_non_jump_event {
    ($intrp:ident, $channels:ident) => {
        $channels
            .state_channel
            .pull(($intrp.pc, *$intrp.fp, $intrp.timestamp));
        $channels.state_channel.push((
            $intrp.pc * $crate::execution::G,
            *$intrp.fp,
            $intrp.timestamp,
        ));
    };
}

#[macro_export]
macro_rules! impl_immediate_binary_operation {
    ($t:ty) => {
        $crate::impl_left_right_output_for_imm_bin_op!($t, B16);
        impl $crate::event::binary_ops::ImmediateBinaryOperation for $t {
            fn new(
                timestamp: u32,
                pc: B32,
                fp: FramePointer,
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
        $crate::impl_left_right_output_for_imm_bin_op!($t, B32);
        #[allow(clippy::too_many_arguments)]
        impl $t {
            const fn new(
                timestamp: u32,
                pc: B32,
                fp: FramePointer,
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
macro_rules! define_bin32_op_event {
    ($(#[$meta:meta])* $name:ident, $trace_field:ident, $op_fn:expr) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone)]
        pub struct $name {
            pub timestamp: u32,
            pub pc: B32,
            pub fp: FramePointer,
            pub dst: u16,
            pub dst_val: u32,
            pub src1: u16,
            pub src1_val: u32,
            pub src2: u16,
            pub src2_val: u32,
        }

        impl BinaryOperation for $name {
            #[inline(always)]
            fn operation(val1: B32, val2: B32) -> B32 {
                $op_fn(val1, val2)
            }
        }

        $crate::impl_binary_operation!($name);
        $crate::impl_event_for_binary_operation!($name, $trace_field);
    };
}

#[macro_export]
macro_rules! define_bin32_imm_op_event {
    ($(#[$meta:meta])* $name:ident, $trace_field:ident, $op_fn:expr) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone)]
        pub struct $name {
            pub timestamp: u32,
            pub pc: B32,
            pub fp: FramePointer,
            pub dst: u16,
            pub dst_val: u32,
            pub src: u16,
            pub src_val: u32,
            pub imm: u16,
        }

        impl BinaryOperation for $name {
            #[inline(always)]
            fn operation(val1: B32, imm: B16) -> B32 {
                $op_fn(val1, imm)
            }
        }

        $crate::impl_immediate_binary_operation!($name);
        $crate::impl_event_for_binary_operation!($name, $trace_field);
    };
}

#[macro_export]
macro_rules! define_bin128_op_event {
    ($(#[$meta:meta])* $name:ident, $trace_field:ident, $op:tt) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone)]
        pub struct $name {
            pub timestamp: u32,
            pub pc: B32,
            pub fp: FramePointer,
            pub dst: u16,
            pub dst_val: u128,
            pub src1: u16,
            pub src1_val: u128,
            pub src2: u16,
            pub src2_val: u128,
        }

        impl BinaryOperation for $name {
            #[inline(always)]
            fn operation(val1: B128, val2: B128) -> B128 {
                val1 $op val2
            }
        }

        $crate::impl_left_right_output_for_bin_op!($name, B128);

        impl $name {
            pub(crate) fn generate_event(
                ctx: &mut EventContext,
                dst: B16,
                src1: B16,
                src2: B16,
            ) -> Result<Self, InterpreterError> {
                // Get source values
                let src1_val = ctx.load_vrom_u128(ctx.addr(src1.val()))?;
                let src2_val = ctx.load_vrom_u128(ctx.addr(src2.val()))?;

                // Binary field operation
                let src1_bf = B128::new(src1_val);
                let src2_bf = B128::new(src2_val);
                let dst_bf = Self::operation(src1_bf, src2_bf);
                let dst_val = dst_bf.val();

                // Store result
                ctx.store_vrom_u128(ctx.addr(dst.val()), dst_val)?;

                let (pc, field_pc, fp, timestamp) = ctx.program_state();
                ctx.incr_pc();

                Ok(Self {
                    timestamp,
                    pc: field_pc,
                    fp,
                    dst: dst.val(),
                    dst_val,
                    src1: src1.val(),
                    src1_val,
                    src2: src2.val(),
                    src2_val,
                })
            }
        }

        impl Event for $name {
            fn generate(
                ctx: &mut EventContext,
                arg0: B16,
                arg1: B16,
                arg2: B16) -> Result<(), InterpreterError> {
                let event = Self::generate_event(ctx, arg0, arg1, arg2)?;
                ctx.trace.$trace_field.push(event);

                Ok(())
            }

            fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
                use super::{LeftOp, OutputOp, RightOp};

                // Verify that the result is correct
                assert_eq!(self.output(), Self::operation(self.left(), self.right()));

                // Update state channel
                channels.state_channel.pull((self.pc, *self.fp, self.timestamp));
                channels
                    .state_channel
                    .push((self.pc * G, *self.fp, self.timestamp));
            }
        }
    };
}
