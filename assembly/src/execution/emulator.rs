//! The core zkVM emulator, that executes instructions parsed from the immutable
//! Instruction Memory (PROM). It processes events and updates the machine state
//! accordingly.

use std::{collections::HashMap, fmt::Debug};

use binius_field::{
    BinaryField, BinaryField16b, BinaryField32b, ExtensionField, Field, PackedField,
};
use tracing::trace;

use crate::{
    assembler::LabelsFrameSizes,
    event::{
        b128::{B128AddEvent, B128MulEvent},
        b32::{
            AndEvent, AndiEvent, B32MulEvent, B32MuliEvent, OrEvent, OriEvent, XorEvent, XoriEvent,
        },
        branch::{BnzEvent, BzEvent},
        call::{CalliEvent, CallvEvent, TailVEvent, TailiEvent},
        integer_ops::{
            Add32Event, Add64Event, AddEvent, AddiEvent, MuliEvent, MuluEvent, SignedMulEvent,
            SignedMulKind, SltEvent, SltiEvent, SltiuEvent, SltuEvent, SubEvent,
        },
        jump::{JumpiEvent, JumpvEvent},
        mv::{LDIEvent, MVIHEvent, MVInfo, MVKind, MVVLEvent, MVVWEvent},
        ret::RetEvent,
        shift::{ShiftEvent, ShiftOperation},
        ImmediateBinaryOperation,
        NonImmediateBinaryOperation, // Add the import for RetEvent
    },
    execution::{StateChannel, ZCrayTrace},
    memory::{Memory, MemoryError, ProgramRom, ValueRom},
    opcodes::Opcode,
};

pub(crate) const G: BinaryField32b = BinaryField32b::MULTIPLICATIVE_GENERATOR;

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
    frames: LabelsFrameSizes,
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

/// An `Instruction` is composed of an opcode and up to three 16-bit arguments
/// to be used by this operation.
pub(crate) type Instruction = [BinaryField16b; 4];

#[derive(Debug, Default, PartialEq, Clone)]
pub struct InterpreterInstruction {
    pub(crate) instruction: Instruction,
    pub(crate) field_pc: BinaryField32b,
}

impl InterpreterInstruction {
    pub(crate) const fn new(instruction: Instruction, field_pc: BinaryField32b) -> Self {
        Self {
            instruction,
            field_pc,
        }
    }
    pub fn opcode(&self) -> Opcode {
        Opcode::try_from(self.instruction[0].val()).unwrap_or(Opcode::Invalid)
    }
}

#[derive(Debug)]
pub enum InterpreterError {
    InvalidOpcode,
    BadPc,
    InvalidInput,
    MemoryError(MemoryError),
    Exception(InterpreterException),
}

impl From<MemoryError> for InterpreterError {
    fn from(err: MemoryError) -> Self {
        InterpreterError::MemoryError(err)
    }
}

#[derive(Debug)]
pub enum InterpreterException {}

impl Interpreter {
    pub(crate) const fn new(
        frames: LabelsFrameSizes,
        pc_field_to_int: HashMap<BinaryField32b, u32>,
    ) -> Self {
        Self {
            pc: 1,
            fp: 0,
            timestamp: 0,
            frames,
            pc_field_to_int,
            moves_to_apply: vec![],
        }
    }

    #[inline(always)]
    pub(crate) const fn incr_pc(&mut self) {
        if self.pc == u32::MAX {
            // Skip over 0, as it is inaccessible in the multiplicative group.
            self.pc = 1
        } else {
            self.pc += 1;
        }
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
            debug_assert!(G.pow(self.pc as u64 - 1) == target);
        }
    }

    /// This method should only be called once the frame pointer has been
    /// allocated. It is used to generate events -- whenever possible --
    /// once the next_fp has been set by the allocator. When it is not yet
    /// possible to generate the MOVE event (because we are dealing with a
    /// return value that has not yet been set), we add the move information to
    /// the trace's `pending_updates`, so that it can be generated later on.
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
    pub(crate) const fn is_halted(&self) -> bool {
        self.pc == 0 // The real PC should be 0, which is outside of the
    }

    pub fn run(&mut self, memory: Memory) -> Result<ZCrayTrace, InterpreterError> {
        let mut trace = ZCrayTrace::new(memory);

        let field_pc = trace.prom()[self.pc as usize - 1].field_pc;
        // Start by allocating a frame for the initial label.
        self.allocate_new_frame(&mut trace, field_pc);
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
        if self.pc as usize - 1 > trace.prom().len() {
            return Err(InterpreterError::BadPc);
        }
        let instruction = &trace.prom()[self.pc as usize - 1];
        let [opcode, arg0, arg1, arg2] = instruction.instruction;
        let field_pc = instruction.field_pc;

        debug_assert_eq!(field_pc, G.pow(self.pc as u64 - 1));

        let opcode = Opcode::try_from(opcode.val()).map_err(|_| InterpreterError::InvalidOpcode)?;
        trace!("Executing {:?} at timestamp {:?}", opcode, self.timestamp);
        match opcode {
            Opcode::Bnz => self.generate_bnz(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Jumpi => self.generate_jumpi(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Jumpv => self.generate_jumpv(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Xori => self.generate_xori(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Xor => self.generate_xor(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Slli => self.generate_slli(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Srli => self.generate_srli(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Srai => self.generate_srai(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Sll => self.generate_sll(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Srl => self.generate_srl(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Sra => self.generate_sra(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Addi => self.generate_addi(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Add => self.generate_add(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Sub => self.generate_sub(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Slt => self.generate_slt(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Slti => self.generate_slti(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Sltu => self.generate_sltu(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Sltiu => self.generate_sltiu(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Muli => self.generate_muli(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Mulu => self.generate_mulu(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Mulsu => self.generate_mulsu(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Mul => self.generate_mul(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Ret => self.generate_ret(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Taili => self.generate_taili(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Tailv => self.generate_tailv(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Calli => self.generate_calli(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Callv => self.generate_callv(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::And => self.generate_and(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Andi => self.generate_andi(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Or => self.generate_or(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Ori => self.generate_ori(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Mvih => self.generate_mvih(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Mvvw => self.generate_mvvw(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Mvvl => self.generate_mvvl(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Ldi => self.generate_ldi(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::B32Mul => self.generate_b32_mul(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::B32Muli => self.generate_b32_muli(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::B128Add => self.generate_b128_add(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::B128Mul => self.generate_b128_mul(trace, field_pc, arg0, arg1, arg2)?,
            Opcode::Invalid => return Err(InterpreterError::InvalidOpcode),
        }
        self.timestamp += 1;
        Ok(Some(()))
    }

    fn generate_bnz(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        cond: BinaryField16b,
        target_low: BinaryField16b,
        target_high: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let target = (BinaryField32b::from_bases([target_low, target_high]))
            .map_err(|_| InterpreterError::InvalidInput)?;
        let cond_val = trace.get_vrom_u32(self.fp ^ cond.val() as u32)?;
        if cond_val != 0 {
            let new_bnz_event = BnzEvent::generate_event(self, trace, cond, target, field_pc)?;
            trace.bnz.push(new_bnz_event);
        } else {
            let new_bz_event = BzEvent::generate_event(self, trace, cond, target, field_pc)?;
            trace.bz.push(new_bz_event);
        }

        Ok(())
    }

    fn generate_jumpi(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        target_low: BinaryField16b,
        target_high: BinaryField16b,
        _: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let target = (BinaryField32b::from_bases([target_low, target_high]))
            .map_err(|_| InterpreterError::InvalidInput)?;
        let new_jumpi_event = JumpiEvent::generate_event(self, trace, target, field_pc)?;
        trace.jumpi.push(new_jumpi_event);

        Ok(())
    }

    fn generate_jumpv(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        offset: BinaryField16b,
        _: BinaryField16b,
        _: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_jumpv_event = JumpvEvent::generate_event(self, trace, offset, field_pc)?;
        trace.jumpv.push(new_jumpv_event);

        Ok(())
    }

    fn generate_xori(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
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
        _: BinaryField16b,
        _: BinaryField16b,
        _: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_ret_event = RetEvent::generate_event(self, trace, field_pc)?;
        trace.ret.push(new_ret_event);

        Ok(())
    }

    fn generate_slli(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_shift_event = ShiftEvent::generate_immediate_event(
            self,
            trace,
            dst,
            src,
            imm,
            ShiftOperation::LogicalLeft,
            field_pc,
        )?;
        trace.shifts.push(new_shift_event);
        Ok(())
    }

    fn generate_srli(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_shift_event = ShiftEvent::generate_immediate_event(
            self,
            trace,
            dst,
            src,
            imm,
            ShiftOperation::LogicalRight,
            field_pc,
        )?;
        trace.shifts.push(new_shift_event);
        Ok(())
    }

    fn generate_srai(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_shift_event = ShiftEvent::generate_immediate_event(
            self,
            trace,
            dst,
            src,
            imm,
            ShiftOperation::ArithmeticRight,
            field_pc,
        )?;
        trace.shifts.push(new_shift_event);
        Ok(())
    }

    fn generate_sll(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_shift_event = ShiftEvent::generate_vrom_event(
            self,
            trace,
            dst,
            src1,
            src2,
            ShiftOperation::LogicalLeft,
            field_pc,
        )?;
        trace.shifts.push(new_shift_event);
        Ok(())
    }

    fn generate_srl(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_shift_event = ShiftEvent::generate_vrom_event(
            self,
            trace,
            dst,
            src1,
            src2,
            ShiftOperation::LogicalRight,
            field_pc,
        )?;
        trace.shifts.push(new_shift_event);
        Ok(())
    }

    fn generate_sra(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_shift_event = ShiftEvent::generate_vrom_event(
            self,
            trace,
            dst,
            src1,
            src2,
            ShiftOperation::ArithmeticRight,
            field_pc,
        )?;
        trace.shifts.push(new_shift_event);

        Ok(())
    }

    fn generate_tailv(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
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
        target_low: BinaryField16b,
        target_high: BinaryField16b,
        next_fp: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let target = BinaryField32b::from_bases([target_low, target_high])
            .map_err(|_| InterpreterError::InvalidInput)?;
        let next_fp_val = self.allocate_new_frame(trace, target)?;
        let new_taili_event =
            TailiEvent::generate_event(self, trace, target, next_fp, next_fp_val, field_pc)?;
        trace.taili.push(new_taili_event);

        Ok(())
    }

    fn generate_calli(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        target_low: BinaryField16b,
        target_high: BinaryField16b,
        next_fp: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let target = BinaryField32b::from_bases([target_low, target_high])
            .map_err(|_| InterpreterError::InvalidInput)?;
        let next_fp_val = self.allocate_new_frame(trace, target)?;
        let new_calli_event =
            CalliEvent::generate_event(self, trace, target, next_fp, next_fp_val, field_pc)?;
        trace.calli.push(new_calli_event);

        Ok(())
    }

    fn generate_callv(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        offset: BinaryField16b,
        next_fp: BinaryField16b,
        _: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_callv_event = CallvEvent::generate_event(self, trace, offset, next_fp, field_pc)?;
        trace.callv.push(new_callv_event);

        Ok(())
    }

    fn generate_and(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
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
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_andi_event = AndiEvent::generate_event(self, trace, dst, src, imm, field_pc)?;
        trace.andi.push(new_andi_event);

        Ok(())
    }

    fn generate_sub(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_sub_event = SubEvent::generate_event(self, trace, dst, src1, src2, field_pc)?;
        trace.sub.push(new_sub_event);

        Ok(())
    }

    fn generate_slt(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_slt_event = SltEvent::generate_event(self, trace, dst, src1, src2, field_pc)?;
        trace.slt.push(new_slt_event);

        Ok(())
    }

    fn generate_slti(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_slti_event = SltiEvent::generate_event(self, trace, dst, src, imm, field_pc)?;
        trace.slti.push(new_slti_event);

        Ok(())
    }

    fn generate_sltu(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_sltu_event = SltuEvent::generate_event(self, trace, dst, src1, src2, field_pc)?;
        trace.sltu.push(new_sltu_event);

        Ok(())
    }

    fn generate_sltiu(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_sltiu_event = SltiuEvent::generate_event(self, trace, dst, src, imm, field_pc)?;
        trace.sltiu.push(new_sltiu_event);

        Ok(())
    }

    fn generate_or(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
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
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_muli_event = MuliEvent::generate_event(self, trace, dst, src, imm, field_pc)?;

        trace.muli.push(new_muli_event);

        Ok(())
    }

    fn generate_mulu(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_mulu_event = MuluEvent::generate_event(self, trace, dst, src1, src2, field_pc)?;
        let aux = new_mulu_event.aux;
        let aux_sums = new_mulu_event.aux_sums;
        let cum_sums = new_mulu_event.cum_sums;

        // This is to check aux_sums[i] = aux[2i] + aux[2i+1] << 8.
        for i in 0..aux.len() / 2 {
            trace.add64.push(Add64Event::generate_event(
                self,
                aux[2 * i] as u64,
                (aux[2 * i + 1] as u64) << 8,
            ));
        }
        // This is to check cum_sums[i] = cum_sums[i-1] + aux_sums[i] << 8.
        // Check the first element.
        trace.add64.push(Add64Event::generate_event(
            self,
            aux_sums[0],
            aux_sums[1] << 8,
        ));
        // CHeck the second element.
        trace.add64.push(Add64Event::generate_event(
            self,
            cum_sums[0],
            aux_sums[2] << 16,
        ));

        // This is to check that dst_val = cum_sums[1] + aux_sums[3] << 24.
        trace.add64.push(Add64Event::generate_event(
            self,
            cum_sums[1],
            aux_sums[3] << 24,
        ));
        trace.mulu.push(new_mulu_event);

        Ok(())
    }

    fn generate_mul(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,

        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_mul_event = SignedMulEvent::generate_event(
            self,
            trace,
            dst,
            src1,
            src2,
            field_pc,
            SignedMulKind::Mul,
        )?;

        trace.signed_mul.push(new_mul_event);

        Ok(())
    }

    fn generate_mulsu(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let new_mulsu_event = SignedMulEvent::generate_event(
            self,
            trace,
            dst,
            src1,
            src2,
            field_pc,
            SignedMulKind::Mulsu,
        )?;

        trace.signed_mul.push(new_mulsu_event);

        Ok(())
    }

    fn generate_b32_mul(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
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
        dst: BinaryField16b,
        src: BinaryField16b,
        imm_low: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        if self.pc as usize > trace.prom().len() {
            return Err(InterpreterError::BadPc);
        }
        let [second_opcode, imm_high, third, fourth] = trace.prom()[self.pc as usize].instruction;

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
        dst: BinaryField16b,
        offset: BinaryField16b,
        src: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let opt_new_mvvw_event =
            MVVWEvent::generate_event(self, trace, dst, offset, src, field_pc)?;
        if let Some(new_mvvw_event) = opt_new_mvvw_event {
            trace.mvvw.push(new_mvvw_event);
        }

        Ok(())
    }

    fn generate_mvvl(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        dst: BinaryField16b,
        offset: BinaryField16b,
        src: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let opt_new_mvvl_event =
            MVVLEvent::generate_event(self, trace, dst, offset, src, field_pc)?;
        if let Some(new_mvvl_event) = opt_new_mvvl_event {
            trace.mvvl.push(new_mvvl_event);
        }

        Ok(())
    }

    fn generate_mvih(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
        dst: BinaryField16b,
        offset: BinaryField16b,
        imm: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let opt_new_mvih_event =
            MVIHEvent::generate_event(self, trace, dst, offset, imm, field_pc)?;
        if let Some(new_mvih_event) = opt_new_mvih_event {
            trace.mvih.push(new_mvih_event);
        }

        Ok(())
    }

    fn generate_ldi(
        &mut self,
        trace: &mut ZCrayTrace,
        field_pc: BinaryField32b,
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

    pub(crate) fn allocate_new_frame(
        &self,
        trace: &mut ZCrayTrace,
        target: BinaryField32b,
    ) -> Result<u32, InterpreterError> {
        let frame_size = self
            .frames
            .get(&target)
            .ok_or(InterpreterError::InvalidInput)?;
        Ok(trace.vrom_mut().allocate_new_frame(*frame_size as u32))
    }
}

#[cfg(test)]
mod tests {
    use num_traits::WrappingAdd;

    use super::*;
    use crate::parser::parse_program;
    use crate::util::get_binary_slot;
    use crate::util::{collatz_orbits, init_logger};

    pub(crate) fn code_to_prom(code: &[Instruction]) -> ProgramRom {
        let mut prom = ProgramRom::new();
        let mut pc = BinaryField32b::ONE; // we start at PC = 1G.
        for (i, &instruction) in code.iter().enumerate() {
            let interp_inst = InterpreterInstruction::new(instruction, pc);
            prom.push(interp_inst);
            pc *= G;
        }

        prom
    }

    #[test]
    fn test_zcray() {
        let zero = BinaryField16b::zero();
        let code = vec![[Opcode::Ret.get_field_elt(), zero, zero, zero]];
        let prom = code_to_prom(&code);
        let memory = Memory::new(prom, ValueRom::new_with_init_vals(&[0, 0]));

        let mut frames = HashMap::new();
        frames.insert(BinaryField32b::ONE, 12);

        let (trace, boundary_values) =
            ZCrayTrace::generate(memory, frames, HashMap::new()).expect("Ouch!");
        trace.validate(boundary_values);
    }

    #[test]
    fn test_compiled_collatz() {
        //     collatz:
        //     ;; Frame:
        //     ;; Slot @0: Return PC
        //     ;; Slot @1: Return FP
        //     ;; Slot @2: Arg: n
        //     ;; Slot @3: Return value
        //     ;; Slot @4: ND Local: Next FP
        //     ;; Slot @5: Local: n == 1
        //     ;; Slot @6: Local: n % 2
        //     ;; Slot @7: Local: n >> 1 or 3*n + 1
        //     ;; Slot @8: Local: 3*n (lower 32bits)
        //     ;; Slot @9: Local 3*n (higher 32bits, unused)

        //     ;; Branch to recursion label if value in slot 2 is not 1
        //     XORI @5, @2, #1
        //     BNZ case_recurse, @5 ;; branch if n != 1
        //     XORI @3, @2, #0
        //     RET

        // case_recurse:
        //     ANDI @6, @2, #1  ;; n % 2 is & 0x00..01
        //     BNZ case_odd, @6 ;; branch if n % 2 == 1u32

        //     ;; case even
        //     ;; n >> 1
        //     SRLI @7, @2, #1
        //     MVV.W @4[2], @7
        //     MVV.W @4[3], @3
        //     TAILI collatz, @4

        // case_odd:
        //     MULI @8, @2, #3
        //     ADDI @7, @8, #1
        //     MVV.W @4[2], @7
        //     MVV.W @4[3], @3
        //     TAILI collatz, @4

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
                get_binary_slot(5),
                get_binary_slot(2),
                get_binary_slot(1),
            ], //  0G: XORI @5, @2, #1
            [
                Opcode::Bnz.get_field_elt(),
                get_binary_slot(5),
                case_recurse[0],
                case_recurse[1],
            ], //  1G: BNZ @5, case_recurse,
            // case_return:
            [
                Opcode::Xori.get_field_elt(),
                get_binary_slot(3),
                get_binary_slot(2),
                zero,
            ], //  2G: XORI @3, @2, #0
            [Opcode::Ret.get_field_elt(), zero, zero, zero], //  3G: RET
            // case_recurse:
            [
                Opcode::Andi.get_field_elt(),
                get_binary_slot(6),
                get_binary_slot(2),
                get_binary_slot(1),
            ], // 4G: ANDI @6, @2, #1
            [
                Opcode::Bnz.get_field_elt(),
                get_binary_slot(6),
                case_odd[0],
                case_odd[1],
            ], //  5G: BNZ @6, case_odd
            // case_even:
            [
                Opcode::Srli.get_field_elt(),
                get_binary_slot(7),
                get_binary_slot(2),
                get_binary_slot(1),
            ], //  6G: SRLI @7, @2, #1
            [
                Opcode::Mvvw.get_field_elt(),
                get_binary_slot(4),
                get_binary_slot(2),
                get_binary_slot(7),
            ], //  7G: MVV.W @4[2], @7
            [
                Opcode::Mvvw.get_field_elt(),
                get_binary_slot(4),
                get_binary_slot(3),
                get_binary_slot(3),
            ], //  8G: MVV.W @4[3], @3
            [
                Opcode::Taili.get_field_elt(),
                collatz,
                zero,
                get_binary_slot(4),
            ], // 9G: TAILI collatz, @4
            // case_odd:
            [
                Opcode::Muli.get_field_elt(),
                get_binary_slot(8),
                get_binary_slot(2),
                get_binary_slot(3),
            ], //  10G: MULI @8, @2, #3
            [
                Opcode::Addi.get_field_elt(),
                get_binary_slot(7),
                get_binary_slot(8),
                get_binary_slot(1),
            ], //  11G: ADDI @7, @8, #1
            [
                Opcode::Mvvw.get_field_elt(),
                get_binary_slot(4),
                get_binary_slot(2),
                get_binary_slot(7),
            ], //  12G: MVV.W @4[2], @7
            [
                Opcode::Mvvw.get_field_elt(),
                get_binary_slot(4),
                get_binary_slot(3),
                get_binary_slot(3),
            ], //  13G: MVV.W @4[3], @3
            [
                Opcode::Taili.get_field_elt(),
                collatz,
                zero,
                get_binary_slot(4),
            ], //  14G: TAILI collatz, @4
        ];
        let initial_val = 5;
        let (expected_evens, expected_odds) = collatz_orbits(initial_val);

        let prom = code_to_prom(&instructions);
        // return PC = 0, return FP = 0, n = 5
        let vrom = ValueRom::new_with_init_vals(&[0, 0, initial_val]);

        let memory = Memory::new(prom, vrom);

        // TODO: We could build this with compiler hints.
        let mut frames_args_size = HashMap::new();
        frames_args_size.insert(BinaryField32b::ONE, 10);

        let (traces, boundary_values) =
            ZCrayTrace::generate(memory, frames_args_size, pc_field_to_int)
                .expect("Trace generation should not fail.");

        traces.validate(boundary_values);

        assert!(
            traces.shifts.len() == expected_evens.len(),
            "Generated an incorrect number of even cases."
        );
        for (i, &even) in expected_evens.iter().enumerate() {
            assert!(
                traces.shifts[i].src_val == even,
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
                traces.get_vrom_u32(i as u32 * 16 + 4).unwrap(), // next_fp (slot 4)
                ((i + 1) * 16) as u32                            // next_fp_val
            );
            assert_eq!(
                traces.get_vrom_u32(i as u32 * 16 + 2).unwrap(), // n (slot 2)
                cur_val                                          // n_val
            );

            if cur_val % 2 == 0 {
                cur_val /= 2;
            } else {
                cur_val = 3 * cur_val + 1;
            }
        }

        // Check return value.
        assert_eq!(traces.get_vrom_u32(3).unwrap(), 1);
    }
}
