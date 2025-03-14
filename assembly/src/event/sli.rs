use binius_field::{BinaryField16b, BinaryField32b, Field};

use crate::{
    event::Event,
    execution::{Interpreter, InterpreterChannels, InterpreterError, InterpreterTables, G},
    ZCrayTrace,
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
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
        kind: ShiftKind,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let src_val = trace.get_vrom_u32(interpreter.fp ^ src.val() as u32)?;
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
        trace.set_vrom_u32(interpreter.fp ^ dst.val() as u32, new_val)?;
        interpreter.incr_pc();

        Ok(SliEvent::new(
            field_pc,
            interpreter.fp,
            timestamp,
            dst.val(),
            new_val,
            src.val(),
            src_val,
            imm.val(),
            kind,
        ))
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

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use binius_field::PackedField;

    use super::*;
    use crate::{code_to_prom, event::ret::RetEvent, memory::Memory, opcodes::Opcode, ValueRom};

    #[test]
    fn test_program_with_sli_ops() {
        let zero = BinaryField16b::zero();
        let shift1_dst = BinaryField16b::new(3);
        let shift1_src = BinaryField16b::new(2);
        let shift1 = BinaryField16b::new(5);

        let shift2_dst = BinaryField16b::new(5);
        let shift2_src = BinaryField16b::new(4);
        let shift2 = BinaryField16b::new(7);

        let instructions = vec![
            [Opcode::Slli.get_field_elt(), shift1_dst, shift1_src, shift1],
            [Opcode::Srli.get_field_elt(), shift2_dst, shift2_src, shift2],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];
        let mut frames = HashMap::new();
        frames.insert(BinaryField32b::ONE, 6);

        let prom = code_to_prom(&instructions, &vec![false; instructions.len()]);

        //  ;; Frame:
        // 	;; Slot @0: Return PC
        // 	;; Slot @1: Return FP
        // 	;; Slot @2: Local: src1
        // 	;; Slot @3: Local: dst1
        // 	;; Slot @4: Local: src2
        //  ;; Slot @5: Local: dst2
        let mut vrom = ValueRom::default();
        vrom.set_u32(0, 0);
        vrom.set_u32(1, 0);
        vrom.set_u32(2, 2u32);
        vrom.set_u32(4, 3u32);

        let memory = Memory::new(prom, vrom);

        let (traces, _) = ZCrayTrace::generate(memory, frames, HashMap::new())
            .expect("Trace generation should not fail.");
        let shifts = vec![
            SliEvent::new(BinaryField32b::ONE, 0, 0, 3, 64, 2, 2, 5, ShiftKind::Left),
            SliEvent::new(G, 0, 1, 5, 0, 4, 3, 7, ShiftKind::Right),
        ];

        let ret = RetEvent {
            pc: G.square(), // PC = 3
            fp: 0,
            timestamp: 2,
            fp_0_val: 0,
            fp_1_val: 0,
        };

        assert_eq!(traces.shift, shifts);
        assert_eq!(traces.ret, vec![ret]);
    }
}
