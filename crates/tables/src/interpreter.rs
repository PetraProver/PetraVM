use std::{collections::HashMap, hash::Hash};

use crate::{
    sli::{ShiftKind, SliEventStruct, SliTrace},
    utils::{Event, RetEvent, RetTrace},
};

#[derive(Debug, Default)]
pub struct Channel<T> {
    net_multiplicities: HashMap<T, isize>,
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

#[derive(Default, Debug)]
pub struct Traces {
    shift_trace: SliTrace,
    ret_trace: RetTrace,
}

impl Traces {
    fn generate(prom: Vec<[u32; 4]>) -> Self {
        let mut interpreter = Interpreter::new(prom, vec![0, 0], 1);
        interpreter.run();
        interpreter.traces
    }

    fn generate_with_vrom(prom: Vec<[u32; 4]>, vrom: Vec<u32>) -> Self {
        let mut interpreter = Interpreter::new(prom, vrom, 1);
        interpreter.run();
        interpreter.traces
    }
}

pub(crate) type StateChannelInput = (u32, u32, u32); // (PC, FP, TIMESTAMP)

pub(crate) type ProgramChannelInput = (u32, u32); // (PC, OPCODE)

#[derive(Default)]
pub struct Interpreter {
    pub timestamp: u32,
    pub pc: u32,
    pub fp: u32,
    pub prom: Vec<[u32; 4]>,
    pub vrom: Vec<u32>,
    pub traces: Traces,
}

impl Interpreter {
    fn new(prom: Vec<[u32; 4]>, vrom: Vec<u32>, pc: u32) -> Self {
        Self {
            timestamp: 1,
            pc,
            prom,
            vrom,
            ..Default::default()
        }
    }

    fn run(&mut self) {
        while let Some(_) = self.step() {}
    }

    fn step(&mut self) -> Option<()> {
        assert!(self.pc < self.prom.len() as u32);
        let [opcode, dst, src1, src2] = self.prom[self.pc as usize];
        match opcode {
            0x00 => self.generate_ret(),
            0x1b => self.generate_slli(dst, src1, src2),
            0x1c => self.generate_srli(dst, src1, src2),
            _ => panic!("Opcode not supported."),
        }
        if self.pc == 0 {
            return None;
        }
        Some(())
    }

    fn generate_ret(&mut self) {
        // let new_ret_event = RetEvent::new(&self);
        // new_ret_event.apply_event(self);
        let new_ret_event = RetTrace::generate_event(self);
        self.traces.ret_trace.push_event(new_ret_event);
    }

    fn generate_slli(&mut self, dst: u32, src: u32, imm: u32) {
        // let new_shift_event = SliEventStruct::new(&self, dst, src, imm, ShiftKind::Left);
        // new_shift_event.apply_event(self);
        let new_shift_event = SliTrace::generate_event(self, dst, src, imm, ShiftKind::Left);
        self.traces.shift_trace.push_event(new_shift_event);
    }
    fn generate_srli(&mut self, dst: u32, src: u32, imm: u32) {
        let new_shift_event = SliTrace::generate_event(self, dst, src, imm, ShiftKind::Right);
        self.traces.shift_trace.push_event(new_shift_event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpreter() {
        let prom = vec![[0; 4], [0x1b, 3, 2, 5], [0x1c, 5, 4, 7], [0; 4]];
        let vrom = vec![0, 0, 2, 0, 3];
        let traces = Traces::generate_with_vrom(prom, vrom);
        println!("final trace {:?}", traces);
    }
}
