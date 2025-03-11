//! The core zkVM emulator, that executes instructions parsed from the immutable
//! Instruction Memory (PROM). It processes events and updates the machine state
//! accordingly.

use std::{array::from_fn, collections::HashMap, fmt::Debug, hash::Hash};

use binius_field::{BinaryField, BinaryField16b, BinaryField32b, ExtensionField, Field};
use binius_utils::bail;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use tracing::{debug, trace};

use crate::{
    event::{
        b32::{AndiEvent, B32MulEvent, B32MuliEvent, XorEvent, XoriEvent},
        branch::{BnzEvent, BzEvent},
        call::{TailVEvent, TailiEvent},
        integer_ops::{Add32Event, Add64Event, AddEvent, AddiEvent, MuliEvent},
        mv::{LDIEvent, MVIHEvent, MVVLEvent, MVVWEvent},
        ret::RetEvent,
        sli::{ShiftKind, SliEvent},
        Event,
        ImmediateBinaryOperation,
        NonImmediateBinaryOperation, // Add the import for RetEvent
    },
    instructions_with_labels::LabelsFrameSizes,
    opcodes::Opcode,
};

pub(crate) const G: BinaryField32b = BinaryField32b::MULTIPLICATIVE_GENERATOR;
#[derive(Debug, Default)]
pub struct Channel<T> {
    net_multiplicities: HashMap<T, isize>,
}

// TODO: Think on unifying types used for recurring variables (fp, pc, ...)

type PromChannel = Channel<(u32, u128)>; // PC, opcode, args (so 64 bits overall).
type VromChannel = Channel<u32>;
type StateChannel = Channel<(BinaryField32b, u32, u32)>; // PC, FP, Timestamp

#[derive(Default)]
pub struct InterpreterChannels {
    pub state_channel: StateChannel,
}

type VromTable32 = HashMap<u32, u32>;
#[derive(Default)]
pub struct InterpreterTables {
    pub vrom_table_32: VromTable32,
}

// TODO: Add some structured execution tracing

#[derive(Debug, Default)]
pub(crate) struct Interpreter {
    pub(crate) pc: BinaryField32b,
    pub(crate) fp: u32,
    pub(crate) timestamp: u32,
    pub(crate) prom: ProgramRom,
    pub(crate) vrom: ValueRom,
    latest_fp: u32,
    frames: LabelsFrameSizes,
}

#[derive(Debug, Default)]
pub(crate) struct ValueRom(HashMap<u32, u8>);

// impl Index<usize> for ValueRom {
//     type Output = u8;

//     fn index(&self, index: usize) -> &Self::Output {
//         &self.0[index] // Forward indexing to the inner vector
//     }
// }

// impl IndexMut<usize> for ValueRom {
//     fn index_mut(&mut self, index: usize) -> &mut Self::Output {
//         &mut self.0[index] // Forward indexing to the inner vector
//     }
// }

/// The Program ROM, or Instruction Memory, is an immutable memory where code is
/// loaded. It maps every PC to a specific instruction to execute.
pub type ProgramRom = HashMap<BinaryField32b, Instruction>;

impl ValueRom {
    pub const fn new(vrom: HashMap<u32, u8>) -> Self {
        Self(vrom)
    }

    pub fn new_from_vec(vals: Vec<u8>) -> Self {
        let mut vrom = Self::default();

        for (i, val) in vals.into_iter().enumerate() {
            vrom.set_u8(i as u32, val);
        }

        vrom
    }

    pub fn new_from_vec_u32(vals: Vec<u32>) -> Self {
        let mut vrom = Self::default();

        for (i, val) in vals.into_iter().enumerate() {
            vrom.set_u32(4 * i as u32, val);
        }

        vrom
    }

    pub(crate) fn set_u8(&mut self, index: u32, value: u8) -> Result<(), InterpreterError> {
        if let Some(prev_val) = self.0.insert(index, value) {
            if prev_val != value {
                return Err(InterpreterError::VromRewrite(index));
            }
        }
        Ok(())
    }

    pub(crate) fn set_u16(&mut self, index: u32, value: u16) -> Result<(), InterpreterError> {
        if index % 2 != 0 {
            return Err(InterpreterError::VromMisaligned(16, index));
        }
        let bytes = value.to_le_bytes();
        for i in 0..2 {
            self.set_u8(index + i, bytes[i as usize])?;
        }
        Ok(())
    }

    pub(crate) fn set_u32(&mut self, index: u32, value: u32) -> Result<(), InterpreterError> {
        if index % 4 != 0 {
            return Err(InterpreterError::VromMisaligned(32, index));
        }
        let bytes = value.to_le_bytes();
        for i in 0..4 {
            self.set_u8(index + i, bytes[i as usize])?;
        }
        Ok(())
    }

    pub(crate) fn set_u128(&mut self, index: u32, value: u128) -> Result<(), InterpreterError> {
        if index % 16 != 0 {
            return Err(InterpreterError::VromMisaligned(128, index));
        }
        let bytes = value.to_le_bytes();
        for i in 0..16 {
            self.set_u8(index + i, bytes[i as usize])?;
        }
        Ok(())
    }

    pub(crate) fn get_u8(&self, index: u32) -> Result<u8, InterpreterError> {
        match self.0.get(&index) {
            Some(&value) => Ok(value),
            None => Err(InterpreterError::VromMissingValue(index)),
        }
    }

    pub(crate) fn get_u16(&self, index: u32) -> Result<u16, InterpreterError> {
        if index % 2 != 0 {
            return Err(InterpreterError::VromMisaligned(16, index));
        }

        let mut bytes = [0; 2];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = self.get_u8(index + i as u32)?;
        }

        Ok(u16::from_le_bytes(bytes))
    }

    pub(crate) fn get_u32(&self, index: u32) -> Result<u32, InterpreterError> {
        if index % 4 != 0 {
            return Err(InterpreterError::VromMisaligned(32, index));
        }
        let mut bytes = [0; 4];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = self.get_u8(index + i as u32)?;
        }

        Ok(u32::from_le_bytes(bytes))
    }

    pub(crate) fn get_u128(&self, index: u32) -> Result<u128, InterpreterError> {
        if index % 16 != 0 {
            return Err(InterpreterError::VromMisaligned(128, index));
        }
        let mut bytes = [0; 16];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = self.get_u8(index + i as u32)?;
        }

        Ok(u128::from_le_bytes(bytes))
    }
}

/// An `Instruction` is composed of an opcode and up to three 16-bit arguments
/// to be used by this operation.
pub(crate) type Instruction = [BinaryField16b; 4];

#[derive(Debug)]
pub(crate) enum InterpreterError {
    InvalidOpcode,
    BadPc,
    InvalidInput,
    VromRewrite(u32),
    VromMisaligned(u8, u32),
    VromMissingValue(u32),
    Exception(InterpreterException),
}

#[derive(Debug)]
pub(crate) enum InterpreterException {}

impl Interpreter {
    pub(crate) fn new(prom: ProgramRom, frames: LabelsFrameSizes) -> Self {
        Self {
            pc: BinaryField32b::ONE,
            fp: 0,
            timestamp: 0,
            prom,
            vrom: ValueRom::default(),
            latest_fp: 0,
            frames,
        }
    }

    pub(crate) const fn new_with_vrom(
        prom: ProgramRom,
        vrom: ValueRom,
        frames: LabelsFrameSizes,
    ) -> Self {
        Self {
            pc: BinaryField32b::ONE,
            fp: 0,
            timestamp: 0,
            prom,
            vrom,
            latest_fp: 0,
            frames,
        }
    }

    #[inline(always)]
    pub(crate) fn incr_pc(&mut self) {
        self.pc *= G;
    }

    #[inline(always)]
    pub(crate) fn jump_to(&mut self, target: BinaryField32b) {
        self.pc = target;
    }

    #[inline(always)]
    pub(crate) fn vrom_size(&self) -> usize {
        self.vrom.0.len()
    }

    #[inline(always)]
    pub(crate) fn is_halted(&self) -> bool {
        self.pc == BinaryField32b::ZERO
    }

    pub fn run(&mut self) -> Result<ZCrayTrace, InterpreterError> {
        let mut trace = ZCrayTrace::default();
        self.allocate_new_frame(self.pc)?;
        loop {
            match self.step(&mut trace) {
                Ok(_) => {}
                Err(error) => {
                    match error {
                        InterpreterError::Exception(exc) => {} //TODO: handle exception
                        critical_error => {
                            panic!("{:?}", critical_error);
                        } //TODO: properly format error
                    }
                }
            }
            if self.is_halted() {
                return Ok(trace);
            }
        }
    }

    pub fn step(&mut self, trace: &mut ZCrayTrace) -> Result<Option<()>, InterpreterError> {
        let [opcode, ..] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let opcode = Opcode::try_from(opcode.val()).map_err(|_| InterpreterError::InvalidOpcode)?;
        trace!("Executing {:?} at timestamp {:?}", opcode, self.timestamp);
        match opcode {
            Opcode::Bnz => self.generate_bnz(trace)?,
            Opcode::Xori => self.generate_xori(trace)?,
            Opcode::Xor => self.generate_xor(trace)?,
            Opcode::Slli => self.generate_slli(trace)?,
            Opcode::Srli => self.generate_srli(trace)?,
            Opcode::Addi => self.generate_addi(trace)?,
            Opcode::Add => self.generate_add(trace)?,
            Opcode::Muli => self.generate_muli(trace)?,
            Opcode::Ret => self.generate_ret(trace)?,
            Opcode::Taili => self.generate_taili(trace)?,
            Opcode::TailV => self.generate_tailv(trace)?,
            Opcode::Andi => self.generate_andi(trace)?,
            Opcode::MVIH => self.generate_mvih(trace)?,
            Opcode::MVVW => self.generate_mvvw(trace)?,
            Opcode::MVVL => self.generate_mvvl(trace)?,
            Opcode::LDI => self.generate_ldi(trace)?,
            Opcode::B32Mul => self.generate_b32_mul(trace)?,
            Opcode::B32Muli => self.generate_b32_muli(trace)?,
        }
        self.timestamp += 1;
        Ok(Some(()))
    }

    fn generate_bnz(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let &[_, cond, target_low, target_high] =
            self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let target = (BinaryField32b::from_bases([target_low, target_high]))
            .map_err(|_| InterpreterError::InvalidInput)?;
        let cond_val = self.vrom.get_u32(self.fp ^ cond.val() as u32)?;
        if cond_val != 0 {
            let new_bnz_event = BnzEvent::generate_event(self, cond, target)?;
            trace.bnz.push(new_bnz_event);
        } else {
            let new_bz_event = BzEvent::generate_event(self, cond, target)?;
            trace.bz.push(new_bz_event);
        }

        Ok(())
    }

    fn generate_xori(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, src, imm] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let new_xori_event = XoriEvent::generate_event(self, *dst, *src, *imm)?;
        trace.xori.push(new_xori_event);

        Ok(())
    }

    fn generate_xor(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, src1, src2] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let new_xor_event = XorEvent::generate_event(self, *dst, *src1, *src2)?;
        trace.xor.push(new_xor_event);

        Ok(())
    }

    fn generate_ret(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let new_ret_event = RetEvent::generate_event(self)?;
        trace.ret.push(new_ret_event);

        Ok(())
    }

    fn generate_slli(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        // let new_shift_event = SliEventStruct::new(&self, dst, src, imm,
        // ShiftKind::Left); new_shift_event.apply_event(self);
        let [_, dst, src, imm] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let new_shift_event = SliEvent::generate_event(self, *dst, *src, *imm, ShiftKind::Left)?;
        trace.shift.push(new_shift_event);

        Ok(())
    }
    fn generate_srli(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, src, imm] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let new_shift_event = SliEvent::generate_event(self, *dst, *src, *imm, ShiftKind::Right)?;
        trace.shift.push(new_shift_event);

        Ok(())
    }

    fn generate_tailv(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, offset, next_fp, _] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let offset_b32 = (*offset).into();
        let new_tailv_event = TailVEvent::generate_event(self, *offset, *next_fp)?;
        self.allocate_new_frame(offset_b32)?;
        trace.tailv.push(new_tailv_event);

        Ok(())
    }

    fn generate_taili(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, target_low, target_high, next_fp] =
            self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let target = BinaryField32b::from_bases([*target_low, *target_high])
            .map_err(|_| InterpreterError::InvalidInput)?;
        let new_taili_event = TailiEvent::generate_event(self, target, *next_fp)?;
        self.allocate_new_frame(target)?;
        trace.taili.push(new_taili_event);

        Ok(())
    }

    fn generate_andi(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, src, imm] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let new_andi_event = AndiEvent::generate_event(self, *dst, *src, *imm)?;
        trace.andi.push(new_andi_event);

        Ok(())
    }

    fn generate_muli(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, src, imm] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let new_muli_event = MuliEvent::generate_event(self, *dst, *src, *imm)?;
        let aux = new_muli_event.aux;
        let sum0 = new_muli_event.sum0;
        let sum1 = new_muli_event.sum1;

        // This is to check sum0 = aux[0] + aux[1] << 8.
        trace.add64.push(Add64Event::generate_event(
            self,
            aux[0] as u64,
            (aux[1] as u64) << 8,
        ));
        // This is to check sum1 = aux[2] + aux[3] << 8.
        trace.add64.push(Add64Event::generate_event(
            self,
            aux[2] as u64,
            (aux[3] as u64) << 8,
        ));
        // This is to check that dst_val = sum0 + sum1 << 8.
        trace
            .add64
            .push(Add64Event::generate_event(self, sum0, sum1 << 8));
        trace.muli.push(new_muli_event);

        Ok(())
    }

    fn generate_b32_mul(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, src1, src2] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let new_b32mul_event = B32MulEvent::generate_event(self, *dst, *src1, *src2)?;
        trace.b32_mul.push(new_b32mul_event);

        Ok(())
    }

    fn generate_b32_muli(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, src, imm_low] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let [second_opcode, imm_high, third, fourth] = self
            .prom
            .get(&(self.pc * G))
            .ok_or(InterpreterError::BadPc)?;

        if second_opcode.val() != Opcode::B32Muli.into()
            || *third != BinaryField16b::ZERO
            || *fourth != BinaryField16b::ZERO
        {
            return Err(InterpreterError::BadPc);
        }
        let imm = BinaryField32b::from_bases([*imm_low, *imm_high])
            .map_err(|_| InterpreterError::InvalidInput)?;
        let new_b32muli_event = B32MuliEvent::generate_event(self, *dst, *src, imm)?;
        trace.b32_muli.push(new_b32muli_event);

        Ok(())
    }

    fn generate_add(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, src1, src2] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let new_add_event = AddEvent::generate_event(self, *dst, *src1, *src2)?;
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
        let new_addi_event = AddiEvent::generate_event(self, *dst, *src, imm)?;
        trace.add32.push(Add32Event::generate_event(
            self,
            BinaryField32b::new(new_addi_event.src_val),
            imm.into(),
        ));
        trace.addi.push(new_addi_event);

        Ok(())
    }

    fn generate_mvvw(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, offset, src] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let new_mvvw_event = MVVWEvent::generate_event(self, *dst, *offset, *src)?;
        trace.mvvw.push(new_mvvw_event);

        Ok(())
    }

    fn generate_mvvl(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, offset, src] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let new_mvvl_event = MVVLEvent::generate_event(self, *dst, *offset, *src)?;
        trace.mvvl.push(new_mvvl_event);

        Ok(())
    }

    fn generate_mvih(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, offset, imm] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let new_mvih_event = MVIHEvent::generate_event(self, *dst, *offset, *imm)?;
        trace.mvih.push(new_mvih_event);

        Ok(())
    }

    fn generate_ldi(&mut self, trace: &mut ZCrayTrace) -> Result<(), InterpreterError> {
        let [_, dst, imm_low, imm_high] = self.prom.get(&self.pc).ok_or(InterpreterError::BadPc)?;
        let imm = BinaryField32b::from_bases([*imm_low, *imm_high])
            .map_err(|_| InterpreterError::InvalidInput)?;
        let new_ldi_event = LDIEvent::generate_event(self, *dst, imm)?;
        trace.ldi.push(new_ldi_event);

        Ok(())
    }

    // TODO: This is only a temporary method.
    fn allocate_new_frame(&mut self, target: BinaryField32b) -> Result<(), InterpreterError> {
        // We need the +16 because the frame size we read corresponds to the largest
        // offset accessed within the frame, and the largest possible object is
        // a BF128.
        // TODO: Figure out the exact size of the last object.
        let (frame_size, opt_args_size) = self
            .frames
            .get(&target)
            .ok_or(InterpreterError::InvalidInput)?;
        let mut max_size = *frame_size;
        for (fs, _) in self.frames.values() {
            if *fs > max_size {
                max_size = *fs;
            }
        }
        let frame_size = (max_size + 16).next_power_of_two();
        // Add 8 for return PC and return FP.
        let next_fp_offset = opt_args_size.expect("Args size missing.") as u32 + 8;
        let alignment = (frame_size - self.latest_fp as u16 % frame_size) % frame_size;
        assert!(alignment == 0);

        let next_fp = self.latest_fp + alignment as u32 + frame_size as u32;

        self.vrom.set_u32(self.latest_fp + next_fp_offset, next_fp);

        self.latest_fp = next_fp;

        Ok(())
    }
}

impl<T: Hash + Eq + Debug> Channel<T> {
    pub(crate) fn push(&mut self, val: T) {
        trace!("PUSH {:?}", val);
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
        trace!("PULL {:?}", val);
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
}

impl StateChannel {
    pub(crate) fn is_balanced(&self) -> bool {
        #[cfg(debug_assertions)]
        if !self.net_multiplicities.is_empty() {
            let mut sorted_multiplicities: Vec<_> =
                self.net_multiplicities.clone().into_iter().collect();

            // Sort by timestamp
            sorted_multiplicities.sort_by_key(|((_pc, _fp, timestamp), _)| *timestamp);

            // TODO: better debugging?
            debug!("Unbalanced State Channel:");
            let _ = sorted_multiplicities
                .iter()
                .map(|x| trace!("{:?}", x))
                .collect::<Vec<_>>();
        }
        self.net_multiplicities.is_empty()
    }
}

#[derive(Debug, Default)]
pub(crate) struct ZCrayTrace {
    bnz: Vec<BnzEvent>,
    xor: Vec<XorEvent>,
    bz: Vec<BzEvent>,
    xori: Vec<XoriEvent>,
    andi: Vec<AndiEvent>,
    shift: Vec<SliEvent>,
    addi: Vec<AddiEvent>,
    add32: Vec<Add32Event>,
    add64: Vec<Add64Event>,
    muli: Vec<MuliEvent>,
    taili: Vec<TailiEvent>,
    tailv: Vec<TailVEvent>,
    ret: Vec<RetEvent>,
    mvih: Vec<MVIHEvent>,
    mvvw: Vec<MVVWEvent>,
    mvvl: Vec<MVVLEvent>,
    ldi: Vec<LDIEvent>,
    b32_mul: Vec<B32MulEvent>,
    b32_muli: Vec<B32MuliEvent>,
    add: Vec<AddEvent>,

    vrom: ValueRom,
}

pub(crate) struct BoundaryValues {
    final_pc: BinaryField32b,
    final_fp: u32,
    timestamp: u32,
}

/// Convenience macro to `fire` all events logged.
/// This will execute all the flushes that these events trigger.
macro_rules! fire_events {
    ($events:expr, $channels:expr, $tables:expr) => {
        $events
            .iter()
            .for_each(|event| event.fire($channels, $tables));
    };
}

impl ZCrayTrace {
    fn generate(
        prom: ProgramRom,
        frames: LabelsFrameSizes,
    ) -> Result<(Self, BoundaryValues), InterpreterError> {
        let mut interpreter = Interpreter::new(prom, frames);

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
        frames: LabelsFrameSizes,
    ) -> Result<(Self, BoundaryValues), InterpreterError> {
        let mut interpreter = Interpreter::new_with_vrom(prom, vrom, frames);

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

        let tables = InterpreterTables::default();

        // Initial boundary push: PC = 1, FP = 0, TIMESTAMP = 0.
        channels.state_channel.push((BinaryField32b::ONE, 0, 0));
        // Final boundary pull.
        channels.state_channel.pull((
            boundary_values.final_pc,
            boundary_values.final_fp,
            boundary_values.timestamp,
        ));

        fire_events!(self.bnz, &mut channels, &tables);
        fire_events!(self.bz, &mut channels, &tables);
        fire_events!(self.xor, &mut channels, &tables);
        fire_events!(self.xori, &mut channels, &tables);
        fire_events!(self.andi, &mut channels, &tables);
        fire_events!(self.shift, &mut channels, &tables);
        fire_events!(self.addi, &mut channels, &tables);
        fire_events!(self.add, &mut channels, &tables);
        fire_events!(self.add32, &mut channels, &tables);
        fire_events!(self.add64, &mut channels, &tables);
        fire_events!(self.muli, &mut channels, &tables);
        fire_events!(self.b32_mul, &mut channels, &tables);
        fire_events!(self.b32_muli, &mut channels, &tables);
        fire_events!(self.ldi, &mut channels, &tables);
        fire_events!(self.taili, &mut channels, &tables);
        fire_events!(self.tailv, &mut channels, &tables);
        fire_events!(self.ret, &mut channels, &tables);
        fire_events!(self.mvvw, &mut channels, &tables);
        fire_events!(self.mvvl, &mut channels, &tables);
        fire_events!(self.mvih, &mut channels, &tables);

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
    use env_logger::{try_init_from_env, Env, DEFAULT_FILTER_ENV};
    use tracing_subscriber::EnvFilter;

    use super::*;
    use crate::parser::parse_program;
    use crate::{get_full_prom_and_labels, instructions_with_labels::get_frame_sizes_all_labels};

    fn init_logger() {
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }

    #[test]
    fn test_zcray() {
        let zero = BinaryField16b::zero();
        let code = vec![[Opcode::Ret.get_field_elt(), zero, zero, zero]];
        let prom = code_to_prom(&code);
        let vrom = ValueRom::new_from_vec_u32(vec![0, 0]);
        let mut frames = HashMap::new();
        frames.insert(BinaryField32b::ONE, (12, Some(0)));
        let (trace, boundary_values) =
            ZCrayTrace::generate_with_vrom(prom, vrom, frames).expect("Ouch!");
        trace.validate(boundary_values);
    }

    #[test]
    fn test_sli_ret() {
        let zero = BinaryField16b::zero();
        let shift1_dst = BinaryField16b::new(16);
        let shift1_src = BinaryField16b::new(12);
        let shift1 = BinaryField16b::new(5);

        let shift2_dst = BinaryField16b::new(24);
        let shift2_src = BinaryField16b::new(20);
        let shift2 = BinaryField16b::new(7);

        let instructions = vec![
            [Opcode::Slli.get_field_elt(), shift1_dst, shift1_src, shift1],
            [Opcode::Srli.get_field_elt(), shift2_dst, shift2_src, shift2],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];
        let mut frames = HashMap::new();
        frames.insert(BinaryField32b::ONE, (24, Some(0)));
        let prom = code_to_prom(&instructions);

        //  ;; Frame:
        // 	;; Slot @0: Return PC
        // 	;; Slot @4: Return FP
        // 	;; Slot @8: ND Local: Next FP
        // 	;; Slot @12: Local: src1
        // 	;; Slot @16: Local: dst1
        // 	;; Slot @20: Local: src2
        //  ;; Slot @24: Local: dst2
        let mut vrom = ValueRom::default();
        vrom.set_u32(0, 0);
        vrom.set_u32(4, 0);
        vrom.set_u32(12, 2);
        vrom.set_u32(20, 3);

        let (traces, _) = ZCrayTrace::generate_with_vrom(prom, vrom, frames)
            .expect("Trace generation should not fail.");
        let shifts = vec![
            SliEvent::new(BinaryField32b::ONE, 0, 0, 16, 64, 12, 2, 5, ShiftKind::Left),
            SliEvent::new(G, 0, 1, 24, 0, 20, 3, 7, ShiftKind::Right),
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
        //     collatz:
        //     ;; Frame:
        //     ;; Slot @0: Return PC
        //     ;; Slot @4: Return FP
        //     ;; Slot @8: Arg: n
        //     ;; Slot @12: Return value
        //     ;; Slot @16: ND Local: Next FP
        //     ;; Slot @20: Local: n == 1
        //     ;; Slot @24: Local: n % 2
        //     ;; Slot @28: Local: 3*n
        //     ;; Slot @32: Local: n >> 2 or 3*n + 1

        //     ;; Branch to recursion label if value in slot 2 is not 1
        //     XORI @20, @8, #1
        //     BNZ case_recurse, @20 ;; branch if n != 1
        //     XORI @12, @8, #0
        //     RET

        // case_recurse:
        //     ANDI @24, @8, #1 ;; n % 2 is & 0x00..01
        //     BNZ case_odd, @24 ;; branch if n % 2 == 0u32

        //     ;; case even
        //     ;; n >> 1
        //     SRLI @32, @8, #1
        //     MVV.W @16[8], @32
        //     MVV.W @16[12], @12
        //     TAILI collatz, @16

        // case_odd:
        //     MULI @28, @8, #3
        //     ADDI @32, @28, #1
        //     MVV.W @16[8], @32
        //     MVV.W @16[12], @12
        //     TAILI collatz, @16

        init_logger();

        let zero = BinaryField16b::zero();
        // labels
        let collatz = BinaryField16b::ONE;
        let case_recurse = ExtensionField::<BinaryField16b>::iter_bases(&G.pow(4))
            .collect::<Vec<BinaryField16b>>();
        let case_odd = ExtensionField::<BinaryField16b>::iter_bases(&G.pow(10))
            .collect::<Vec<BinaryField16b>>();

        let instructions = vec![
            // collatz:
            [
                Opcode::Xori.get_field_elt(),
                get_binary_slot(20),
                get_binary_slot(8),
                get_binary_slot(1),
            ], //  0G: XORI 20 8 1
            [
                Opcode::Bnz.get_field_elt(),
                get_binary_slot(20),
                case_recurse[0],
                case_recurse[1],
            ], //  1G: BNZ 20 case_recurse
            // case_return:
            [
                Opcode::Xori.get_field_elt(),
                get_binary_slot(12),
                get_binary_slot(8),
                zero,
            ], //  2G: XORI 12 8 zero
            [Opcode::Ret.get_field_elt(), zero, zero, zero], //  3G: RET
            // case_recurse:
            [
                Opcode::Andi.get_field_elt(),
                get_binary_slot(24),
                get_binary_slot(8),
                get_binary_slot(1),
            ], // 4G: ANDI 24 8 1
            [
                Opcode::Bnz.get_field_elt(),
                get_binary_slot(24),
                case_odd[0],
                case_odd[1],
            ], //  5G: BNZ 24 case_odd
            // case_even:
            [
                Opcode::Srli.get_field_elt(),
                get_binary_slot(32),
                get_binary_slot(8),
                get_binary_slot(1),
            ], //  6G: SRLI 32 8 1
            [
                Opcode::MVVW.get_field_elt(),
                get_binary_slot(16),
                get_binary_slot(8),
                get_binary_slot(32),
            ], //  7G: MVV.W @16[8], @32
            [
                Opcode::MVVW.get_field_elt(),
                get_binary_slot(16),
                get_binary_slot(12),
                get_binary_slot(12),
            ], //  8G: MVV.W @16[12], @12
            [
                Opcode::Taili.get_field_elt(),
                collatz,
                zero,
                get_binary_slot(16),
            ], // 9G: TAILI collatz 16
            // case_odd:
            [
                Opcode::Muli.get_field_elt(),
                get_binary_slot(28),
                get_binary_slot(8),
                get_binary_slot(3),
            ], //  10G: MULI 28 8 3
            [
                Opcode::Addi.get_field_elt(),
                get_binary_slot(32),
                get_binary_slot(28),
                get_binary_slot(1),
            ], //  11G: ADDI 32 28 1
            [
                Opcode::MVVW.get_field_elt(),
                get_binary_slot(16),
                get_binary_slot(8),
                get_binary_slot(32),
            ], //  12G: MVV.W @16[8], @32
            [
                Opcode::MVVW.get_field_elt(),
                get_binary_slot(16),
                get_binary_slot(12),
                get_binary_slot(12),
            ], //  13G: MVV.W @16[12], @12
            [
                Opcode::Taili.get_field_elt(),
                collatz,
                zero,
                get_binary_slot(16),
            ], //  14G: TAILI collatz 16
        ];
        let initial_val = 5;
        let (expected_evens, expected_odds) = collatz_orbits(initial_val);
        let prom = code_to_prom(&instructions);
        // return PC = 0, return FP = 0, n = 5
        let mut vrom = ValueRom::new_from_vec_u32(vec![0, 0, initial_val]);

        // TODO: Do not set the return value manually.
        vrom.set_u32(12, 1);

        // TODO: We could build this with compiler hints.
        let mut frames_args_size = HashMap::new();
        frames_args_size.insert(BinaryField32b::ONE, (32, Some(8)));

        let (traces, boundary_values) =
            ZCrayTrace::generate_with_vrom(prom, vrom, frames_args_size)
                .expect("Trace generation should not fail.");

        traces.validate(boundary_values);

        assert!(traces.shift.len() == expected_evens.len());
        for (i, &even) in expected_evens.iter().enumerate() {
            assert!(traces.shift[i].src_val == even);
        }
        assert!(traces.muli.len() == expected_odds.len());
        for (i, &odd) in expected_odds.iter().enumerate() {
            assert!(traces.muli[i].src_val == odd);
        }

        let nb_frames = expected_evens.len() + expected_odds.len();
        let mut cur_val = initial_val;

        for i in 0..nb_frames {
            assert_eq!(
                traces.vrom.get_u32((i as u32 * 16 + 4) * 4).unwrap(), // next_fp
                ((i + 1) * 16 * 4) as u32                              // next_fp_val
            );
            assert_eq!(
                traces.vrom.get_u32((i as u32 * 16 + 2) * 4).unwrap(), // n
                cur_val                                                // n_val
            );

            if cur_val % 2 == 0 {
                cur_val /= 2;
            } else {
                cur_val = 3 * cur_val + 1;
            }
        }
    }

    #[test]
    fn test_fibonacci() {
        let instructions = parse_program(include_str!("../../examples/fib.asm")).unwrap();

        let (prom, labels) = get_full_prom_and_labels(&instructions)
            .expect("Instructions were not formatted properly.");

        let mut label_args = HashMap::new();
        for (label, pc) in &labels {
            match label.as_str() {
                "fib" => {
                    label_args.insert(*pc, 8);
                }
                "fib_helper" => {
                    label_args.insert(*pc, 16);
                }
                _ => {}
            }
        }

        let frame_sizes = get_frame_sizes_all_labels(&prom, labels, label_args);
        let mut max_frame = 0;
        for (fs, _) in frame_sizes.values() {
            if *fs as u32 > max_frame {
                max_frame = *fs as u32;
            }
        }
        max_frame = (max_frame + 16).next_power_of_two();

        let init_val = 4;
        let initial_value = G.pow(init_val as u64).val();

        // Set initial PC, FP and argument.
        let mut vrom = ValueRom::new_from_vec_u32(vec![0, 0, initial_value]);

        // TODO: Do not set the return value manually.
        vrom.set_u32(12, fibonacci(init_val as usize));

        let (traces, _) = ZCrayTrace::generate_with_vrom(prom, vrom, frame_sizes)
            .expect("Trace generation should not fail.");

        // Check that Fibonacci is computed properly.
        let elt_size = 4_u32; // Each value is 32 bits.
        let mut cur_fibs = [0, 1];
        for i in 0..init_val {
            let s = cur_fibs[0] + cur_fibs[1];
            assert_eq!(
                traces
                    .vrom
                    .get_u32((i + 1) * max_frame + 2 * elt_size)
                    .unwrap(),
                cur_fibs[0],
                "left {} right {}",
                traces
                    .vrom
                    .get_u32((i + 1) * max_frame + 2 * elt_size)
                    .unwrap(),
                cur_fibs[0]
            );
            assert_eq!(
                traces
                    .vrom
                    .get_u32((i + 1) * max_frame + 3 * elt_size)
                    .unwrap(),
                cur_fibs[1]
            );
            assert_eq!(
                traces
                    .vrom
                    .get_u32((i + 1) * max_frame + 7 * elt_size)
                    .unwrap(),
                s
            );
            cur_fibs[0] = cur_fibs[1];
            cur_fibs[1] = s;
        }
        assert_eq!(
            traces
                .vrom
                .get_u32((init_val + 1) * max_frame + 5 * elt_size)
                .unwrap(),
            cur_fibs[0]
        );
    }

    fn fibonacci(n: usize) -> u32 {
        let mut cur_fibs = [0, 1];
        for _ in 0..n {
            let s = cur_fibs[0] + cur_fibs[1];
            cur_fibs[0] = cur_fibs[1];
            cur_fibs[1] = s;
        }
        cur_fibs[0]
    }
}
