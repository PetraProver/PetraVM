use std::fmt::Binary;

use binius_field::{BinaryField16b, BinaryField32b, Field};

use crate::{
    emulator::{Interpreter, InterpreterChannels, InterpreterTables, G},
    event::Event,
    fire_non_jump_event,
};

use super::{BinaryOperation, ImmediateBinaryOperation};

#[derive(Debug, Clone, PartialEq)]
pub enum ShiftKind {
    Left,
    Right,
}

// Struture of an event for one of the shifts.
#[derive(Debug, Clone, PartialEq)]
pub struct SliEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    dst_val: u32,
    src: u16,
    pub(crate) src_val: u32,
    shift: u16,
    kind: ShiftKind,
}

impl SliEvent {
    pub fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        dst_val: u32,
        src: u16,
        src_val: u32,
        shift: u16,
        kind: ShiftKind,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_val,
            src,
            src_val,
            shift,
            kind,
        }
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
        kind: ShiftKind,
    ) -> SliEvent {
        let field_fp = BinaryField32b::new(interpreter.fp);
        let src_val = interpreter.vrom.get(field_fp + src);
        let new_val = if imm == BinaryField16b::ZERO || imm >= BinaryField16b::new(32) {
            0
        } else {
            match kind {
                ShiftKind::Left => src_val << imm.val(),
                ShiftKind::Right => src_val >> imm.val(),
            }
        };

        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;
        interpreter.vrom.set(field_fp + dst, new_val);
        interpreter.incr_pc();

        SliEvent::new(
            pc,
            interpreter.fp,
            timestamp,
            dst.val(),
            new_val,
            src.val(),
            src_val,
            imm.val(),
            kind,
        )
    }
}

impl ImmediateBinaryOperation for SliEvent {
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
        Self::new(pc, fp, timestamp, dst, dst_val.into(), src, src_val, imm.into(), ShiftKind::Left)
    }
}

impl BinaryOperation<BinaryField16b> for SliEvent {
    fn operation(val: BinaryField32b, imm: BinaryField16b) -> BinaryField32b {
        BinaryField32b::new(val.val() << imm.val())
    }
}

impl Event for SliEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        assert_eq!(
            self.dst_val,
            match self.kind {
                ShiftKind::Left => self.src_val << self.shift,
                ShiftKind::Right => self.src_val >> self.shift,
            }
        );
        fire_non_jump_event!(self, channels);
    }
}
