//! Helper macros for [`Event`](crate::event::Event) definitions.

/// Implements the
/// [`NonImmediateBinaryOperation`](crate::event::binary_ops::NonImmediateBinaryOperation)
/// and associated [`LeftOp`](crate::event::binary_ops::LeftOp),
/// [`RightOp`](crate::event::binary_ops::RightOp) and
/// [`OutputOp`](crate::event::binary_ops::OutputOp) traits for an event.
///
/// # Example
///
/// ```ignore
/// impl_binary_operation!(AddEvent)
/// ```
macro_rules! impl_binary_operation {
    ($t:ty) => {
        $crate::macros::impl_left_right_output_for_bin_op!($t, B32);
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

/// Implements the
/// [`LeftOp`](crate::event::binary_ops::LeftOp),
/// [`RightOp`](crate::event::binary_ops::RightOp) and
/// [`OutputOp`](crate::event::binary_ops::OutputOp) traits for an instruction
/// taking an immediate as second argument.
///
/// These are helper traits used to define binary operations, through the
/// [`impl_binary_operation!`] and [`impl_immediate_binary_operation`] macros.
///
/// # Example
///
/// ```ignore
/// impl_left_right_output_for_imm_bin_op!(AddEvent, B32)
/// ```
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

/// Implements the
/// [`LeftOp`](crate::event::binary_ops::LeftOp),
/// [`RightOp`](crate::event::binary_ops::RightOp) and
/// [`OutputOp`](crate::event::binary_ops::OutputOp) traits for an instruction.
///
/// These are helper traits used to define binary operations, through the
/// [`impl_binary_operation!`] and [`impl_immediate_binary_operation`] macros.
///
/// # Example
///
/// ```ignore
/// impl_left_right_output_for_bin_op!(AddEvent, B32)
/// ```
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

/// Implements the [`Event`](crate::event::Event) trait for a binary operation.
///
/// It takes as input the instruction and its corresponding field name in the
/// [`PetraTrace`](crate::execution::trace::PetraTrace) where such events are
/// being logged.
///
/// # Example
///
/// ```ignore
/// impl_event_for_binary_operation!(AddEvent, add)
/// ```
macro_rules! impl_event_for_binary_operation {
    ($ty:ty, $trace_field:ident) => {
        impl $crate::event::Event for $ty {
            fn generate(
                ctx: &mut EventContext,
                arg0: B16,
                arg1: B16,
                arg2: B16,
            ) -> Result<(), InterpreterError> {
                Self::generate_event(ctx, arg0, arg1, arg2)?.map(|event| {
                    ctx.trace.$trace_field.push(event);
                });
                Ok(())
            }

            fn fire(&self, channels: &mut $crate::execution::InterpreterChannels) {
                use $crate::event::binary_ops::{LeftOp, OutputOp, RightOp};
                assert_eq!(self.output(), Self::operation(self.left(), self.right()));
                $crate::macros::fire_non_jump_event!(self, channels);
            }
        }
    };
}

/// Implements the flushing rules for a given [`Event`](crate::event::Event)
/// that is *not* a JUMP instruction with the provided
/// [`InterpreterChannels`](crate::execution::emulator::InterpreterChannels).
///
/// # Example
///
/// ```ignore
/// fire_non_jump_event!(AddEvent, add)
/// ```
macro_rules! fire_non_jump_event {
    ($event:ident, $channels:ident) => {
        $channels
            .state_channel
            .pull(($event.pc, *$event.fp, $event.timestamp));
        $channels.state_channel.push((
            $event.pc * $crate::execution::G,
            *$event.fp,
            $event.timestamp,
        ));
    };
}

/// Implements the
/// [`ImmediateBinaryOperation`](crate::event::binary_ops::ImmediateBinaryOperation)
/// and associated [`LeftOp`](crate::event::binary_ops::LeftOp),
/// [`RightOp`](crate::event::binary_ops::RightOp) and
/// [`OutputOp`](crate::event::binary_ops::OutputOp) traits for an event.
///
/// # Example
///
/// ```ignore
/// impl_immediate_binary_operation!(AddiEvent)
/// ```
macro_rules! impl_immediate_binary_operation {
    ($t:ty) => {
        $crate::macros::impl_left_right_output_for_imm_bin_op!($t, B16);
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

/// Implements the [`LeftOp`](crate::event::binary_ops::LeftOp),
/// [`RightOp`](crate::event::binary_ops::RightOp) and
/// [`OutputOp`](crate::event::binary_ops::OutputOp) traits for an event as well
/// as its constructor.
///
/// # Example
///
/// ```ignore
/// impl_32b_immediate_binary_operation!(B32MuliEvent)
/// ```
macro_rules! impl_32b_immediate_binary_operation {
    ($t:ty) => {
        $crate::macros::impl_left_right_output_for_imm_bin_op!($t, B32);
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

/// Implements the
/// [`BinaryOperation`](crate::event::binary_ops::BinaryOperation),
/// [`NonImmediateBinaryOperation`](crate::event::binary_ops::NonImmediateBinaryOperation)
/// and [`Event`](crate::event::Event) trait for a 32-bit binary operation.
///
/// It takes as argument the instruction, with optional Rust documentation, its
/// corresponding field name in the
/// [`PetraTrace`](crate::execution::trace::PetraTrace) where such events are
/// being logged, and the operation to be applied on the instruction's inputs.
///
/// # Example
///
/// ```ignore
/// define_bin32_op_event!(
///    /// Event for ADD.
///    ///
///    /// Performs an ADD between two target addresses.
///    ///
///    /// Logic:
///    ///   1. FP[dst] = FP[src1] + FP[src2]
///    AddEvent,
///    add,
///    |a: B32, b: B32| B32::new((a.val() as i32).wrapping_add(b.val() as i32) as u32)
/// );
/// ```
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

        $crate::macros::impl_binary_operation!($name);
        $crate::macros::impl_event_for_binary_operation!($name, $trace_field);
    };
}

/// Implements the
/// [`BinaryOperation`](crate::event::binary_ops::BinaryOperation),
/// [`ImmediateBinaryOperation`](crate::event::binary_ops::ImmediateBinaryOperation)
/// and [`Event`](crate::event::Event) trait for a 32-bit immediate binary
/// operation.
///
/// It takes as argument the instruction, with optional Rust documentation, its
/// corresponding field name in the
/// [`PetraTrace`](crate::execution::trace::PetraTrace) where such events are
/// being logged, and the operation to be applied on the instruction's inputs.
///
/// # Example
///
/// ```ignore
/// define_bin32_imm_op_event!(
///    /// Event for ADDI.
///    ///
///    /// Performs an ADD between a target address and an immediate.
///    ///
///    /// Logic:
///    ///   1. FP[dst] = FP[src] + imm
///    AddiEvent,
///    addi,
///    |a: B32, imm: B16| B32::new((a.val() as i32).wrapping_add(imm.val() as i16 as i32) as u32)
/// );
/// ```
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

        $crate::macros::impl_immediate_binary_operation!($name);
        $crate::macros::impl_event_for_binary_operation!($name, $trace_field);
    };
}

/// Implements the
/// [`BinaryOperation`](crate::event::binary_ops::BinaryOperation),
/// [`NonImmediateBinaryOperation`](crate::event::binary_ops::NonImmediateBinaryOperation)
/// and [`Event`](crate::event::Event) trait for a 128-bit binary operation.
///
/// It takes as argument the instruction, with optional Rust documentation, its
/// corresponding field name in the
/// [`PetraTrace`](crate::execution::trace::PetraTrace) where such events are
/// being logged, and the operation to be applied on the instruction's inputs.
///
/// # Example
///
/// ```ignore
/// define_bin128_op_event!(
///    /// Event for B128_ADD.
///    ///
///    /// Performs a 128-bit binary field addition (XOR) between two target addresses.
///    ///
///    /// Logic:
///    ///   1. FP[dst] = __b128_add(FP[src1], FP[src2])
///    B128AddEvent,
///    b128_add,
///    +
/// );
/// ```
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

        $crate::macros::impl_left_right_output_for_bin_op!($name, B128);

        impl Event for $name {
            fn generate(
                ctx: &mut EventContext,
                dst: B16,
                src1: B16,
                src2: B16,
            ) -> Result<(), InterpreterError> {
                // Get source values
                let src1_val = ctx.vrom_read::<u128>(ctx.addr(src1.val()))?;
                let src2_val = ctx.vrom_read::<u128>(ctx.addr(src2.val()))?;

                // Binary field operation
                let src1_bf = B128::new(src1_val);
                let src2_bf = B128::new(src2_val);
                let dst_bf = Self::operation(src1_bf, src2_bf);
                let dst_val = dst_bf.val();

                // Store result
                ctx.vrom_write(ctx.addr(dst.val()), dst_val)?;
                if !ctx.prover_only {
                    let (_pc, field_pc, fp, timestamp) = ctx.program_state();

                    let event = Self {
                        timestamp,
                        pc: field_pc,
                        fp,
                        dst: dst.val(),
                        dst_val,
                        src1: src1.val(),
                        src1_val,
                        src2: src2.val(),
                        src2_val,
                    };

                    ctx.trace.$trace_field.push(event);

                }
                ctx.incr_counters();
                Ok(())
            }

            fn fire(&self, channels: &mut InterpreterChannels) {
                use super::{LeftOp, OutputOp, RightOp};

                // Verify that the result is correct
                assert_eq!(self.output(), Self::operation(self.left(), self.right()));

                // Update state channel
                $crate::macros::fire_non_jump_event!(self, channels);
            }
        }
    };
}

// Re-export macros for use in other modules
pub(crate) use {
    define_bin128_op_event, define_bin32_imm_op_event, define_bin32_op_event, fire_non_jump_event,
    impl_32b_immediate_binary_operation, impl_binary_operation, impl_event_for_binary_operation,
    impl_immediate_binary_operation, impl_left_right_output_for_bin_op,
    impl_left_right_output_for_imm_bin_op,
};
