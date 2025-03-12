//! The core zkVM emulator, that executes instructions parsed from the immutable
//! Instruction Memory (PROM). It processes events and updates the machine state
//! accordingly.

use std::{array::from_fn, collections::HashMap, fmt::Debug, hash::Hash};

use binius_field::{
    BinaryField, BinaryField128b, BinaryField16b, BinaryField32b, ExtensionField, Field,
    PackedField,
};
use binius_utils::bail;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use tracing::{debug, trace};

use crate::{
    event::{
        b128::{B128AddEvent, B128MulEvent},
        b32::{
            AndEvent, AndiEvent, B32MulEvent, B32MuliEvent, OrEvent, OriEvent, XorEvent, XoriEvent,
        },
        branch::{BnzEvent, BzEvent},
        call::{TailVEvent, TailiEvent},
        integer_ops::{Add32Event, Add64Event, AddEvent, AddiEvent, MuliEvent},
        mv::{LDIEvent, MVEventOutput, MVIHEvent, MVInfo, MVKind, MVVLEvent, MVVWEvent},
        ret::RetEvent,
        sli::{ShiftKind, SliEvent},
        Event,
        ImmediateBinaryOperation,
        NonImmediateBinaryOperation, // Add the import for RetEvent
    },
    instructions_with_labels::LabelsFrameSizes,
    opcodes::Opcode,
    vrom_allocator::VromAllocator,
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
    /// The integer PC represents to the exponent of the actual field
    /// PC (which starts at `BinaryField32b::ONE` and iterate over the
    /// multiplicative group). Since we need to have a value for 0 as well
    /// (which is not in the multiplicative group), we shift all powers by
    /// 1, and 0 can be the halting value.
    pub(crate) pc: u32,
    pub(crate) fp: u32,
    pub(crate) timestamp: u32,
    pub(crate) prom: ProgramRom,
    pub(crate) vrom: ValueRom,
    frames: LabelsFrameSizes,
    vrom_allocator: VromAllocator,
    /// Before a CALL, there are a few move operations used to populate the next
    /// frame. But the next frame pointer is not necessarily known at this
    /// point, and return values may also not be known. Thus, this `Vec` is
    /// used to store the move operations that need to be handled once we
    /// have enough information. Stores all move operations that should be
    /// handles during the current call procedure.
    pub(crate) moves_to_apply: Vec<MVInfo>,
    // Temporary HashMap storing the mapping between binary field elements that appear in the PROM
    // and their associated integer PC.
    pc_field_to_int: HashMap<BinaryField32b, u32>,
}

type ToSetValue = (
    u32, // parent addr
    Opcode,
    BinaryField32b, // field PC
    u32,            // fp
    u32,            // timestamp
    BinaryField16b, // dst
    BinaryField16b, // src
    BinaryField16b, // offset
);

#[derive(Debug, Default)]
pub(crate) struct ValueRom {
    vrom: HashMap<u32, u8>,
    // HashMap used to set values and push MV events during a CALL procedure.
    // When a MV occurs with a value that isn't set within a CALL procedure, we
    // assume it is a return value. Then, we add (addr_next_frame,
    // to_set_value) to `moves_to_set`, where `to_set_value` contains enough information to create
    // a move event later. Whenever an address in the HashMap's keys is finally set, we populate
    // the missing values and remove them from the HashMap.
    to_set: HashMap<u32, ToSetValue>,
}

/// The Program ROM, or Instruction Memory, is an immutable memory where code is
/// loaded. It maps every PC to a specific instruction to execute.
pub type ProgramRom = Vec<InterpreterInstruction>;

impl ValueRom {
    pub fn new(vrom: HashMap<u32, u8>) -> Self {
        Self {
            vrom,
            to_set: HashMap::new(),
        }
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
            vrom.set_u32(&mut ZCrayTrace::default(), 4 * i as u32, val);
        }

        vrom
    }

    pub(crate) fn set_u8(&mut self, index: u32, value: u8) -> Result<(), InterpreterError> {
        if let Some(prev_val) = self.vrom.insert(index, value) {
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

    pub(crate) fn set_u32(
        &mut self,
        trace: &mut ZCrayTrace,
        index: u32,
        value: u32,
    ) -> Result<(), InterpreterError> {
        if index % 4 != 0 {
            return Err(InterpreterError::VromMisaligned(32, index));
        }
        let bytes = value.to_le_bytes();
        for i in 0..4 {
            self.set_u8(index + i, bytes[i as usize])?;
        }

        if let Some((parent, opcode, field_pc, fp, timestamp, dst, src, offset)) =
            self.to_set.remove(&index)
        {
            self.set_u32(trace, parent, value);
            let event_out = MVEventOutput::new(
                parent,
                opcode,
                field_pc,
                fp,
                timestamp,
                dst,
                src,
                offset,
                value as u128,
            );
            event_out.push_mv_event(trace);
        }
        Ok(())
    }

    pub(crate) fn set_u128(
        &mut self,
        trace: &mut ZCrayTrace,
        index: u32,
        value: u128,
    ) -> Result<(), InterpreterError> {
        if index % 16 != 0 {
            return Err(InterpreterError::VromMisaligned(128, index));
        }
        let bytes = value.to_le_bytes();
        for i in 0..16 {
            self.set_u8(index + i, bytes[i as usize])?;
        }

        if let Some((parent, opcode, field_pc, fp, timestamp, dst, src, offset)) =
            self.to_set.remove(&index)
        {
            self.set_u128(trace, parent, value)?;
            let event_out = MVEventOutput::new(
                parent, opcode, field_pc, fp, timestamp, dst, src, offset, value,
            );
            event_out.push_mv_event(trace);
        }
        Ok(())
    }

    pub(crate) fn get_u8(&self, index: u32) -> Result<u8, InterpreterError> {
        match self.vrom.get(&index) {
            Some(&value) => Ok(value),
            None => Err(InterpreterError::VromMissingValue(index)),
        }
    }

    pub(crate) fn get_u8_call_procedure(&self, index: u32) -> Option<u8> {
        self.vrom.get(&index).copied()
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

    pub(crate) fn get_u32_move(&self, index: u32) -> Result<Option<u32>, InterpreterError> {
        if index % 4 != 0 {
            return Err(InterpreterError::VromMisaligned(32, index));
        }
        let opt_bytes = from_fn(|i| self.get_u8_call_procedure(index + i as u32));
        if opt_bytes.iter().any(|v| v.is_none()) {
            Ok(None)
        } else {
            Ok(Some(u32::from_le_bytes(opt_bytes.map(|v| v.unwrap()))))
        }
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

    pub(crate) fn get_u128_move(&self, index: u32) -> Result<Option<u128>, InterpreterError> {
        if index % 16 != 0 {
            return Err(InterpreterError::VromMisaligned(128, index));
        }
        let opt_bytes = from_fn(|i| self.get_u8_call_procedure(index + i as u32));

        if opt_bytes.iter().any(|v| v.is_none()) {
            Ok(None)
        } else {
            Ok(Some(u128::from_le_bytes(opt_bytes.map(|v| v.unwrap()))))
        }
    }

    pub(crate) fn insert_to_set(
        &mut self,
        dst: u32,
        to_set_val: ToSetValue,
    ) -> Result<(), InterpreterError> {
        if self.to_set.insert(dst, to_set_val).is_some() {
            return Err(InterpreterError::VromRewrite(dst));
        };

        Ok(())
    }
}

/// An `Instruction` is composed of an opcode and up to three 16-bit arguments
/// to be used by this operation.
pub(crate) type Instruction = [BinaryField16b; 4];

#[derive(Debug, Default, PartialEq)]
pub(crate) struct InterpreterInstruction {
    pub(crate) instruction: Instruction,
    pub(crate) field_pc: BinaryField32b,
    // Hint given by the compiler to let us know whether the current instruction is part of a CALL
    // procedure. If so, all following instructions are too, until we reach a CALL. Moreover, we
    // assume all instructions that are part of the call procedure to be MV instructions used to
    // populate the next frame.
    is_call_procedure: bool,
}

impl InterpreterInstruction {
    pub(crate) fn new(
        instruction: Instruction,
        field_pc: BinaryField32b,
        is_call_procedure: bool,
    ) -> Self {
        Self {
            instruction,
            field_pc,
            is_call_procedure,
        }
    }
}

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
    pub(crate) fn new(
        prom: ProgramRom,
        frames: LabelsFrameSizes,
        pc_field_to_int: HashMap<BinaryField32b, u32>,
    ) -> Self {
        Self {
            pc: 1,
            fp: 0,
            timestamp: 0,
            prom,
            vrom: ValueRom::default(),
            frames,
            pc_field_to_int,
            vrom_allocator: VromAllocator::default(),
            moves_to_apply: vec![],
        }
    }

    pub(crate) fn new_with_vrom(
        prom: ProgramRom,
        vrom: ValueRom,
        frames: LabelsFrameSizes,
        pc_field_to_int: HashMap<BinaryField32b, u32>,
    ) -> Self {
        Self {
            pc: 1,
            fp: 0,
            timestamp: 0,
            prom,
            vrom,
            frames,
            pc_field_to_int,
            vrom_allocator: VromAllocator::default(),
            moves_to_apply: vec![],
        }
    }

    #[inline(always)]
    pub(crate) fn incr_pc(&mut self) {
        self.pc += 1;
    }

    #[inline(always)]
    pub(crate) fn jump_to(&mut self, target: BinaryField32b) {
        if target == BinaryField32b::zero() {
            self.pc = 0;
        } else {
            self.pc = *self
                .pc_field_to_int
                .get(&target)
                .expect("This target should have been parsed.");
        }
    }

    /// This method should only be called once the frame pointer has been
    /// allocated. It is used to generate events -- whenever possible --
    /// once the next_fp has been set by the allocator. When it is not yet
    /// possible to generate the move event (because we are dealing with a
    /// return value that has not yet been set), we add the move information to
    /// `self.to_set`, so that it can be generated later on.
    pub(crate) fn handles_call_moves(
        &mut self,
        trace: &mut ZCrayTrace,
    ) -> Result<(), InterpreterError> {
        for mv_info in &self.moves_to_apply.clone() {
            match mv_info.mv_kind {
                MVKind::Mvvw => {
                    let opt_event = MVVWEvent::generate_event_from_info(
                        self,
                        trace,
                        mv_info.pc,
                        mv_info.timestamp,
                        self.fp,
                        mv_info.dst,
                        mv_info.offset,
                        mv_info.src,
                    )?;
                    if let Some(event) = opt_event {
                        trace.mvvw.push(event);
                    }
                }
                MVKind::Mvvl => {
                    let opt_event = MVVLEvent::generate_event_from_info(
                        self,
                        trace,
                        mv_info.pc,
                        mv_info.timestamp,
                        self.fp,
                        mv_info.dst,
                        mv_info.offset,
                        mv_info.src,
                    )?;
                    if let Some(event) = opt_event {
                        trace.mvvl.push(event);
                    }
                }
                MVKind::Mvih => {
                    let event = MVIHEvent::generate_event_from_info(
                        self,
                        trace,
                        mv_info.pc,
                        mv_info.timestamp,
                        self.fp,
                        mv_info.dst,
                        mv_info.offset,
                        mv_info.src,
                    )?;
                    trace.mvih.push(event);
                }
            }
        }
        self.moves_to_apply = vec![];
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn vrom_size(&self) -> usize {
        self.vrom.vrom.len()
    }

    #[inline(always)]
    pub(crate) fn is_halted(&self) -> bool {
        self.pc == 0 // The real PC should be 0, which is outside of the
    }

    pub fn run(&mut self) -> Result<ZCrayTrace, InterpreterError> {
        let mut trace = ZCrayTrace::default();

        let field_pc = self.prom[self.pc as usize - 1].field_pc;
        // Start by allocating a frame for the initial label.
        self.allocate_new_frame(field_pc);
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
        if self.pc as usize - 1 > self.prom.len() {
            return Err(InterpreterError::BadPc);
        }
        let instruction = &self.prom[self.pc as usize - 1];
        let [opcode, arg0, arg1, arg2] = instruction.instruction;
        let field_pc = instruction.field_pc;
        let is_call_procedure = instruction.is_call_procedure;

        debug_assert_eq!(field_pc, G.pow(self.pc as u64 - 1));

        let opcode = Opcode::try_from(opcode.val()).map_err(|_| InterpreterError::InvalidOpcode)?;
        trace!("Executing {:?} at timestamp {:?}", opcode, self.timestamp);
        match opcode {
            Opcode::Bnz => {
                self.generate_bnz(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::Xori => {
                self.generate_xori(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::Xor => {
                self.generate_xor(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::Slli => {
                self.generate_slli(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::Srli => {
                self.generate_srli(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::Addi => {
                self.generate_addi(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::Add => {
                self.generate_add(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::Muli => {
                self.generate_muli(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::Ret => {
                self.generate_ret(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::Taili => {
                self.generate_taili(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::TailV => {
                self.generate_tailv(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::And => {
                self.generate_and(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::Andi => {
                self.generate_andi(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::Or => self.generate_or(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?,
            Opcode::Ori => {
                self.generate_ori(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::MVIH => {
                self.generate_mvih(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::MVVW => {
                self.generate_mvvw(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::MVVL => {
                self.generate_mvvl(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::LDI => {
                self.generate_ldi(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::B32Mul => {
                self.generate_b32_mul(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::B32Muli => {
                self.generate_b32_muli(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::B128Add => {
                self.generate_b128_add(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
            Opcode::B128Mul => {
                self.generate_b128_mul(trace, field_pc, is_call_procedure, arg0, arg1, arg2)?
            }
        }
        self.timestamp += 1;
        Ok(Some(()))
    }

    fn generate_bnz(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        cond: BinaryField16b,
        target_low: BinaryField16b,
        target_high: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let target = (BinaryField32b::from_bases([target_low, target_high]))
            .map_err(|_| InterpreterError::InvalidInput)?;
        let cond_val = self.vrom.get_u32(self.fp ^ cond.val() as u32)?;
        if cond_val != 0 {
            let new_bnz_event = BnzEvent::generate_event(self, cond, target, field_pc)?;
            trace.bnz.push(new_bnz_event);
        } else {
            let new_bz_event = BzEvent::generate_event(self, cond, target, field_pc)?;
            trace.bz.push(new_bz_event);
        }

        Ok(())
    }

    fn generate_xori(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_xori_event = XoriEvent::generate_event(self, trace, dst, src, imm, field_pc)?;
        trace.xori.push(new_xori_event);

        Ok(())
    }

    fn generate_xor(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_xor_event = XorEvent::generate_event(self, trace, dst, src1, src2, field_pc)?;
        trace.xor.push(new_xor_event);

        Ok(())
    }

    fn generate_ret(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        _: BinaryField16b,
        _: BinaryField16b,
        _: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_ret_event = RetEvent::generate_event(self, field_pc)?;
        trace.ret.push(new_ret_event);

        Ok(())
    }

    fn generate_slli(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_shift_event =
            SliEvent::generate_event(self, trace, dst, src, imm, ShiftKind::Left, field_pc)?;
        trace.shift.push(new_shift_event);

        Ok(())
    }
    fn generate_srli(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_shift_event =
            SliEvent::generate_event(self, trace, dst, src, imm, ShiftKind::Right, field_pc)?;
        trace.shift.push(new_shift_event);

        Ok(())
    }

    fn generate_tailv(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        offset: BinaryField16b,
        next_fp: BinaryField16b,
        _: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_tailv_event = TailVEvent::generate_event(self, trace, offset, next_fp, field_pc)?;
        trace.tailv.push(new_tailv_event);

        Ok(())
    }

    fn generate_taili(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        target_low: BinaryField16b,
        target_high: BinaryField16b,
        next_fp: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let target = BinaryField32b::from_bases([target_low, target_high])
            .map_err(|_| InterpreterError::InvalidInput)?;
        let next_fp_val = self.allocate_new_frame(target)?;
        let new_taili_event =
            TailiEvent::generate_event(self, trace, target, next_fp, next_fp_val, field_pc)?;
        trace.taili.push(new_taili_event);

        Ok(())
    }

    fn generate_and(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_and_event = AndEvent::generate_event(self, trace, dst, src1, src2, field_pc)?;
        trace.and.push(new_and_event);

        Ok(())
    }

    fn generate_andi(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_andi_event = AndiEvent::generate_event(self, trace, dst, src, imm, field_pc)?;
        trace.andi.push(new_andi_event);

        Ok(())
    }

    fn generate_or(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_or_event = OrEvent::generate_event(self, trace, dst, src1, src2, field_pc)?;
        trace.or.push(new_or_event);

        Ok(())
    }

    fn generate_ori(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_ori_event = OriEvent::generate_event(self, trace, dst, src, imm, field_pc)?;
        trace.ori.push(new_ori_event);

        Ok(())
    }

    fn generate_muli(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_muli_event = MuliEvent::generate_event(self, trace, dst, src, imm, field_pc)?;
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

    fn generate_b32_mul(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_b32mul_event = B32MulEvent::generate_event(self, trace, dst, src1, src2, field_pc)?;
        trace.b32_mul.push(new_b32mul_event);

        Ok(())
    }

    fn generate_b32_muli(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm_low: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        if self.pc as usize > self.prom.len() {
            return Err(InterpreterError::BadPc);
        }
        let [second_opcode, imm_high, third, fourth] = self.prom[self.pc as usize].instruction;

        if second_opcode.val() != Opcode::B32Muli.into()
            || third != BinaryField16b::ZERO
            || fourth != BinaryField16b::ZERO
        {
            return Err(InterpreterError::BadPc);
        }
        let imm = BinaryField32b::from_bases([imm_low, imm_high])
            .map_err(|_| InterpreterError::InvalidInput)?;
        let new_b32muli_event = B32MuliEvent::generate_event(self, trace, dst, src, imm, field_pc)?;
        trace.b32_muli.push(new_b32muli_event);

        Ok(())
    }

    fn generate_b128_add(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_b128_add_event =
            B128AddEvent::generate_event(self, trace, dst, src1, src2, field_pc)?;
        trace.b128_add.push(new_b128_add_event);
        Ok(())
    }

    fn generate_b128_mul(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_b128_mul_event =
            B128MulEvent::generate_event(self, trace, dst, src1, src2, field_pc)?;
        trace.b128_mul.push(new_b128_mul_event);
        Ok(())
    }

    fn generate_add(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_add_event = AddEvent::generate_event(self, trace, dst, src1, src2, field_pc)?;
        trace.add32.push(Add32Event::generate_event(
            self,
            new_add_event.src1_val,
            new_add_event.src2_val,
        ));
        trace.add.push(new_add_event);

        Ok(())
    }

    fn generate_addi(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_addi_event = AddiEvent::generate_event(self, trace, dst, src, imm, field_pc)?;
        trace.add32.push(Add32Event::generate_event(
            self,
            new_addi_event.src_val,
            imm.val() as u32,
        ));
        trace.addi.push(new_addi_event);

        Ok(())
    }

    fn generate_mvvw(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        is_call_procedure: bool,
        dst: BinaryField16b,
        offset: BinaryField16b,
        src: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let opt_new_mvvw_event =
            MVVWEvent::generate_event(self, trace, dst, offset, src, field_pc, is_call_procedure)?;
        if let Some(new_mvvw_event) = opt_new_mvvw_event {
            trace.mvvw.push(new_mvvw_event);
        }

        Ok(())
    }

    fn generate_mvvl(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        is_call_procedure: bool,
        dst: BinaryField16b,
        offset: BinaryField16b,
        src: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let opt_new_mvvl_event =
            MVVLEvent::generate_event(self, trace, dst, offset, src, field_pc, is_call_procedure)?;
        if let Some(new_mvvl_event) = opt_new_mvvl_event {
            trace.mvvl.push(new_mvvl_event);
        }

        Ok(())
    }

    fn generate_mvih(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        is_call_procedure: bool,
        dst: BinaryField16b,
        offset: BinaryField16b,
        imm: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let opt_new_mvih_event =
            MVIHEvent::generate_event(self, trace, dst, offset, imm, field_pc, is_call_procedure)?;
        if let Some(new_mvih_event) = opt_new_mvih_event {
            trace.mvih.push(new_mvih_event);
        }

        Ok(())
    }

    fn generate_ldi(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        _: bool,
        dst: BinaryField16b,
        imm_low: BinaryField16b,
        imm_high: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let imm = BinaryField32b::from_bases([imm_low, imm_high])
            .map_err(|_| InterpreterError::InvalidInput)?;
        let new_ldi_event = LDIEvent::generate_event(self, trace, dst, imm, field_pc)?;
        trace.ldi.push(new_ldi_event);

        Ok(())
    }

    // TODO: This is only a temporary method.
    pub(crate) fn allocate_new_frame(
        &mut self,
        target: BinaryField32b,
        // trace: &mut ZCrayTrace,
    ) -> Result<u32, InterpreterError> {
        let frame_size = self
            .frames
            .get(&target)
            .ok_or(InterpreterError::InvalidInput)?;
        // We need the +16 because the frame size we read corresponds to the largest
        // offset accessed within the frame, and the largest possible object is
        // a BF128.
        // TODO: Figure out the exact size of the last object.
        Ok(self.vrom_allocator.alloc(*frame_size as u32 + 16))
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
    or: Vec<OrEvent>,
    ori: Vec<OriEvent>,
    xori: Vec<XoriEvent>,
    and: Vec<AndEvent>,
    andi: Vec<AndiEvent>,
    shift: Vec<SliEvent>,
    add: Vec<AddEvent>,
    addi: Vec<AddiEvent>,
    add32: Vec<Add32Event>,
    add64: Vec<Add64Event>,
    muli: Vec<MuliEvent>,
    taili: Vec<TailiEvent>,
    tailv: Vec<TailVEvent>,
    ret: Vec<RetEvent>,
    mvih: Vec<MVIHEvent>,
    pub(crate) mvvw: Vec<MVVWEvent>,
    pub(crate) mvvl: Vec<MVVLEvent>,
    ldi: Vec<LDIEvent>,
    b32_mul: Vec<B32MulEvent>,
    b32_muli: Vec<B32MuliEvent>,
    b128_add: Vec<B128AddEvent>,
    b128_mul: Vec<B128MulEvent>,

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
        pc_field_to_int: HashMap<BinaryField32b, u32>,
    ) -> Result<(Self, BoundaryValues), InterpreterError> {
        let mut interpreter = Interpreter::new(prom, frames, pc_field_to_int);

        let mut trace = interpreter.run()?;
        trace.vrom = interpreter.vrom;

        let final_pc = if interpreter.pc == 0 {
            BinaryField32b::zero()
        } else {
            G.pow(interpreter.pc as u64)
        };

        let boundary_values = BoundaryValues {
            final_pc,
            final_fp: interpreter.fp,
            timestamp: interpreter.timestamp,
        };

        Ok((trace, boundary_values))
    }

    pub(crate) fn generate_with_vrom(
        prom: ProgramRom,
        vrom: ValueRom,
        frames: LabelsFrameSizes,
        pc_field_to_int: HashMap<BinaryField32b, u32>,
    ) -> Result<(Self, BoundaryValues), InterpreterError> {
        let mut interpreter = Interpreter::new_with_vrom(prom, vrom, frames, pc_field_to_int);

        let mut trace = interpreter.run()?;
        trace.vrom = interpreter.vrom;

        let final_pc = if interpreter.pc == 0 {
            BinaryField32b::zero()
        } else {
            G.pow(interpreter.pc as u64)
        };

        let boundary_values = BoundaryValues {
            final_pc,
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
        fire_events!(self.xor, &mut channels, &tables);
        fire_events!(self.bz, &mut channels, &tables);
        fire_events!(self.or, &mut channels, &tables);
        fire_events!(self.ori, &mut channels, &tables);
        fire_events!(self.xori, &mut channels, &tables);
        fire_events!(self.and, &mut channels, &tables);
        fire_events!(self.andi, &mut channels, &tables);
        fire_events!(self.shift, &mut channels, &tables);
        fire_events!(self.add, &mut channels, &tables);
        fire_events!(self.addi, &mut channels, &tables);
        fire_events!(self.add32, &mut channels, &tables);
        fire_events!(self.add64, &mut channels, &tables);
        fire_events!(self.muli, &mut channels, &tables);
        fire_events!(self.taili, &mut channels, &tables);
        fire_events!(self.tailv, &mut channels, &tables);
        fire_events!(self.ret, &mut channels, &tables);
        fire_events!(self.mvih, &mut channels, &tables);
        fire_events!(self.mvvw, &mut channels, &tables);
        fire_events!(self.mvvl, &mut channels, &tables);
        fire_events!(self.ldi, &mut channels, &tables);
        fire_events!(self.b32_mul, &mut channels, &tables);
        fire_events!(self.b32_muli, &mut channels, &tables);
        fire_events!(self.b128_add, &mut channels, &tables);
        fire_events!(self.b128_mul, &mut channels, &tables);

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

#[cfg(test)]
mod tests {
    use binius_field::{BinaryField128b, Field, PackedField};
    use env_logger::{try_init_from_env, Env, DEFAULT_FILTER_ENV};
    use tracing_subscriber::EnvFilter;

    use super::*;
    use crate::parser::parse_program;
    use crate::{get_full_prom_and_labels, instructions_with_labels::get_frame_sizes_all_labels};

    fn init_logger() {
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }

    pub(crate) fn code_to_prom(
        code: &[Instruction],
        is_calling_procedure_hints: &[bool],
    ) -> ProgramRom {
        let mut prom = ProgramRom::new();
        let mut pc = BinaryField32b::ONE; // we start at PC = 1G.
        for (i, &instruction) in code.iter().enumerate() {
            let interp_inst =
                InterpreterInstruction::new(instruction, pc, is_calling_procedure_hints[i]);
            prom.push(interp_inst);
            pc *= G;
        }

        prom
    }

    #[test]
    fn test_zcray() {
        let zero = BinaryField16b::zero();
        let code = vec![[Opcode::Ret.get_field_elt(), zero, zero, zero]];
        let prom = code_to_prom(&code, &[false]);
        let vrom = ValueRom::new_from_vec_u32(vec![0, 0]);
        let mut frames = HashMap::new();
        frames.insert(BinaryField32b::ONE, 12);

        let (trace, boundary_values) =
            ZCrayTrace::generate_with_vrom(prom, vrom, frames, HashMap::new()).expect("Ouch!");
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
        frames.insert(BinaryField32b::ONE, 24);

        let prom = code_to_prom(&instructions, &vec![false; instructions.len()]);

        // Create a dummy trace only used to populate the initial VROM.
        let mut dummy_zcray = ZCrayTrace::default();

        //  ;; Frame:
        // 	;; Slot @0: Return PC
        // 	;; Slot @4: Return FP
        // 	;; Slot @8: ND Local: Next FP
        // 	;; Slot @12: Local: src1
        // 	;; Slot @16: Local: dst1
        // 	;; Slot @20: Local: src2
        //  ;; Slot @24: Local: dst2
        let mut vrom = ValueRom::default();
        vrom.set_u32(&mut dummy_zcray, 0, 0);
        vrom.set_u32(&mut dummy_zcray, 4, 0);
        vrom.set_u32(&mut dummy_zcray, 12, 2);
        vrom.set_u32(&mut dummy_zcray, 20, 3);

        let (traces, _) = ZCrayTrace::generate_with_vrom(prom, vrom, frames, HashMap::new())
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

        // Add targets needed in the code.
        let mut pc_field_to_int = HashMap::new();
        // Add collatz
        pc_field_to_int.insert(collatz.into(), 1);
        // Add case_recurse
        pc_field_to_int.insert(G.pow(4), 5);
        // Add case_odd
        pc_field_to_int.insert(G.pow(10), 11);

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

        // Set to `true` the move operations that are part of a CALL procedure in the
        // Collatz code.
        let mut is_calling_procedure_hints = vec![false; instructions.len()];
        let indices = vec![7, 8, 9, 12, 13, 14];
        for idx in indices {
            is_calling_procedure_hints[idx] = true;
        }

        let prom = code_to_prom(&instructions, &is_calling_procedure_hints);
        // return PC = 0, return FP = 0, n = 5
        let mut vrom = ValueRom::new_from_vec_u32(vec![0, 0, initial_val]);

        // TODO: We could build this with compiler hints.
        let mut frames_args_size = HashMap::new();
        frames_args_size.insert(BinaryField32b::ONE, 32);

        let (traces, boundary_values) =
            ZCrayTrace::generate_with_vrom(prom, vrom, frames_args_size, pc_field_to_int)
                .expect("Trace generation should not fail.");

        traces.validate(boundary_values);

        assert!(
            traces.shift.len() == expected_evens.len(),
            "Generated an incorrect number of even cases."
        );
        for (i, &even) in expected_evens.iter().enumerate() {
            assert!(
                traces.shift[i].src_val == even,
                "Incorrect input to an even case."
            );
        }
        assert!(
            traces.muli.len() == expected_odds.len(),
            "Generated an incorrect number of odd cases."
        );
        for (i, &odd) in expected_odds.iter().enumerate() {
            assert!(
                traces.muli[i].src_val == odd,
                "Incorrect input to an odd case."
            );
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
        let mut instructions = parse_program(include_str!("../../examples/fib.asm")).unwrap();

        let mut is_calling_procedure_hints = vec![false; instructions.len()];
        let indices = vec![1, 2, 3, 4, 5, 15, 16, 17, 18, 19];
        for idx in indices {
            is_calling_procedure_hints[idx] = true;
        }

        let (prom, labels, pc_field_to_int) =
            get_full_prom_and_labels(&instructions, &is_calling_procedure_hints)
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

        // The following method gives the correct frames for Fibonacci.
        let frame_sizes = get_frame_sizes_all_labels(&prom, labels, &pc_field_to_int);
        let mut max_frame = 0;
        for fs in frame_sizes.values() {
            if *fs as u32 > max_frame {
                max_frame = *fs as u32;
            }
        }
        max_frame = (max_frame + 16).next_power_of_two();

        let init_val = 4;
        let initial_value = G.pow(init_val as u64).val();

        // Set initial PC, FP and argument.
        let mut vrom = ValueRom::new_from_vec_u32(vec![0, 0, initial_value]);

        let (traces, _) = ZCrayTrace::generate_with_vrom(prom, vrom, frame_sizes, pc_field_to_int)
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

    #[test]
    fn test_b128_operations() {
        // Define opcodes and test values
        let zero = BinaryField16b::zero();

        // Offsets/addresses in our test program
        let a_offset = 0x10; // Must be 16-byte aligned
        let b_offset = 0x20; // Must be 16-byte aligned
        let c_offset = 0x30; // Must be 16-byte aligned
        let add_result_offset = 0x40; // Must be 16-byte aligned
        let mul_result_offset = 0x50; // Must be 16-byte aligned

        // Create binary field slot references
        let a_slot = BinaryField16b::new(a_offset as u16);
        let b_slot = BinaryField16b::new(b_offset as u16);
        let c_slot = BinaryField16b::new(c_offset as u16);
        let add_result_slot = BinaryField16b::new(add_result_offset as u16);
        let mul_result_slot = BinaryField16b::new(mul_result_offset as u16);

        // Construct a simple program with B128_ADD and B128_MUL instructions
        // 1. B128_ADD @add_result, @a, @b
        // 2. B128_MUL @mul_result, @add_result, @c
        // 3. RET
        let instructions = vec![
            [
                Opcode::B128Add.get_field_elt(),
                add_result_slot,
                a_slot,
                b_slot,
            ],
            [
                Opcode::B128Mul.get_field_elt(),
                mul_result_slot,
                add_result_slot,
                c_slot,
            ],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];

        // Create the PROM
        let prom = code_to_prom(&instructions, &vec![false; instructions.len()]);

        // Set up the VROM with test values
        let mut vrom = ValueRom::default();

        // Test values
        let a_val = 0x1111111122222222u128 | (0x3333333344444444u128 << 64);
        let b_val = 0x5555555566666666u128 | (0x7777777788888888u128 << 64);
        let c_val = 0x9999999988888888u128 | (0x7777777766666666u128 << 64);

        // Create a dummy trace only used to populate the initial VROM.
        let mut dummy_zcray = ZCrayTrace::default();

        // Set up return PC = 0, return FP = 0
        vrom.set_u32(&mut dummy_zcray, 0, 0).unwrap();
        vrom.set_u32(&mut dummy_zcray, 4, 0).unwrap();

        // Set the test values in VROM
        vrom.set_u128(&mut dummy_zcray, a_offset, a_val).unwrap();
        vrom.set_u128(&mut dummy_zcray, b_offset, b_val).unwrap();
        vrom.set_u128(&mut dummy_zcray, c_offset, c_val).unwrap();

        // Set up frame sizes
        let mut frames = HashMap::new();
        frames.insert(BinaryField32b::ONE, 128);

        // Create an interpreter and run the program
        let (trace, boundary_values) =
            ZCrayTrace::generate_with_vrom(prom, vrom, frames, HashMap::new())
                .expect("Trace generation should not fail.");

        // Capture the final PC before boundary_values is moved
        let final_pc = boundary_values.final_pc;

        // Validate the trace (this consumes boundary_values)
        trace.validate(boundary_values);

        // Calculate the expected results
        let expected_add = a_val ^ b_val;
        let a_bf = BinaryField128b::new(a_val);
        let b_bf = BinaryField128b::new(b_val);
        let c_bf = BinaryField128b::new(c_val);
        let add_result_bf = a_bf + b_bf;
        let expected_mul = (add_result_bf * c_bf).val();

        // Verify the results in VROM
        let actual_add = trace.vrom.get_u128(add_result_offset).unwrap();
        let actual_mul = trace.vrom.get_u128(mul_result_offset).unwrap();

        assert_eq!(actual_add, expected_add, "B128_ADD operation failed");
        assert_eq!(actual_mul, expected_mul, "B128_MUL operation failed");

        // Check that the events were created
        assert_eq!(
            trace.b128_add.len(),
            1,
            "Expected exactly one B128_ADD event"
        );
        assert_eq!(
            trace.b128_mul.len(),
            1,
            "Expected exactly one B128_MUL event"
        );

        // The trace should have completed successfully
        assert_eq!(
            final_pc,
            BinaryField32b::ZERO,
            "Program did not end correctly"
        );
    }
}
