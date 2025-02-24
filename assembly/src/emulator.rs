use std::{
    collections::HashMap,
    hash::Hash,
    ops::{Index, IndexMut},
};

use crate::{
    sli::{ShiftKind, SliTrace},
    utils::RetTrace,
};

#[derive(Debug, Default)]
pub struct Channel<T> {
    net_multiplicities: HashMap<T, isize>,
}

pub(crate) type StateChannelInput = (u16, u16, u16); // (PC, FP, TIMESTAMP)
pub(crate) type ProgramChannelInput = (u16, u32); // (PC, OPCODE)

#[derive(Debug, Default)]
pub enum Opcode {
    #[default]
    Bnz = 0,
    Xori = 1,
    Srli = 2,
    Slli = 3,
    Ret = 4,
}

#[derive(Debug, Default)]
pub(crate) struct Interpreter {
    pc: u16,
    fp: u16,
    timestamp: u16,
    prom: ProgramRom,
    vrom: ValueRom,
}

#[derive(Debug, Default)]
struct ValueRom(Vec<u32>);

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

impl ValueRom {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn extend(&mut self, slice: &[u32]) {
        self.0.extend(slice);
    }
}

#[derive(Debug, Default)]
struct ProgramRom(Vec<Instruction>);

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

impl ProgramRom {
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[derive(Debug, Default)]
struct Instruction {
    opcode: Opcode,
    src1: u32,
    src2: u32,
    dst: u32,
}

impl Instruction {
    fn new(opcode: Opcode, src1: u32, src2: u32, dst: u32) -> Self {
        Self {
            opcode,
            src1,
            src2,
            dst,
        }
    }
}

#[derive(Debug, Default)]
struct InterpreterError;

impl Interpreter {
    pub fn new(prom: ProgramRom) -> Self {
        Self {
            pc: 0,
            fp: 0,
            timestamp: 0,
            prom,
            vrom: ValueRom::default(),
        }
    }

    pub fn new_with_vrom(prom: ProgramRom, vrom: ValueRom) -> Self {
        Self {
            pc: 1,
            fp: 0,
            timestamp: 0,
            prom,
            vrom,
        }
    }

    pub(crate) fn get_pc(&self) -> u16 {
        self.pc
    }

    pub(crate) fn set_pc(&mut self, pc: u16) {
        self.pc = pc;
    }

    pub(crate) fn get_fp(&self) -> u16 {
        self.fp
    }

    pub(crate) fn set_fp(&mut self, fp: u16) {
        self.fp = fp;
    }

    pub(crate) fn get_timestamp(&self) -> u16 {
        self.timestamp
    }

    pub(crate) fn set_timestamp(&mut self, timestamp: u16) {
        self.timestamp = timestamp;
    }

    pub(crate) fn get_vrom_index(&self, index: usize) -> u32 {
        self.vrom[index]
    }

    pub(crate) fn get_vrom_size(&self) -> usize {
        self.vrom.len()
    }

    pub(crate) fn extend_size(&mut self, slice: &[u32]) {
        self.vrom.extend(slice);
    }

    pub(crate) fn get_prom_index(&self, index: usize) -> &Instruction {
        &self.prom[index]
    }

    pub(crate) fn set_vrom_index(&mut self, index: usize, val: u32) {
        self.vrom[index] = val;
    }

    pub fn run(&mut self) -> Result<ZCrayTrace, InterpreterError> {
        let mut trace = ZCrayTrace::default();
        while let Some(_) = self.step(&mut trace)? {
            if self.pc == 0 {
                return Ok(trace);
            }
        }
        Ok(trace)
    }

    pub fn step(&mut self, trace: &mut ZCrayTrace) -> Result<Option<()>, InterpreterError> {
        let Instruction {
            opcode,
            src1,
            src2,
            dst,
        } = &self.prom[self.pc as usize - 1];
        match opcode {
            Opcode::Bnz => {
                let cond = self.vrom[self.fp as usize + *src1 as usize];
                if cond != 0 {
                    self.pc = *src2 as u16;
                } else {
                    self.pc += 1;
                }
                trace.bnz.push(BnzEvent {
                    timestamp: self.timestamp,
                    pc: self.pc,
                    fp: self.fp,
                    cond: *src1 as u16,
                    con_val: cond,
                    target: *src2,
                });
            }
            Opcode::Xori => {
                let src1_val = self.vrom[self.fp as usize + *src1 as usize];
                let imm = *src2;
                let dst_val = src1_val ^ imm;
                self.vrom[self.fp as usize + *dst as usize] = dst_val;
                self.pc += 1;
                trace.xori.push(XoriEvent {
                    timestamp: self.timestamp,
                    pc: self.pc,
                    fp: self.fp,
                    dst: *dst as u16,
                    dst_val,
                    src1: *src1 as u16,
                    src1_val,
                    target: imm,
                    imm,
                });
            }
            Opcode::Slli => self.generate_slli(trace, *dst, *src1, *src2),
            Opcode::Srli => self.generate_srli(trace, *dst, *src1, *src2),
            Opcode::Ret => self.generate_ret(trace),
        }
        self.timestamp += 1;
        Ok(Some(()))
    }

    fn generate_ret(&mut self, trace: &mut ZCrayTrace) {
        let new_ret_event = RetTrace::generate_event(self);
        trace.ret.push_event(new_ret_event);
    }

    fn generate_slli(&mut self, trace: &mut ZCrayTrace, dst: u32, src: u32, imm: u32) {
        // let new_shift_event = SliEventStruct::new(&self, dst, src, imm, ShiftKind::Left);
        // new_shift_event.apply_event(self);
        let new_shift_event = SliTrace::generate_event(self, dst, src, imm, ShiftKind::Left);
        trace.shift.push_event(new_shift_event);
    }
    fn generate_srli(&mut self, trace: &mut ZCrayTrace, dst: u32, src: u32, imm: u32) {
        let new_shift_event = SliTrace::generate_event(self, dst, src, imm, ShiftKind::Right);
        trace.shift.push_event(new_shift_event);
    }
}

impl<T: Hash + Eq> Channel<T> {
    pub fn push(&mut self, val: T) {
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

    pub fn pull(&mut self, val: T) {
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

    pub fn is_balanced(&self) -> bool {
        self.net_multiplicities.is_empty()
    }
}

#[derive(Debug, Default, Clone)]
struct BnzEvent {
    timestamp: u16,
    pc: u16,
    fp: u16,
    cond: u16,
    con_val: u32,
    target: u32,
}

impl BnzEvent {
    fn fire(&self, prom_chan: &mut Channel<u32>, vrom_chan: &mut Channel<u32>) {
        unimplemented!();
    }
}

#[derive(Debug, Default, Clone)]
struct XoriEvent {
    timestamp: u16,
    pc: u16,
    fp: u16,
    dst: u16,
    dst_val: u32,
    src1: u16,
    src1_val: u32,
    target: u32,
    imm: u32,
}

impl XoriEvent {
    fn fire(&self, prom_chan: &mut Channel<u32>, vrom_chan: &mut Channel<u32>) {
        unimplemented!();
    }
}

#[derive(Debug, Default)]
struct ZCrayTrace {
    bnz: Vec<BnzEvent>,
    xori: Vec<XoriEvent>,
    shift: SliTrace,
    ret: RetTrace,
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
        let trace = ZCrayTrace::generate(ProgramRom(vec![Instruction {
            opcode: Opcode::Bnz,
            src1: 0,
            src2: 0,
            dst: 0,
        }]))
        .expect("Ocuh!");
        trace.validate();
    }

    #[test]
    fn test_sli_ret() {
        // let prom = vec![[0; 4], [0x1b, 3, 2, 5], [0x1c, 5, 4, 7], [0; 4]];
        let instructions = vec![
            Instruction::new(Opcode::Slli, 2, 5, 3),
            Instruction::new(Opcode::Srli, 4, 7, 5),
            Instruction::new(Opcode::Ret, 0, 0, 0),
        ];
        let prom = ProgramRom(instructions);
        let vrom = ValueRom(vec![0, 0, 2, 0, 3]);
        let traces = ZCrayTrace::generate_with_vrom(prom, vrom);
        println!("final trace {:?}", traces);
    }
}
