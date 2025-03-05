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

pub(crate) trait BinaryOperation: Sized {
    fn operation(val: BinaryField32b, imm: BinaryField32b) -> BinaryField32b;
}

// TODO: Add type paraeter for operation over other fields?
pub(crate) trait ImmediateBinaryOperation: BinaryOperation {
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
        interpreter.incr_pc();
        event
    }
}

pub(crate) trait NonImmediateBinaryOperation: BinaryOperation {
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
        println!("getting src1");
        let src1_val = interpreter
            .vrom
            .get(BinaryField32b::new(interpreter.fp) + src1);
        println!("get src1 val {:?}", src1_val);
        let src2_val = interpreter
            .vrom
            .get(BinaryField32b::new(interpreter.fp) + src2);
        println!("get src2 val {:?}", src2_val);
        let dst_val = Self::operation(BinaryField32b::new(src1_val), BinaryField32b::new(src2_val));
        println!("dst_val {:?}", dst_val);
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
        interpreter.incr_pc();
        event
    }
}
