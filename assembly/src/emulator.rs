use std::{
    collections::HashMap,
    hash::Hash,
    ops::{Index, IndexMut},
};

use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::event::{
    b32::XoriEvent,
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

type PromChannel = Channel<(u16, u32, u16, u32)>;
type VromChannel = Channel<u32>;
type StateChannel = Channel<(u16, u16, u16)>; // PC, FP, Timestamp

pub struct InterpreterChannels {
    state_channel: StateChannel,
}

impl Default for InterpreterChannels {
    fn default() -> Self {
        InterpreterChannels {
            state_channel: StateChannel::default(),
        }
    }
}

type VromTable32 = HashMap<u32, u32>;
pub struct InterpreterTables {
    vrom_table_32: VromTable32,
}

impl Default for InterpreterTables {
    fn default() -> Self {
        InterpreterTables {
            vrom_table_32: VromTable32::default(),
        }
    }
}

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
        let mut channels = InterpreterChannels::default();
        let mut tables = InterpreterTables::default();

        self.bnz
            .iter()
            .for_each(|event| event.fire(&mut channels, &tables));

        self.xori
            .iter()
            .for_each(|event| event.fire(&mut channels, &tables));
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
            [Opcode::Slli as u32, 2, 5, 3],
            [Opcode::Srli as u32, 4, 7, 5],
            [Opcode::Ret as u32, 0, 0, 0],
        ];
        let prom = ProgramRom(instructions);
        let vrom = ValueRom(vec![0, 0, 2, 0, 3]);
        let traces = ZCrayTrace::generate_with_vrom(prom, vrom);
        println!("final trace {:?}", traces);
    }
}
