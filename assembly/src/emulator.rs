use std::{
    collections::HashMap,
    hash::Hash,
    ops::{Index, IndexMut},
};

use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::event::{
    b32::{AndiEvent, XoriEvent},
    branch::BnzEvent,
    call::TailiEvent,
    integer_ops::{Add32Event, Add64Event, AddiEvent, MuliEvent},
    mv::MVVWEvent,
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
    pub state_channel: StateChannel,
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
    pub vrom_table_32: VromTable32,
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
    Andi = 0x03,
    Srli = 0x04,
    Slli = 0x05,
    Addi = 0x06,
    Muli = 0x07,
    Ret = 0x08,
    Taili = 0x09,
    MVVW = 0x0a,
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

    pub(crate) fn set(&mut self, index: usize, value: u32) {
        if index >= self.len() {
            self.extend(&vec![0; index + 1 - self.len()]);
        }

        self[index] = value;
    }
    pub(crate) fn get(&self, index: usize) -> u32 {
        assert!(
            index < self.len(),
            "Value read in the VROM was never written before."
        );

        self[index]
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
            Opcode::Addi => self.generate_addi(trace),
            Opcode::Muli => self.generate_muli(trace),
            Opcode::Ret => self.generate_ret(trace),
            Opcode::Taili => self.generate_taili(trace),
            Opcode::Andi => self.generate_andi(trace),
            Opcode::MVVW => self.generate_mvv(trace),
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
        let new_taili_event = TailiEvent::generate_event(self, target as u16, next_fp as u16);
        trace.taili.push(new_taili_event);
    }

    fn generate_andi(&mut self, trace: &mut ZCrayTrace) {
        let [_, dst, src, imm] = self.prom[self.pc as usize - 1];
        let new_andi_event = AndiEvent::generate_event(self, dst as u16, src as u16, imm);
        trace.andi.push(new_andi_event);
    }

    fn generate_muli(&mut self, trace: &mut ZCrayTrace) {
        let [_, dst, src, imm] = self.prom[self.pc as usize - 1];
        let new_muli_event = MuliEvent::generate_event(self, dst, src, imm);
        let aux = new_muli_event.aux;
        let sum = new_muli_event.sum;
        let interm_sum = new_muli_event.interm_sum;

        // This is to check sum[0] = aux[0] + aux[1]
        trace.add64.push(Add64Event::generate_event(
            self,
            aux[0] as u64,
            aux[1] as u64,
        ));
        for i in 1..3 {
            trace.add64.push(Add64Event::generate_event(
                self,
                aux[2 * i] as u64,
                aux[2 * i + 1] as u64,
            ));
            trace
                .add64
                .push(Add64Event::generate_event(self, sum[i - 1], interm_sum[i]));
        }
        trace.muli.push(new_muli_event);
    }

    fn generate_addi(&mut self, trace: &mut ZCrayTrace) {
        let [_, dst, src, imm] = self.prom[self.pc as usize - 1];
        let new_addi_event = AddiEvent::generate_event(self, dst, src, imm);
        trace.add32.push(Add32Event::generate_event(
            self,
            new_addi_event.src_val,
            imm,
        ));
        trace.addi.push(new_addi_event);
    }

    fn generate_mvv(&mut self, trace: &mut ZCrayTrace) {
        let [_, dst, offset, src] = self.prom[self.pc as usize - 1];
        let new_mvvw_event = MVVWEvent::generate_event(self, dst as u16, offset as u16, src as u16);
        trace.mvvw.push(new_mvvw_event);
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
    addi: Vec<AddiEvent>,
    add32: Vec<Add32Event>,
    add64: Vec<Add64Event>,
    muli: Vec<MuliEvent>,
    taili: Vec<TailiEvent>,
    ret: Vec<RetEvent>,
    mvvw: Vec<MVVWEvent>,
    vrom: ValueRom,
}

struct BoundaryValues {
    final_pc: u16,
    final_fp: u16,
    timestamp: u16,
}

impl ZCrayTrace {
    fn generate(prom: ProgramRom) -> Result<(Self, BoundaryValues), InterpreterError> {
        let mut interpreter = Interpreter::new(prom);

        let mut trace = interpreter.run()?;
        trace.vrom = interpreter.vrom;

        let boundary_values = BoundaryValues {
            final_pc: interpreter.pc,
            final_fp: interpreter.fp,
            timestamp: interpreter.timestamp,
        };

        Ok((trace, boundary_values))
    }

    fn generate_with_vrom(
        prom: ProgramRom,
        vrom: ValueRom,
    ) -> Result<(Self, BoundaryValues), InterpreterError> {
        let mut interpreter = Interpreter::new_with_vrom(prom, vrom);

        let mut trace = interpreter.run()?;
        trace.vrom = interpreter.vrom;

        let boundary_values = BoundaryValues {
            final_pc: interpreter.pc,
            final_fp: interpreter.fp,
            timestamp: interpreter.timestamp,
        };
        Ok((trace, boundary_values))
    }

    fn validate(&self, boundary_values: BoundaryValues) {
        let mut channels = InterpreterChannels::default();

        let vrom_table_32 = self
            .vrom
            .0
            .iter()
            .enumerate()
            .map(|(i, &elem)| (i as u32, elem))
            .collect();

        let tables = InterpreterTables { vrom_table_32 };

        // Initial boundary push: PC = 1, FP = 0, TIMESTAMP = 0.
        channels.state_channel.push((1, 0, 0));
        // Final boundary pull.
        channels.state_channel.pull((
            boundary_values.final_pc,
            boundary_values.final_fp,
            boundary_values.timestamp,
        ));

        self.bnz
            .iter()
            .for_each(|event| event.fire(&mut channels, &tables));

        self.xori
            .iter()
            .for_each(|event| event.fire(&mut channels, &tables));

        self.andi
            .iter()
            .for_each(|event| event.fire(&mut channels, &tables));

        self.shift
            .iter()
            .for_each(|event| event.fire(&mut channels, &tables));

        self.addi
            .iter()
            .for_each(|event| event.fire(&mut channels, &tables));

        self.muli
            .iter()
            .for_each(|event| event.fire(&mut channels, &tables));

        self.taili
            .iter()
            .for_each(|event| event.fire(&mut channels, &tables));

        self.ret
            .iter()
            .for_each(|event| event.fire(&mut channels, &tables));

        self.mvvw
            .iter()
            .for_each(|event| event.fire(&mut channels, &tables));

        assert!(channels.state_channel.is_balanced());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zcray() {
        let (trace, boundary_values) =
            ZCrayTrace::generate(ProgramRom(vec![[Opcode::Ret as u32, 0, 0, 0]])).expect("Ouch!");
        trace.validate(boundary_values);
    }

    #[test]
    fn test_sli_ret() {
        // let prom = vec![[0; 4], [0x1b, 3, 2, 5], [0x1c, 5, 4, 7], [0; 4]];
        let instructions = vec![
            [Opcode::Slli as u32, 3, 2, 5],
            [Opcode::Srli as u32, 5, 4, 7],
            [Opcode::Ret as u32, 0, 0, 0],
        ];
        let prom = ProgramRom(instructions);
        let vrom = ValueRom(vec![0, 0, 2, 0, 3]);
        let (traces, _) =
            ZCrayTrace::generate_with_vrom(prom, vrom).expect("Trace generation should not fail.");
        let shifts = vec![
            SliEvent::new(1, 0, 0, 3, 64, 2, 2, 5, ShiftKind::Left),
            SliEvent::new(2, 0, 1, 5, 0, 4, 3, 7, ShiftKind::Right),
        ];

        let ret = RetEvent {
            pc: 3,
            fp: 0,
            timestamp: 2,
            fp_0_val: 0,
            fp_1_val: 0,
        };

        assert_eq!(traces.shift, shifts);
        assert_eq!(traces.ret, vec![ret]);
    }

    #[test]
    fn test_compiled_collatz() {
        // collatz:
        //  ;; Frame:
        // 	;; Slot @0: Return PC
        // 	;; Slot @1: Return FP
        // 	;; Slot @2: Arg: n
        //  ;; Slot @3: Return value
        // 	;; Slot @4: Local: n == 1
        // 	;; Slot @5: Local: n % 2
        // 	;; Slot @6: Local: 3*n
        //  ;; Slot @7: Local: n >> 2 or 3*n + 1
        // 	;; Slot @8: ND Local: Next FP

        // 	;; Branch to recursion label if value in slot 2 is not 1
        // 	XORI @4, @2, #1G
        // 	BNZ case_recurse, @4 ;; branch if n == 1
        // 	XORI @3, @2, #0G
        // 	RET

        // case_recurse:
        // 	ANDI @5, @2, #1 ;; n % 2 is & 0x00..01
        //  BNZ case_odd, @5 ;; branch if n % 2 == 0u32

        // 	;; case even
        //  ;; n >> 1
        // 	SRLI @7, @2, #1
        //  MVV.W @8[2], @7
        //  MVV.W @8[3], @3
        //  TAILI collatz, @8

        // case_odd:
        // 	MULI @6, @2, #3
        // 	ADDI @7, @6, #1
        //  MVV.W @8[2], @7
        //  MVV.W @8[3], @3
        // 	TAILI collatz, @8

        // labels
        let collatz = 1;
        let case_recurse = 5;
        let case_odd = 11;
        let next_fp_offset = 8;
        let next_fp = 9;
        let instructions = vec![
            // collatz:
            [Opcode::Xori as u32, 4, 2, 1],           //  1: XORI 4 2 1
            [Opcode::Bnz as u32, 4, case_recurse, 0], //  2: BNZ 4 case_recurse
            // case_return:
            [Opcode::Xori as u32, 3, 2, 0], //  3: XORI 3 2 0
            [Opcode::Ret as u32, 0, 0, 0],  //  4: RET
            // case_recurse:
            [Opcode::Andi as u32, 5, 2, 1],       //  5: ANDI 5 2 1
            [Opcode::Bnz as u32, 5, case_odd, 0], //  6: BNZ 5 case_odd 0 0
            // case_even:
            [Opcode::Srli as u32, 7, 2, 1],        //  7: SRLI 7 2 1
            [Opcode::MVVW as u32, 8, 2, 7],        //  8: MVV.W @8[2], @7
            [Opcode::MVVW as u32, 8, 3, 3],        //  9: MVV.W @8[3], @3
            [Opcode::Taili as u32, collatz, 8, 0], // 10: TAILI collatz 8 0
            // case_odd:
            [Opcode::Muli as u32, 6, 2, 3],        //  11: MULI 6 2 3
            [Opcode::Addi as u32, 7, 6, 1],        //  12: ADDI 7 6 1
            [Opcode::MVVW as u32, 8, 2, 7],        //  13: MVV.W @8[2], @7
            [Opcode::MVVW as u32, 8, 3, 3],        //  14: MVV.W @8[3], @3
            [Opcode::Taili as u32, collatz, 8, 0], //  15: TAILI collatz 8 0
        ];
        let initial_val = 3999;
        let (expected_evens, expected_odds) = collatz_orbits(initial_val);
        let nb_frames = expected_evens.len() + expected_odds.len();
        let prom = ProgramRom(instructions);
        // return PC = 0, return FP = 0, n = 3999
        let mut vrom = ValueRom(vec![0, 0, initial_val]);
        for i in 0..nb_frames {
            vrom.set(i * next_fp + next_fp_offset, ((i + 1) * next_fp) as u32);
        }

        let (traces, _) =
            ZCrayTrace::generate_with_vrom(prom, vrom).expect("Trace generation should not fail.");

        assert!(traces.shift.len() == expected_evens.len()); // There are 4 even cases.
        for i in 0..expected_evens.len() {
            assert!(traces.shift[i].src_val == expected_evens[i]);
        }
        assert!(traces.muli.len() == expected_odds.len()); // There is 1 odd case.
        for i in 0..expected_odds.len() {
            assert!(traces.muli[i].src_val == expected_odds[i]);
        }
    }

    fn collatz_orbits(initial_val: u32) -> (Vec<u32>, Vec<u32>) {
        let mut cur_value = initial_val;
        let mut evens = vec![];
        let mut odds = vec![];
        while cur_value != 1 {
            if cur_value % 2 == 0 {
                evens.push(cur_value);
                cur_value /= 2;
            } else {
                odds.push(cur_value);
                cur_value = 3 * cur_value + 1;
            }
        }
        (evens, odds)
    }
}
