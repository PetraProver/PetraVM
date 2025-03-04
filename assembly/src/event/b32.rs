use binius_field::{BinaryField16b, BinaryField1b, BinaryField32b, ExtensionField};

use crate::{emulator::{InterpreterChannels, InterpreterTables}, impl_event_for_non_jump_event};

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

impl ImmediateBinaryOperation for XoriEvent {
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

impl BinaryOperation for XoriEvent {
    fn operation(val: BinaryField32b, imm: BinaryField16b) -> BinaryField32b {
        val + imm
    }
}

impl_event_for_non_jump_event!(XoriEvent);

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

impl ImmediateBinaryOperation for AndiEvent {
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

impl BinaryOperation for AndiEvent {
    fn operation(val: BinaryField32b, imm: BinaryField16b) -> BinaryField32b {
        let imm_32b = BinaryField32b::from(imm);
        let and_bits = <BinaryField32b as ExtensionField<BinaryField1b>>::iter_bases(&val)
            .zip(<BinaryField32b as ExtensionField<BinaryField1b>>::iter_bases(&imm_32b))
            .map(|(b1, b2)| b1 * b2)
            .collect::<Vec<_>>();
        BinaryField32b::from_bases(&and_bits).expect("hello")
    }
}

impl_event_for_non_jump_event!(AndiEvent);

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

impl ImmediateBinaryOperation for B32MuliEvent {
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

impl BinaryOperation for B32MuliEvent {
    fn operation(val: BinaryField32b, imm: BinaryField16b) -> BinaryField32b {
        val * imm
    }
}

impl_event_for_non_jump_event!(B32MuliEvent);
