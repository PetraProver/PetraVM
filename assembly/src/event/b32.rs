use binius_field::{BinaryField16b, BinaryField1b, BinaryField32b, ExtensionField};

use crate::{
    emulator::{InterpreterChannels, InterpreterTables},
    fire_non_jump_event, impl_event_for_binary_operation, impl_immediate_binary_operation,
    impl_left_right_output_for_imm_bin_op,
};

use super::{BinaryOperation, Event, ImmediateBinaryOperation};

#[derive(Debug, Default, Clone)]
pub(crate) struct XoriEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    dst: u16,
    dst_val: u32,
    src: u16,
    src_val: u32,
    imm: u16,
}

impl BinaryOperation for XoriEvent {
    fn operation(val: BinaryField32b, imm: BinaryField16b) -> BinaryField32b {
        val + imm
    }
}

impl_immediate_binary_operation!(XoriEvent);

impl_event_for_binary_operation!(XoriEvent);

#[derive(Debug, Default, Clone)]
pub(crate) struct AndiEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    dst: u16,
    dst_val: u32,
    src: u16,
    src_val: u32,
    imm: u16,
}

impl BinaryOperation for AndiEvent {
    fn operation(val: BinaryField32b, imm: BinaryField16b) -> BinaryField32b {
        BinaryField32b::new(val.val() & imm.val() as u32)
    }
}

impl_immediate_binary_operation!(AndiEvent);

impl_event_for_binary_operation!(AndiEvent);

#[derive(Debug, Default, Clone)]
pub(crate) struct B32MuliEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    dst: u16,
    dst_val: u32,
    src: u16,
    src_val: u32,
    imm: u16,
}

impl_immediate_binary_operation!(B32MuliEvent);

impl BinaryOperation for B32MuliEvent {
    fn operation(val: BinaryField32b, imm: BinaryField16b) -> BinaryField32b {
        val * imm
    }
}

impl_event_for_binary_operation!(B32MuliEvent);
