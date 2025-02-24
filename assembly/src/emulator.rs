use std::{
    collections::HashMap,
    hash::Hash,
    ops::{Index, IndexMut},
};

#[derive(Debug, Default)]
pub struct Channel<T> {
    net_multiplicities: HashMap<T, isize>,
}

#[derive(Debug, Default)]
pub enum Opcode {
    #[default]
    Bnz = 0,
    Xori = 1,
}

#[derive(Debug, Default)]
struct Interpreter {
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

#[derive(Debug, Default)]
struct Instruction {
    opcode: Opcode,
    src1: u32,
    src2: u32,
    dst: u32,
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

    pub fn run(&mut self) -> Result<ZCrayTrace, InterpreterError> {
        let mut trace = ZCrayTrace::default();
        while let Some(_) = self.step(&mut trace)? {
            // Do nothing
        }
        Ok(trace)
    }

    pub fn step(&mut self, trace: &mut ZCrayTrace) -> Result<Option<()>, InterpreterError> {
        let Instruction {
            opcode,
            src1,
            src2,
            dst,
        } = &self.prom[self.pc as usize];
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
        }
        self.timestamp += 1;
        Ok(Some(()))
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
}

impl ZCrayTrace {
    fn generate(prom: ProgramRom) -> Result<Self, InterpreterError> {
        let mut interpreter = Interpreter::new(prom);

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
}
