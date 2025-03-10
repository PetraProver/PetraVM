use binius_field::{BinaryField16b, BinaryField32b, Field};

use crate::{
    emulator::{Interpreter, InterpreterChannels, InterpreterTables, G},
    event::Event,
};

#[derive(Debug, Clone, PartialEq)]
pub enum ShiftKind {
    Left,
    Right,
}

/// Event for SLLI and SRLI.
///
/// Performs a left or right shift of a target address by an immediate bit
/// count.
///
/// Logic:
///   1. SLLI: FP[dst] = FP[src] << imm
///   1. SRLI: FP[dst] = FP[src] >> imm
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
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
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
        let src_val = interpreter.vrom.get_u32(interpreter.fp ^ src.val() as u32);
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
        interpreter
            .vrom
            .set_u32(interpreter.fp ^ dst.val() as u32, new_val);
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

impl Event for SliEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.pc * G, self.fp, self.timestamp + 1));
    }
}
