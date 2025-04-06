//! Helper macros for [`Event`] definitions.

/// Implements the [`NonImmediateBinaryOperation`] and associated
/// [`LeftOp`](crate::event::binary_ops::LeftOp),
/// [`RightOp`](crate::event::binary_ops::RightOp) and
/// [`OutputOp`](crate::event::binary_ops::OutputOp) traits for an event.
///
/// # Example
///
/// ```ignore
/// impl_binary_operation!(AddEvent)
/// ```
#[macro_export]
macro_rules! impl_binary_operation {
    ($t:ty) => {
        $crate::impl_left_right_output_for_bin_op!($t, BinaryField32b);
        impl $crate::event::binary_ops::NonImmediateBinaryOperation for $t {
            fn new(
                timestamp: u32,
                pc: BinaryField32b,
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
/// impl_left_right_output_for_imm_bin_op!(AddEvent, BinaryField32b)
/// ```
#[macro_export]
macro_rules! impl_left_right_output_for_imm_bin_op {
    ($t:ty, $imm_field_ty:ty) => {
        impl $crate::event::binary_ops::LeftOp for $t {
            type Left = BinaryField32b;
            fn left(&self) -> BinaryField32b {
                BinaryField32b::new(self.src_val)
            }
        }
        impl $crate::event::binary_ops::RightOp for $t {
            type Right = $imm_field_ty;

            fn right(&self) -> $imm_field_ty {
                <$imm_field_ty>::new(self.imm)
            }
        }
        impl $crate::event::binary_ops::OutputOp for $t {
            type Output = BinaryField32b;

            fn output(&self) -> BinaryField32b {
                BinaryField32b::new(self.dst_val)
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
/// impl_left_right_output_for_bin_op!(AddEvent, BinaryField32b)
/// ```
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

/// Implements the [`Event`](crate::event::Event) trait for a binary operation.
///
/// It takes as input the instruction and its corresponding field name in the
/// [`ZCrayTrace`](crate::execution::trace::ZCrayTrace) where such events are
/// being logged.
///
/// # Example
///
/// ```ignore
/// impl_event_for_binary_operation!(AddEvent, add)
/// ```
#[macro_export]
macro_rules! impl_event_for_binary_operation {
    ($ty:ty, $trace_field:ident) => {
        impl $crate::event::Event for $ty {
            fn generate(
                ctx: &mut EventContext,
                arg0: BinaryField16b,
                arg1: BinaryField16b,
                arg2: BinaryField16b,
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

/// Implements the flushing rules for a given [`Event`](crate::event::Event)
/// that is *not* a JUMP instruction with the provided
/// [`InterpreterChannels`](crate::execution::emulator::InterpreterChannels).
///
/// # Example
///
/// ```ignore
/// fire_non_jump_event!(AddEvent, add)
/// ```
#[macro_export]
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

/// Implements the [`ImmediateBinaryOperation`] and associated
/// [`LeftOp`](crate::event::binary_ops::LeftOp),
/// [`RightOp`](crate::event::binary_ops::RightOp) and
/// [`OutputOp`](crate::event::binary_ops::OutputOp) traits for an event.
///
/// # Example
///
/// ```ignore
/// impl_immediate_binary_operation!(AddiEvent)
/// ```
#[macro_export]
macro_rules! impl_immediate_binary_operation {
    ($t:ty) => {
        $crate::impl_left_right_output_for_imm_bin_op!($t, BinaryField16b);
        impl $crate::event::binary_ops::ImmediateBinaryOperation for $t {
            fn new(
                timestamp: u32,
                pc: BinaryField32b,
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
#[macro_export]
macro_rules! impl_32b_immediate_binary_operation {
    ($t:ty) => {
        $crate::impl_left_right_output_for_imm_bin_op!($t, BinaryField32b);
        #[allow(clippy::too_many_arguments)]
        impl $t {
            const fn new(
                timestamp: u32,
                pc: BinaryField32b,
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
/// [`ZCrayTrace`](crate::execution::trace::ZCrayTrace) where such events are
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
///    |a: BinaryField32b, b: BinaryField32b| BinaryField32b::new((a.val() as i32).wrapping_add(b.val() as i32) as u32)
/// );
/// ```
#[macro_export]
macro_rules! define_bin32_op_event {
    ($(#[$meta:meta])* $name:ident, $trace_field:ident, $op_fn:expr) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone)]
        pub struct $name {
            pub timestamp: u32,
            pub pc: BinaryField32b,
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
            fn operation(val1: BinaryField32b, val2: BinaryField32b) -> BinaryField32b {
                $op_fn(val1, val2)
            }
        }

        $crate::impl_binary_operation!($name);
        $crate::impl_event_for_binary_operation!($name, $trace_field);
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
/// [`ZCrayTrace`](crate::execution::trace::ZCrayTrace) where such events are
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
///    |a: BinaryField32b, imm: BinaryField16b| BinaryField32b::new((a.val() as i32).wrapping_add(imm.val() as i16 as i32) as u32)
/// );
/// ```
#[macro_export]
macro_rules! define_bin32_imm_op_event {
    ($(#[$meta:meta])* $name:ident, $trace_field:ident, $op_fn:expr) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone)]
        pub struct $name {
            pub timestamp: u32,
            pub pc: BinaryField32b,
            pub fp: FramePointer,
            pub dst: u16,
            pub dst_val: u32,
            pub src: u16,
            pub src_val: u32,
            pub imm: u16,
        }

        impl BinaryOperation for $name {
            #[inline(always)]
            fn operation(val1: BinaryField32b, imm: BinaryField16b) -> BinaryField32b {
                $op_fn(val1, imm)
            }
        }

        $crate::impl_immediate_binary_operation!($name);
        $crate::impl_event_for_binary_operation!($name, $trace_field);
    };
}

/// Implements the
/// [`BinaryOperation`](crate::event::binary_ops::BinaryOperation),
/// [`NonImmediateBinaryOperation`](crate::event::binary_ops::NonImmediateBinaryOperation)
/// and [`Event`](crate::event::Event) trait for a 128-bit binary operation.
///
/// It takes as argument the instruction, with optional Rust documentation, its
/// corresponding field name in the
/// [`ZCrayTrace`](crate::execution::trace::ZCrayTrace) where such events are
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
#[macro_export]
macro_rules! define_bin128_op_event {
    ($(#[$meta:meta])* $name:ident, $trace_field:ident, $op:tt) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone)]
        pub struct $name {
            pub timestamp: u32,
            pub pc: BinaryField32b,
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
            fn operation(val1: BinaryField128b, val2: BinaryField128b) -> BinaryField128b {
                val1 $op val2
            }
        }

        $crate::impl_left_right_output_for_bin_op!($name, BinaryField128b);

        impl $name {
            pub(crate) fn generate_event(
                ctx: &mut EventContext,
                dst: BinaryField16b,
                src1: BinaryField16b,
                src2: BinaryField16b,
            ) -> Result<Self, InterpreterError> {
                // Get source values
                let src1_val = ctx.load_vrom_u128(ctx.addr(src1.val()))?;
                let src2_val = ctx.load_vrom_u128(ctx.addr(src2.val()))?;

                // Binary field operation
                let src1_bf = BinaryField128b::new(src1_val);
                let src2_bf = BinaryField128b::new(src2_val);
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
                arg0: BinaryField16b,
                arg1: BinaryField16b,
                arg2: BinaryField16b) -> Result<(), InterpreterError> {
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
