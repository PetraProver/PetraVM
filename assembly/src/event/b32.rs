use binius_field::{BinaryField16b, BinaryField1b, BinaryField32b, ExtensionField};

use crate::emulator::{InterpreterChannels, InterpreterTables};

use super::{BinaryOperation, Event, ImmediateBinaryOperation, NonImmediateBinaryOperation};

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

impl Event for XoriEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        channels
            .state_channel
            .push((self.pc, self.fp, self.timestamp));
    }
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
    fn operation(val: BinaryField32b, imm: BinaryField32b) -> BinaryField32b {
        val + imm
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct XorEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    dst: u16,
    dst_val: u32,
    src1: u16,
    src1_val: u32,
    src2: u16,
    src2_val: u32,
}

impl Event for XorEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        channels
            .state_channel
            .push((self.pc, self.fp, self.timestamp));
    }
}

impl NonImmediateBinaryOperation for XorEvent {
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

impl BinaryOperation for XorEvent {
    fn operation(val: BinaryField32b, imm: BinaryField32b) -> BinaryField32b {
        val + imm
    }
}

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

impl Event for AndiEvent {
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables) {}
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
    fn operation(val: BinaryField32b, imm: BinaryField32b) -> BinaryField32b {
        let imm_32b = BinaryField32b::from(imm);
        let and_bits = <BinaryField32b as ExtensionField<BinaryField1b>>::iter_bases(&val)
            .zip(<BinaryField32b as ExtensionField<BinaryField1b>>::iter_bases(&imm_32b))
            .map(|(b1, b2)| b1 * b2)
            .collect::<Vec<_>>();
        BinaryField32b::from_bases(&and_bits).expect("hello")
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct B32MuliEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    dst: u16,
    dst_val: u32,
    src: u16,
    src_val: u32,
    imm: u32,
}

impl Event for B32MuliEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        channels
            .state_channel
            .push((self.pc, self.fp, self.timestamp));
    }
}

impl B32MuliEvent {
    fn new(
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

    pub fn generate_event(
        interpreter: &mut crate::emulator::Interpreter,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField32b,
    ) -> Self {
        let src_val = interpreter
            .vrom
            .get(BinaryField32b::new(interpreter.fp) + src);
        let dst_val = Self::operation(BinaryField32b::new(src_val), imm.into());
        let event = Self::new(
            interpreter.timestamp,
            interpreter.pc,
            interpreter.fp,
            dst.val(),
            dst_val.val(),
            src.val(),
            src_val,
            imm.val(),
        );
        interpreter
            .vrom
            .set(BinaryField32b::new(interpreter.fp) + dst, dst_val.val());
        // The instruction is over two rows in the PROM.
        interpreter.incr_pc();
        interpreter.incr_pc();
        event
    }
}

impl BinaryOperation for B32MuliEvent {
    fn operation(val: BinaryField32b, imm: BinaryField32b) -> BinaryField32b {
        val * imm
    }
}
