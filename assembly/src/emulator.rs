use std::{
    collections::HashMap,
    hash::Hash,
    ops::{Index, IndexMut},
};

use binius_field::{BinaryField, BinaryField16b, BinaryField32b, ExtensionField, Field};
use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::event::{
    b32::{AndiEvent, B32MuliEvent, XoriEvent},
    branch::BnzEvent,
    call::TailiEvent,
    integer_ops::{Add32Event, Add64Event, AddEvent, AddiEvent, MuliEvent},
    mv::MVVWEvent,
    ret::RetEvent,
    sli::{ShiftKind, SliEvent},
    Event,
    ImmediateBinaryOperation, // Add the import for RetEvent
};

pub(crate) const G: BinaryField32b = BinaryField32b::MULTIPLICATIVE_GENERATOR;
#[derive(Debug, Default)]
pub struct Channel<T> {
    net_multiplicities: HashMap<T, isize>,
}

type PromChannel = Channel<(u32, u128)>; // PC, opcode, args (so 64 bits overall).
type VromChannel = Channel<u32>;
type StateChannel = Channel<(BinaryField32b, u32, u32)>; // PC, FP, Timestamp

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
#[repr(u16)]
pub enum Opcode {
    #[default]
    Bnz = 0x01,
    Xori = 0x02,
    Andi = 0x03,
    Srli = 0x04,
    Slli = 0x05,
    Addi = 0x06,
    Add = 0x07,
    Muli = 0x08,
    B32Muli = 0x09,
    Ret = 0x0a,
    Taili = 0x0b,
    MVVW = 0x0c,
}

impl Opcode {
    pub fn get_field_elt(&self) -> BinaryField16b {
        BinaryField16b::new(*self as u16)
    }
}

#[derive(Debug, Default)]
pub(crate) struct Interpreter {
    pub(crate) pc: BinaryField32b,
    pub(crate) fp: u32,
    pub(crate) timestamp: u32,
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

pub type ProgramRom = HashMap<BinaryField32b, Instruction>;

impl ValueRom {
    pub fn new(vrom: Vec<u32>) -> Self {
        Self(vrom)
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn extend(&mut self, slice: &[u32]) {
        self.0.extend(slice);
    }

    pub(crate) fn set(&mut self, index: BinaryField32b, value: u32) {
        let index_val = index.val() as usize;
        if index.val() as usize >= self.len() {
            self.extend(&vec![0; index_val + 1 - self.len()]);
        }

        self[index_val] = value;
    }
    pub(crate) fn get(&self, index: BinaryField32b) -> u32 {
        let index_val = BinaryField32b::val(index) as usize;
        assert!(
            index_val < self.len(),
            "Value read in the VROM was never written before."
        );

        self[index_val]
    }
}

pub(crate) type Instruction = [BinaryField16b; 4];

#[derive(Debug)]
pub(crate) enum InterpreterError {
    InvalidOpcode,
    BadPc,
}

impl Interpreter {
    pub(crate) fn new(prom: ProgramRom) -> Self {
        Self {
            pc: BinaryField32b::ONE,
            fp: 0,
            timestamp: 0,
            prom,
            vrom: ValueRom::default(),
        }
    }

    pub(crate) fn new_with_vrom(prom: ProgramRom, vrom: ValueRom) -> Self {
        Self {
            pc: BinaryField32b::ONE,
            fp: 0,
            timestamp: 0,
            prom,
            vrom,
        }
    }

    pub(crate) fn incr_pc(&mut self) {
        self.pc *= G;
    }

    pub(crate) fn set_pc(&mut self, target: BinaryField32b) -> Result<(), InterpreterError> {
        self.pc = target;
        Ok(())
    }

    pub(crate) fn vrom_size(&self) -> usize {
        self.vrom.0.len()
    }

    pub(crate) fn is_halted(&self) -> bool {
        self.pc == BinaryField32b::ZERO
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
        let [opcode, ..] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let opcode = Opcode::try_from(opcode.val()).map_err(|_| InterpreterError::InvalidOpcode)?;
        match opcode {
            Opcode::Bnz => self.generate_bnz(trace)?,
            Opcode::Xori => self.generate_xori(trace)?,
            Opcode::Slli => self.generate_slli(trace)?,
            Opcode::Srli => self.generate_srli(trace)?,
            Opcode::Addi => self.generate_addi(trace)?,
            Opcode::Muli => self.generate_muli(trace)?,
            Opcode::Ret => self.generate_ret(trace)?,
            Opcode::Taili => self.generate_taili(trace)?,
            Opcode::Andi => self.generate_andi(trace)?,
            Opcode::MVVW => self.generate_mvv(trace)?,
            Opcode::B32Muli => self.generate_b32_muli(trace)?,
            Opcode::Add => self.generate_add(trace)?,
        }
        self.timestamp += 1;
        Ok(Some(()))
    }

    fn generate_bnz(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, cond, target_high, target_low] =
            self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let target = BinaryField32b::from_bases(&vec![*target_high, *target_low]).expect("Hello");
        let target_first: BinaryField16b = target.get_base(0);
        let target_second: BinaryField16b = target.get_base(1);
        println!(
            "target first {:?}, target second {:?}, target_high {:?} target low {:?}",
            target_first.val(),
            target_second.val(),
            target_high,
            target_low
        );
        let new_bnz_event = BnzEvent::generate_event(self, *cond, target);
        trace.bnz.push(new_bnz_event);

        Ok(())
    }

    fn generate_xori(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, src, imm] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let new_xori_event = XoriEvent::generate_event(self, *dst, *src, *imm);
        trace.xori.push(new_xori_event);

        Ok(())
    }

    fn generate_ret(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let new_ret_event = RetEvent::generate_event(self);
        trace.ret.push(new_ret_event);

        Ok(())
    }

    fn generate_slli(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        // let new_shift_event = SliEventStruct::new(&self, dst, src, imm, ShiftKind::Left);
        // new_shift_event.apply_event(self);
        let [_, dst, src, imm] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let new_shift_event = SliEvent::generate_event(self, *dst, *src, *imm, ShiftKind::Left);
        trace.shift.push(new_shift_event);

        Ok(())
    }
    fn generate_srli(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, src, imm] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let new_shift_event = SliEvent::generate_event(self, *dst, *src, *imm, ShiftKind::Right);
        trace.shift.push(new_shift_event);

        Ok(())
    }

    fn generate_taili(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, target_high, target_low, next_fp] =
            self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let target = BinaryField32b::from_bases(&vec![*target_high, *target_low]).expect("Hello");
        let new_taili_event = TailiEvent::generate_event(self, target, *next_fp);
        trace.taili.push(new_taili_event);

        Ok(())
    }

    fn generate_andi(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, src, imm] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let new_andi_event = AndiEvent::generate_event(self, *dst, *src, *imm);
        trace.andi.push(new_andi_event);

        Ok(())
    }

    fn generate_muli(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, src, imm] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let new_muli_event = MuliEvent::generate_event(self, *dst, *src, *imm);
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

        Ok(())
    }

    fn generate_b32_muli(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, src, imm] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let new_b32muli_event = B32MuliEvent::generate_event(self, *dst, *src, *imm);
        trace.b32_muli.push(new_b32muli_event);

        Ok(())
    }
    fn generate_add(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, src1, src2] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let new_add_event = AddEvent::generate_event(self, *dst, *src1, *src2);
        trace.add32.push(Add32Event::generate_event(
            self,
            BinaryField32b::new(new_add_event.src1_val),
            BinaryField32b::new(new_add_event.src2_val),
        ));
        trace.add.push(new_add_event);

        Ok(())
    }

    fn generate_addi(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, src, imm] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let imm = *imm;
        let new_addi_event = AddiEvent::generate_event(self, *dst, *src, imm);
        trace.add32.push(Add32Event::generate_event(
            self,
            BinaryField32b::new(new_addi_event.src_val),
            imm.into(),
        ));
        trace.addi.push(new_addi_event);

        Ok(())
    }

    fn generate_mvv(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, offset, src] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let new_mvvw_event = MVVWEvent::generate_event(self, *dst, *offset, *src);
        trace.mvvw.push(new_mvvw_event);

        Ok(())
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
    b32_muli: Vec<B32MuliEvent>,
    add: Vec<AddEvent>,
    vrom: ValueRom,
}

pub(crate) struct BoundaryValues {
    final_pc: BinaryField32b,
    final_fp: u32,
    timestamp: u32,
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

    pub(crate) fn generate_with_vrom(
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
        channels.state_channel.push((BinaryField32b::ONE, 0, 0));
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

pub(crate) fn collatz_orbits(initial_val: u32) -> (Vec<u32>, Vec<u32>) {
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

pub(crate) fn code_to_prom(code: &[Instruction]) -> ProgramRom {
    let mut prom = ProgramRom::new();
    let mut pc = BinaryField32b::ONE; // we start at PC = 1G.
    for inst in code {
        prom.insert(pc, *inst);
        pc *= G;
    }

    prom
}
#[cfg(test)]
mod tests {
    use binius_field::{Field, PackedField};

    use super::*;

    #[test]
    fn test_zcray() {
        let zero = BinaryField16b::zero();
        let code = vec![[Opcode::Ret.get_field_elt(), zero, zero, zero]];
        let prom = code_to_prom(&code);
        let (trace, boundary_values) = ZCrayTrace::generate(prom).expect("Ouch!");
        trace.validate(boundary_values);
    }

    #[test]
    fn test_sli_ret() {
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
        let prom = code_to_prom(&instructions);
        let vrom = ValueRom(vec![0, 0, 2, 0, 3]);
        let (traces, _) =
            ZCrayTrace::generate_with_vrom(prom, vrom).expect("Trace generation should not fail.");
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

    pub(crate) fn get_binary_slot(i: u16) -> BinaryField16b {
        BinaryField16b::new(i)
    }

    #[test]
    fn test_compiled_collatz() {
        // collatz:
        //  ;; Frame:
        // 	;; Slot @0: Return PC
        // 	;; Slot @1: Return FP
        // 	;; Slot @2: Arg: n
        //  ;; Slot @3: Return value
        // 	;; Slot @4: ND Local: Next FP
        // 	;; Slot @5: Local: n == 1
        // 	;; Slot @6: Local: n % 2
        // 	;; Slot @7: Local: 3*n
        //  ;; Slot @8: Local: n >> 2 or 3*n + 1

        // 	;; Branch to recursion label if value in slot 2 is not 1
        // 	XORI @5, @2, #1G
        // 	BNZ case_recurse, @5 ;; branch if n == 1
        // 	XORI @3, @2, #0G
        // 	RET

        // case_recurse:
        // 	ANDI @6, @2, #1 ;; n % 2 is & 0x00..01
        //  BNZ case_odd, @6 ;; branch if n % 2 == 0u32

        // 	;; case even
        //  ;; n >> 1
        // 	SRLI @8, @2, #1
        //  MVV.W @4[2], @8
        //  MVV.W @4[3], @3
        //  TAILI collatz, @4

        // case_odd:
        // 	MULI @7, @2, #3
        // 	ADDI @8, @6, #1
        //  MVV.W @4[2], @8
        //  MVV.W @4[3], @3
        // 	TAILI collatz, @4

        let zero = BinaryField16b::zero();
        // labels
        let collatz = BinaryField16b::ONE;
        let case_recurse = BinaryField16b::new(5);
        let case_odd = BinaryField16b::new(11);
        let next_fp_offset = 4;
        let next_fp = 9;
        let instructions = vec![
            // collatz:
            [
                Opcode::Xori.get_field_elt(),
                get_binary_slot(5),
                get_binary_slot(2),
                get_binary_slot(1),
            ], //  1: XORI 5 2 1
            [
                Opcode::Bnz.get_field_elt(),
                get_binary_slot(5),
                case_recurse,
                zero,
            ], //  2: BNZ 5 case_recurse
            // case_return:
            [
                Opcode::Xori.get_field_elt(),
                get_binary_slot(3),
                get_binary_slot(2),
                zero,
            ], //  3: XORI 3 2 zero
            [Opcode::Ret.get_field_elt(), zero, zero, zero], //  4: RET
            // case_recurse:
            [
                Opcode::Andi.get_field_elt(),
                get_binary_slot(6),
                get_binary_slot(2),
                get_binary_slot(1),
            ], //  5: ANDI 6 2 1
            [
                Opcode::Bnz.get_field_elt(),
                get_binary_slot(6),
                case_odd,
                zero,
            ], //  6: BNZ 6 case_odd 0 0
            // case_even:
            [
                Opcode::Srli.get_field_elt(),
                get_binary_slot(8),
                get_binary_slot(2),
                get_binary_slot(1),
            ], //  7: SRLI 8 2 1
            [
                Opcode::MVVW.get_field_elt(),
                get_binary_slot(4),
                get_binary_slot(2),
                get_binary_slot(8),
            ], //  8: MVV.W @4[2], @8
            [
                Opcode::MVVW.get_field_elt(),
                get_binary_slot(4),
                get_binary_slot(3),
                get_binary_slot(3),
            ], //  9: MVV.W @4[3], @3
            [
                Opcode::Taili.get_field_elt(),
                collatz,
                get_binary_slot(4),
                zero,
            ], // 10: TAILI collatz 4 0
            // case_odd:
            [
                Opcode::Muli.get_field_elt(),
                get_binary_slot(7),
                get_binary_slot(2),
                get_binary_slot(3),
            ], //  11: MULI 7 2 3
            [
                Opcode::Addi.get_field_elt(),
                get_binary_slot(8),
                get_binary_slot(7),
                get_binary_slot(1),
            ], //  12: ADDI 8 7 1
            [
                Opcode::MVVW.get_field_elt(),
                get_binary_slot(4),
                get_binary_slot(2),
                get_binary_slot(8),
            ], //  13: MVV.W @4[2], @7
            [
                Opcode::MVVW.get_field_elt(),
                get_binary_slot(4),
                get_binary_slot(3),
                get_binary_slot(3),
            ], //  14: MVV.W @4[3], @3
            [
                Opcode::Taili.get_field_elt(),
                collatz,
                get_binary_slot(4),
                zero,
            ], //  15: TAILI collatz 4 0
        ];
        let initial_val = 3999;
        let (expected_evens, expected_odds) = collatz_orbits(initial_val);
        let nb_frames = expected_evens.len() + expected_odds.len();
        let prom = code_to_prom(&instructions);
        // return PC = 0, return FP = 0, n = 3999
        let mut vrom = ValueRom(vec![0, 0, initial_val]);
        for i in 0..nb_frames {
            vrom.set(
                BinaryField32b::new((i * next_fp + next_fp_offset) as u32),
                ((i + 1) * next_fp) as u32,
            );
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
}
