use std::{
    collections::HashMap,
    hash::Hash,
    ops::{Index, IndexMut},
};

use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::event::{
    b32::{AndiEvent, XoriEvent},
    branch::BnzEvent,
    call::TailIEvent,
    ret::RetEvent,
    sli::{ShiftKind, SliEvent},
    Event,
    ImmediateBinaryOperation, // Add the import for RetEvent
};

#[derive(Debug, Default)]
pub struct Channel<T> {
    net_multiplicities: HashMap<T, isize>,
}

type StateChannelInput = (u16, u16, u16); // PC, FP, Timestamp
pub(crate) type StateChannel = Channel<StateChannelInput>;

#[derive(Debug, Clone, Copy, Default, TryFromPrimitive, IntoPrimitive, PartialEq, Eq)]
#[repr(u32)]
pub enum Opcode {
    #[default]
    Bnz = 0x01,
    Xori = 0x02,
    Srli = 0x03,
    Slli = 0x04,
    Ret = 0x05,
    Taili = 0x06,
    Andi = 0x07,
    Muli = 0x08,
    Addi = 0x09,
}

#[derive(Debug, Default)]
pub(crate) struct Interpreter {
    pub(crate) pc: u16,
    pub(crate) fp: u16,
    pub(crate) timestamp: u16,
    pub(crate) prom: ProgramRom,
    pub(crate) vrom: ValueRom,
}

#[derive(Debug, Default)]
pub(crate) struct ValueRom(Vec<u32>);

impl Index<usize> for ValueRom {
    type Output = u32;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index] // Forward indexing to the inner vector
    }
}

impl IndexMut<usize> for ValueRom {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index] // Forward indexing to the inner vector
    }
}

#[derive(Debug, Default)]
pub struct ProgramRom(Vec<Instruction>);

impl Index<usize> for ProgramRom {
    type Output = Instruction;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index] // Forward indexing to the inner vector
    }
}

impl IndexMut<usize> for ProgramRom {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index] // Forward indexing to the inner vector
    }
}

impl ValueRom {
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn extend(&mut self, slice: &[u32]) {
        self.0.extend(slice);
    }
}

type Instruction = [u32; 4];

#[derive(Debug)]
pub(crate) enum InterpreterError {
    InvalidOpcode,
}

impl Interpreter {
    pub(crate) fn new(prom: ProgramRom) -> Self {
        Self {
            pc: 1,
            fp: 0,
            timestamp: 0,
            prom,
            vrom: ValueRom::default(),
        }
    }

    pub(crate) fn new_with_vrom(prom: ProgramRom, vrom: ValueRom) -> Self {
        Self {
            pc: 1,
            fp: 0,
            timestamp: 0,
            prom,
            vrom,
        }
    }

    pub(crate) fn vrom_size(&self) -> usize {
        self.vrom.0.len()
    }

    pub(crate) fn extend_vrom(&mut self, slice: &[u32]) {
        self.vrom.0.extend(slice);
    }

    pub(crate) fn is_halted(&self) -> bool {
        self.pc == 0
    }

    pub fn run(&mut self) -> Result<ZCrayTrace, InterpreterError> {
        let mut trace = ZCrayTrace::default();
        while let Some(_) = self.step(&mut trace)? {
            if self.is_halted() {
                return Ok(trace);
            }
        }
        Ok(trace)
    }

    pub fn step(&mut self, trace: &mut ZCrayTrace) -> Result<Option<()>, InterpreterError> {
        let [opcode, ..] = self.prom[self.pc as usize - 1];
        let opcode = Opcode::try_from(opcode).map_err(|_| InterpreterError::InvalidOpcode)?;
        match opcode {
            Opcode::Bnz => self.generate_bnz(trace),
            Opcode::Xori => self.generate_xori(trace),
            Opcode::Slli => self.generate_slli(trace),
            Opcode::Srli => self.generate_srli(trace),
            Opcode::Ret => self.generate_ret(trace),
            Opcode::Taili => self.generate_taili(trace),
            Opcode::Andi => self.generate_andi(trace),
            Opcode::Muli => todo!(),
            Opcode::Addi => todo!(),
        }
        self.timestamp += 1;
        Ok(Some(()))
    }

    fn generate_bnz(&mut self, trace: &mut ZCrayTrace) {
        let [_, cond, target, _] = self.prom[self.pc as usize - 1];
        let new_bnz_event = BnzEvent::generate_event(self, cond as u16, target as u16);
        trace.bnz.push(new_bnz_event);
    }

    fn generate_xori(&mut self, trace: &mut ZCrayTrace) {
        let [_, dst, src, imm] = self.prom[self.pc as usize - 1];
        let new_xori_event = XoriEvent::generate_event(self, dst as u16, src as u16, imm);
        trace.xori.push(new_xori_event);
    }

    fn generate_ret(&mut self, trace: &mut ZCrayTrace) {
        let new_ret_event = RetEvent::generate_event(self);
        trace.ret.push(new_ret_event);
    }

    fn generate_slli(&mut self, trace: &mut ZCrayTrace) {
        // let new_shift_event = SliEventStruct::new(&self, dst, src, imm, ShiftKind::Left);
        // new_shift_event.apply_event(self);
        let [_, dst, src, imm] = self.prom[self.pc as usize - 1];
        let new_shift_event = SliEvent::generate_event(self, dst, src, imm, ShiftKind::Left);
        trace.shift.push(new_shift_event);
    }
    fn generate_srli(&mut self, trace: &mut ZCrayTrace) {
        let [_, dst, src, imm] = self.prom[self.pc as usize - 1];
        let new_shift_event = SliEvent::generate_event(self, dst, src, imm, ShiftKind::Right);
        trace.shift.push(new_shift_event);
    }

    fn generate_taili(&mut self, trace: &mut ZCrayTrace) {
        let [_, target, next_fp, _] = self.prom[self.pc as usize - 1];
        let new_taili_event = TailIEvent::generate_event(self, target as u16, next_fp as u16);
        trace.taili.push(new_taili_event);
    }

    fn generate_andi(&mut self, trace: &mut ZCrayTrace) {
        let [_, dst, src, imm] = self.prom[self.pc as usize - 1];
        let new_andi_event = AndiEvent::generate_event(self, dst as u16, src as u16, imm);
        trace.andi.push(new_andi_event);
    }
}

impl<T: Hash + Eq> Channel<T> {
    pub(crate) fn push(&mut self, val: T) {
        match self.net_multiplicities.get_mut(&val) {
            Some(multiplicity) => {
                *multiplicity += 1;

                // Remove the key if the multiplicity is zero, to improve Debug behavior.
                if *multiplicity == 0 {
                    self.net_multiplicities.remove(&val);
                }
            }
            None => {
                let _ = self.net_multiplicities.insert(val, 1);
            }
        }
    }

    pub(crate) fn pull(&mut self, val: T) {
        match self.net_multiplicities.get_mut(&val) {
            Some(multiplicity) => {
                *multiplicity -= 1;

                // Remove the key if the multiplicity is zero, to improve Debug behavior.
                if *multiplicity == 0 {
                    self.net_multiplicities.remove(&val);
                }
            }
            None => {
                let _ = self.net_multiplicities.insert(val, -1);
            }
        }
    }

    pub(crate) fn is_balanced(&self) -> bool {
        self.net_multiplicities.is_empty()
    }
}

#[derive(Debug, Default)]
pub(crate) struct ZCrayTrace {
    bnz: Vec<BnzEvent>,
    xori: Vec<XoriEvent>,
    andi: Vec<AndiEvent>,
    shift: Vec<SliEvent>,
    ret: Vec<RetEvent>,
    taili: Vec<TailIEvent>,
}

impl ZCrayTrace {
    fn generate(prom: ProgramRom) -> Result<Self, InterpreterError> {
        let mut interpreter = Interpreter::new(prom);

        let trace = interpreter.run()?;

        Ok(trace)
    }

    fn generate_with_vrom(prom: ProgramRom, vrom: ValueRom) -> Result<Self, InterpreterError> {
        let mut interpreter = Interpreter::new_with_vrom(prom, vrom);

        let trace = interpreter.run()?;

        Ok(trace)
    }

    fn validate(&self) {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zcray() {
        let trace = ZCrayTrace::generate(ProgramRom(vec![[0, 0, 0, 0]])).expect("Ocuh!");
        trace.validate();
    }

    #[test]
    fn test_sli_ret() {
        // let prom = vec![[0; 4], [0x1b, 3, 2, 5], [0x1c, 5, 4, 7], [0; 4]];
        let instructions = vec![
            [Opcode::Slli as u32, 1, 2, 3], // SLLI 2 5 3
            [Opcode::Srli as u32, 3, 4, 5], // SRLI 4 7 5
            [Opcode::Ret as u32, 0, 0, 0],  // RET
        ];
        let prom = ProgramRom(instructions);
        let vrom = ValueRom(vec![0, 0, 2, 0, 3]);
        let traces = ZCrayTrace::generate_with_vrom(prom, vrom);
        println!("final trace {:?}", traces);
    }

    #[test]
    fn test_compiled_collatz() {
        //collatz:
        // ;; Frame:
        // 	;; Slot @0: Return PC
        // 	;; Slot @1: Return FP
        // 	;; Slot @2: Arg: n
        //  ;; Slot @3: Return value
        // 	;; Slot @4: Local: n == 1
        // 	;; Slot @5: Local: n % 2
        // 	;; Slot @6: Local: 3*n
        // 	;; Slot @7: ND Local: Next FP
        // 	;; Slot @8: Local: n >> 2 or 3*n + 1

        // 	;; Branch to recursion label if value in slot 2 is not 1
        // 	XORI @5, @2, #1
        // 	BNZ case_recurse, @5 ;; branch if n == 1
        // 	XORI @3 @2 #0
        // 	RET
        // ;;

        // case_recurse:
        // 	ANDI @6, #1 ;; n % 2 is & 0x00..01
        // BNZ case_odd, @6  ;; branch if n % 2 == 0u32

        // 	;; case even
        // ;; n >> 1
        // 	SRLI @8[2], @2, #1
        // TAILI collatz, @8

        // case_odd:
        // 	MULI @7, @2, #3u32
        // 	ADDI @9, @7, #1u32
        // 	TAILI collatz, @8

        //     }

        // labels
        let collatz = 0;
        let case_recurse = 4;
        let case_odd = 8;
        let instructions = vec![
            // collatz:
            [Opcode::Xori as u32, 4, 2, 1],           //  0: XORI 4 2 1
            [Opcode::Bnz as u32, 4, case_recurse, 0], //  1: BNZ 4 case_recurse
            // case_return:
            [Opcode::Xori as u32, 3, 2, 0],           //  2: XORI 3 2 0
            [Opcode::Ret as u32, 0, 0, 0],            //  3: RET
            // case_recurse:
            [Opcode::Andi as u32, 6, 0, 1],       //  4: ANDI 6 0 1
            [Opcode::Bnz as u32, 6, case_odd, 0], //  5: BNZ 6 case_odd 0 0
            // case_even:
            [Opcode::Srli as u32, 8, 2, 1],        //  6: SRLI 8 2 1
            [Opcode::Taili as u32, collatz, 8, 0], // 7: TAILI collatz 8 0
            // case_odd:
            [Opcode::Muli as u32, 7, 2, 3],        //  8: MULI 7 2 3
            [Opcode::Addi as u32, 9, 7, 1],        //  9: ADDI 9 7 1
            [Opcode::Taili as u32, collatz, 8, 0], //  10: TAILI collatz 8 0
        ];
        let initial_val = 3999;
        let prom = ProgramRom(instructions);
        // return PC = 0, return FP = 0, n = 3999
        let vrom = ValueRom(vec![0, 0, initial_val]);
        let traces = ZCrayTrace::generate_with_vrom(prom, vrom);
        println!("final trace {:?}", traces);
    }
}
