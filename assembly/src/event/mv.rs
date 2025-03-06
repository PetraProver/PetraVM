use binius_field::{BinaryField16b, BinaryField32b};

use crate::{
    emulator::{Interpreter, InterpreterChannels, InterpreterTables, G},
    event::Event,
    fire_non_jump_event,
};

// Struture of an event for MVV.W.
#[derive(Debug, Clone)]
pub(crate) struct MVVWEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    dst_addr: u32,
    src: u16,
    src_val: u32,
    offset: u16,
}

// TODO: this is a 4-byte move instruction. So it needs to be updated once we have multi-granularity.
impl MVVWEvent {
    pub fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        dst_addr: u32,
        src: u16,
        src_val: u32,
        offset: u16,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_addr,
            src,
            src_val,
            offset,
        }
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        dst: BinaryField16b,
        offset: BinaryField16b,
        src: BinaryField16b,
    ) -> Self {
        let fp = interpreter.fp;
        let fp_field = BinaryField32b::new(fp);
        let dst_addr = interpreter.vrom.get(fp_field + dst);
        let src_val = interpreter.vrom.get(fp_field + src);
        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;

        interpreter
            .vrom
            .set(BinaryField32b::new(dst_addr) + offset, src_val);
        interpreter.incr_pc();

        Self {
            pc,
            fp,
            timestamp,
            dst: dst.val(),
            dst_addr,
            src: src.val(),
            src_val,
            offset: offset.val(),
        }
    }
}

impl Event for MVVWEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        fire_non_jump_event!(self, channels);
    }
}

// Struture of an event for MVV.W.
#[derive(Debug, Clone)]
pub(crate) struct MVIHEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    dst_addr: u32,
    imm: u16,
    offset: u16,
}

// TODO: this is a 2-byte move instruction, which sets a 4 byte address to imm zero-extended.
// So it needs to be updated once we have multi-granularity.
impl MVIHEvent {
    pub fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        dst_addr: u32,
        imm: u16,
        offset: u16,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_addr,
            imm,
            offset,
        }
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        dst: BinaryField16b,
        offset: BinaryField16b,
        imm: BinaryField16b,
    ) -> Self {
        let fp = interpreter.fp;
        let fp_field = BinaryField32b::new(fp);
        let dst_addr = interpreter.vrom.get(fp_field + dst);
        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;

        interpreter
            .vrom
            .set(BinaryField32b::new(dst_addr) + offset, imm.val() as u32);
        interpreter.incr_pc();

        Self {
            pc,
            fp,
            timestamp,
            dst: dst.val(),
            dst_addr,
            imm: imm.val(),
            offset: offset.val(),
        }
    }
}

impl Event for MVIHEvent {
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.pc * G, self.fp, self.timestamp + 1));
    }
}

// Struture of an event for MVV.W.
#[derive(Debug, Clone)]
pub(crate) struct LDIEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    imm: u32,
}

impl LDIEvent {
    pub fn new(pc: BinaryField32b, fp: u32, timestamp: u32, dst: u16, imm: u32) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            imm,
        }
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        dst: BinaryField16b,
        imm: BinaryField32b,
    ) -> Self {
        let fp = interpreter.fp;
        let fp_field = BinaryField32b::new(fp);
        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;

        interpreter.vrom.set(fp_field + dst, imm.val() as u32);
        interpreter.incr_pc();

        Self {
            pc,
            fp,
            timestamp,
            dst: dst.val(),
            imm: imm.val(),
        }
    }
}

impl Event for LDIEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        fire_non_jump_event!(self, channels);
    }
}
