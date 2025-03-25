//! Helper macros for [`Event`] definitions.

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
        $crate::impl_left_right_output_for_bin_op!($t, BinaryField32b);
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
    ($t:ty, $field_ty:ty) => {
        impl $crate::event::LeftOp for $t {
            type Left = $field_ty;
            fn left(&self) -> $field_ty {
                <$field_ty>::new(self.src1_val)
            }
        }
        impl $crate::event::RigthOp for $t {
            type Right = $field_ty;
            fn right(&self) -> $field_ty {
                <$field_ty>::new(self.src2_val)
            }
        }
        impl $crate::event::OutputOp for $t {
            type Output = $field_ty;
            fn output(&self) -> $field_ty {
                <$field_ty>::new(self.dst_val)
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
                channels: &mut $crate::execution::InterpreterChannels,
                _tables: &$crate::execution::InterpreterTables,
            ) {
                use $crate::event::{LeftOp, OutputOp, RigthOp};
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
            .pull(($intrp.pc, $intrp.fp, $intrp.timestamp));
        $channels.state_channel.push((
            $intrp.pc * $crate::execution::G,
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

#[macro_export]
macro_rules! define_b32_op_event {
    ($(#[$meta:meta])* $name:ident, $op_fn:expr) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone)]
        pub(crate) struct $name {
            pub(crate) timestamp: u32,
            pub(crate) pc: BinaryField32b,
            pub(crate) fp: u32,
            pub(crate) dst: u16,
            pub(crate) dst_val: u32,
            pub(crate) src1: u16,
            pub(crate) src1_val: u32,
            pub(crate) src2: u16,
            pub(crate) src2_val: u32,
        }

        impl BinaryOperation for $name {
            #[inline(always)]
            fn operation(val1: BinaryField32b, val2: BinaryField32b) -> BinaryField32b {
                $op_fn(val1, val2)
            }
        }

        $crate::impl_binary_operation!($name);
        $crate::impl_event_for_binary_operation!($name);
    };
}

#[macro_export]
macro_rules! define_b32_imm_op_event {
    ($(#[$meta:meta])* $name:ident, $op_fn:expr) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone)]
        pub(crate) struct $name {
            pub(crate) timestamp: u32,
            pub(crate) pc: BinaryField32b,
            pub(crate) fp: u32,
            pub(crate) dst: u16,
            pub(crate) dst_val: u32,
            pub(crate) src: u16,
            pub(crate) src_val: u32,
            pub(crate) imm: u16,
        }

        impl BinaryOperation for $name {
            #[inline(always)]
            fn operation(val1: BinaryField32b, imm: BinaryField16b) -> BinaryField32b {
                $op_fn(val1, imm)
            }
        }

        $crate::impl_immediate_binary_operation!($name);
        $crate::impl_event_for_binary_operation!($name);
    };
}
