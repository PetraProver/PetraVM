use binius_field::{BinaryField16b, BinaryField32b};

use crate::emulator::{InterpreterChannels, InterpreterTables};

pub(crate) mod b32;
pub(crate) mod branch;
pub(crate) mod call;
pub(crate) mod integer_ops;
pub(crate) mod mv;
pub(crate) mod ret;
pub(crate) mod sli;

pub trait Event {
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables);
}

pub(crate) trait BinaryOperation<T: From<BinaryField16b>>: Sized {
    fn operation(left: BinaryField32b, right: T) -> BinaryField32b;
}

// TODO: Add type paraeter for operation over other fields?
pub(crate) trait ImmediateBinaryOperation: BinaryOperation<BinaryField16b> {
    // TODO: Add some trick to implement new only once
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
        let src_val = BinaryField32b::new(
            interpreter
                .vrom
                .get(BinaryField32b::new(interpreter.fp) + src),
        );
        let dst_val = Self::operation(src_val, imm.into());
        let event = Self::new(
            interpreter.timestamp,
            interpreter.pc,
            interpreter.fp,
            dst.val(),
            dst_val.clone().val(),
            src.val(),
            src_val.val(),
            imm.into(),
        );
        interpreter
            .vrom
            .set(BinaryField32b::new(interpreter.fp) + dst, dst_val.val());
        // The instruction is over two rows in the PROM.
        interpreter.incr_pc();
        event
    }
}

pub(crate) trait NonImmediateBinaryOperation: BinaryOperation<BinaryField32b> {
    // TODO: Add some trick to implement new only once
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
        let src1_val = interpreter
            .vrom
            .get(BinaryField32b::new(interpreter.fp) + src1);
        let src2_val = interpreter
            .vrom
            .get(BinaryField32b::new(interpreter.fp) + src2);
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
            .set(BinaryField32b::new(interpreter.fp) + dst, dst_val.val());
        // The instruction is over two rows in the PROM.
        interpreter.incr_pc();
        event
    }
}

#[macro_export]
macro_rules! impl_immediate_binary_operation {
    ($t:ty) => {
        impl crate::event::ImmediateBinaryOperation for $t {
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
                    src_val: src_val,
                    imm: imm,
                }
            }
        }
    };
}

#[macro_export]
macro_rules! impl_non_immediate_binary_operation {
    ($t:ty) => {
        impl crate::event::NonImmediateBinaryOperation for $t {
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
macro_rules! impl_event_for_binary_operation {
    ($t:ty) => {
        impl Event for $t {
            fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
                assert_eq!(
                    self.dst_val,
                    Self::operation(
                        BinaryField32b::new(self.src_val),
                        BinaryField16b::new(self.imm)
                    )
                    .val()
                );
                fire_non_jump_event!(self, channels);
            }
        }
    };
}

#[macro_export]
macro_rules! impl_event_for_binary_operation_32b {
    ($t:ty) => {
        impl Event for $t {
            fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
                assert_eq!(
                    self.dst_val,
                    Self::operation(
                        BinaryField32b::new(self.src1_val),
                        BinaryField32b::new(self.src2_val)
                    )
                    .val()
                );
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
            $intrp.pc * crate::emulator::G,
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
