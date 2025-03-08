use binius_field::{BinaryField16b, BinaryField32b};

use crate::{
    emulator::{Interpreter, InterpreterChannels, InterpreterTables},
    event::Event,
    fire_non_jump_event,
};

/// Event for MVV.W.
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

// TODO: this is a 4-byte move instruction. So it needs to be updated once we
// have multi-granularity.
impl MVVWEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
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
        let dst_addr = interpreter.vrom.get_u32(fp ^ dst.val() as u32);
        let src_val = interpreter.vrom.get_u32(fp ^ src.val() as u32);
        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;

        interpreter
            .vrom
            .set_u32(dst_addr ^ offset.val() as u32, src_val);
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

/// Event for MVV.L.
#[derive(Debug, Clone)]
pub(crate) struct MVVLEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    dst_addr: u32,
    src: u16,
    src_val: u128,
    offset: u16,
}

impl MVVLEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        dst_addr: u32,
        src: u16,
        src_val: u128,
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
        let dst_addr = interpreter.vrom.get_u32(fp ^ dst.val() as u32);
        let src_val = interpreter.vrom.get_u128(fp ^ src.val() as u32);
        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;

        interpreter
            .vrom
            .set_u128(dst_addr ^ offset.val() as u32, src_val);
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

impl Event for MVVLEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        fire_non_jump_event!(self, channels);
    }
}

/// Event for MVI.H.
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

// TODO: this is a 2-byte move instruction, which sets a 4 byte address to imm
// zero-extended. So it needs to be updated once we have multi-granularity.
impl MVIHEvent {
    pub const fn new(
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
        let dst_addr = interpreter.vrom.get_u32(fp ^ dst.val() as u32);
        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;

        interpreter
            .vrom
            .set_u32(dst_addr ^ offset.val() as u32, imm.val() as u32);
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
        fire_non_jump_event!(self, channels);
    }
}

// Event for LDI.
#[derive(Debug, Clone)]
pub(crate) struct LDIEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    imm: u32,
}

impl LDIEvent {
    pub const fn new(pc: BinaryField32b, fp: u32, timestamp: u32, dst: u16, imm: u32) -> Self {
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
        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;

        interpreter.vrom.set_u32(fp ^ dst.val() as u32, imm.val());
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
