//! The core zkVM emulator, that executes instructions parsed from the immutable
//! Instruction Memory (PROM). It processes events and updates the machine state
//! accordingly.

use std::{
    collections::HashMap,
    fmt::Debug,
    ops::{Deref, DerefMut},
};

use binius_field::{BinaryField, PackedField};
use binius_m3::builder::{B16, B32};
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
        context::EventContext,
        integer_ops::{
            AddEvent, AddiEvent, MulEvent, MuliEvent, MulsuEvent, MuluEvent, SltEvent, SltiEvent,
            SltiuEvent, SltuEvent, SubEvent,
        },
        jump::{JumpiEvent, JumpvEvent},
        mv::{LDIEvent, MVIHEvent, MVInfo, MVVLEvent, MVVWEvent},
        ret::RetEvent,
        shift::*,
        Event,
    },
    execution::{StateChannel, ZCrayTrace},
    gadgets::Add32Gadget,
    get_last_event,
    memory::{Memory, MemoryError},
    opcodes::Opcode,
};

pub(crate) const G: B32 = B32::MULTIPLICATIVE_GENERATOR;

/// Channels used to communicate data through event execution.
#[derive(Default)]
pub struct InterpreterChannels {
    pub state_channel: StateChannel,
}

/// A wrapper around a `u32` representing the frame pointer (FP) in VROM for
/// type-safety and easy memory-address access.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FramePointer(u32);

impl FramePointer {
    /// Outputs a memory address from a provided offset.
    #[inline(always)]
    pub fn addr<T: Into<u32>>(&self, offset: T) -> u32 {
        self.0 ^ offset.into()
    }
}

impl From<u32> for FramePointer {
    fn from(fp: u32) -> Self {
        Self(fp)
    }
}

impl Deref for FramePointer {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for FramePointer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Main program executor, used to build a [`ZCrayTrace`] from a program's PROM.
///
/// The interpreter manages control flow, memory accesses, instruction execution
/// and state updates.
#[derive(Debug)]
pub struct Interpreter {
    /// The integer PC represents to the exponent of the actual field
    /// PC (which starts at `B32::ONE` and iterate over the
    /// multiplicative group). Since we need to have a value for 0 as well
    /// (which is not in the multiplicative group), we shift all powers by
    /// 1, and 0 can be the halting value.
    pub(crate) pc: u32,
    pub(crate) fp: FramePointer,
    /// The system timestamp. Only RAM operations increase it.
    pub timestamp: u32,
    frames: LabelsFrameSizes,
    /// Before a CALL, there are a few move operations used to populate the next
    /// frame. But the next frame pointer is not necessarily known at this
    /// point, and return values may also not be known. Thus, this `Vec` is
    /// used to store the move operations that need to be handled once we
    /// have enough information. Stores all move operations that should be
    /// handles during the current call procedure.
    pub moves_to_apply: Vec<MVInfo>,
    // Temporary HashMap storing the mapping between binary field elements that appear in the PROM
    // and their associated integer PC.
    pc_field_to_int: HashMap<B32, u32>,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self {
            pc: 1, // default starting value for PC
            fp: FramePointer(0),
            timestamp: 0,
            frames: HashMap::new(),
            pc_field_to_int: HashMap::new(),
            moves_to_apply: vec![],
        }
    }
}

/// An [`Instruction`] in raw form, composed of an opcode and up to three 16-bit
/// arguments to be used by this operation.
pub type Instruction = [B16; 4];

#[derive(Debug, Default, PartialEq, Clone)]
pub struct InterpreterInstruction {
    pub instruction: Instruction,
    pub field_pc: B32,
}

impl InterpreterInstruction {
    pub const fn new(instruction: Instruction, field_pc: B32) -> Self {
        Self {
            instruction,
            field_pc,
        }
    }
    pub fn opcode(&self) -> Opcode {
        Opcode::try_from(self.instruction[0].val()).unwrap_or(Opcode::Invalid)
    }

    /// Get the arguments of this instruction.
    pub fn args(&self) -> [B16; 3] {
        [
            self.instruction[1],
            self.instruction[2],
            self.instruction[3],
        ]
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
    pub(crate) const fn new(frames: LabelsFrameSizes, pc_field_to_int: HashMap<B32, u32>) -> Self {
        Self {
            pc: 1,
            fp: FramePointer(0),
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
    pub(crate) fn jump_to(&mut self, target: B32) {
        if target == B32::zero() {
            self.pc = 0;
        } else {
            self.pc = *self
                .pc_field_to_int
                .get(&target)
                .expect("This target should have been parsed.");
            debug_assert!(G.pow(self.pc as u64 - 1) == target);
        }
    }

    #[inline(always)]
    pub(crate) const fn is_halted(&self) -> bool {
        self.pc == 0 // The real PC should be 0, which is outside of the
    }

    pub fn run(&mut self, memory: Memory) -> Result<ZCrayTrace, InterpreterError> {
        let mut trace = ZCrayTrace::new(memory);

        let field_pc = trace.prom()[self.pc as usize - 1].field_pc;
        // Start by allocating a frame for the initial label.
        self.allocate_new_frame(&mut trace, field_pc)?;
        loop {
            match self.step(&mut trace) {
                Ok(_) => {}
                Err(error) => {
                    match error {
                        InterpreterError::Exception(_exc) => {} //TODO: handle exception
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
        trace!(
            "Executing {:?} with args {:?}",
            opcode,
            (1..1 + opcode.num_args())
                .map(|i| instruction.instruction[i].val())
                .collect::<Vec<_>>()
        );

        let mut ctx = EventContext {
            interpreter: self,
            trace,
            field_pc,
        };

        match opcode {
            Opcode::Bnz => BnzEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Jumpi => JumpiEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Jumpv => JumpvEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Xori => XoriEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Xor => XorEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Slli => SlliEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Srli => SrliEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Srai => SraiEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Sll => SllEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Srl => SrlEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Sra => SraEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Addi => Self::generate_addi(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Add => Self::generate_add(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Sub => SubEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Slt => SltEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Slti => SltiEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Sltu => SltuEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Sltiu => SltiuEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Muli => MuliEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Mulu => MuluEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Mulsu => MulsuEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Mul => MulEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Ret => RetEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Taili => TailiEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Tailv => TailVEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Calli => CalliEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Callv => CallvEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::And => AndEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Andi => AndiEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Or => OrEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Ori => OriEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Mvih => MVIHEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Mvvw => MVVWEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Mvvl => MVVLEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Ldi => LDIEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::B32Mul => B32MulEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::B32Muli => B32MuliEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::B128Add => B128AddEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::B128Mul => B128MulEvent::generate(&mut ctx, arg0, arg1, arg2)?,
            Opcode::Invalid => return Err(InterpreterError::InvalidOpcode),
        }
        Ok(Some(()))
    }

    fn generate_add(
        ctx: &mut EventContext,
        dst: B16,
        src1: B16,
        src2: B16,
    ) -> Result<(), InterpreterError> {
        AddEvent::generate(ctx, dst, src1, src2)?;

        // TODO(Robin): Do this directly within AddEvent / AddiEvent?

        // Retrieve event
        let new_add_event = get_last_event!(ctx, add);

        let new_add32_gadget =
            Add32Gadget::generate_gadget(ctx, new_add_event.src1_val, new_add_event.src2_val);
        ctx.trace.add32.push(new_add32_gadget);

        Ok(())
    }

    fn generate_addi(
        ctx: &mut EventContext,
        dst: B16,
        src: B16,
        imm: B16,
    ) -> Result<(), InterpreterError> {
        AddiEvent::generate(ctx, dst, src, imm)?;

        // TODO(Robin): Do this directly within AddEvent / AddiEvent?

        // Retrieve event
        let new_addi_event = get_last_event!(ctx, addi);

        let new_add32_gadget =
            Add32Gadget::generate_gadget(ctx, new_addi_event.src_val, imm.val() as u32);
        ctx.trace.add32.push(new_add32_gadget);

        Ok(())
    }

    pub(crate) fn allocate_new_frame(
        &self,
        trace: &mut ZCrayTrace,
        target: B32,
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
    use binius_field::{ExtensionField, Field};

    use super::*;
    use crate::util::{code_to_prom, get_binary_slot};
    use crate::util::{collatz_orbits, init_logger};
    use crate::ValueRom;

    #[test]
    fn test_zcray() {
        let zero = B16::zero();
        let code = vec![[Opcode::Ret.get_field_elt(), zero, zero, zero]];
        let prom = code_to_prom(&code);
        let memory = Memory::new(prom, ValueRom::new_with_init_vals(&[0, 0]));

        let mut frames = HashMap::new();
        frames.insert(B32::ONE, 12);

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

        let zero = B16::zero();
        // labels
        let collatz = B16::ONE;
        let case_recurse = ExtensionField::<B16>::iter_bases(&G.pow(4)).collect::<Vec<B16>>();
        let case_odd = ExtensionField::<B16>::iter_bases(&G.pow(10)).collect::<Vec<B16>>();

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
        frames_args_size.insert(B32::ONE, 10);

        let (traces, boundary_values) =
            ZCrayTrace::generate(memory, frames_args_size, pc_field_to_int)
                .expect("Trace generation should not fail.");

        traces.validate(boundary_values);

        assert!(
            traces.shifts.len() == expected_evens.len(),
            "Generated an incorrect number of even cases."
        );
        for (i, &even) in expected_evens.iter().enumerate() {
            let event = match traces.shifts[i].as_any() {
                AnyShiftEvent::Srli(ev) => ev,
                _ => panic!("Expected SrliEvent"),
            };

            assert!(event.src_val == even, "Incorrect input to an even case.");
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
                traces.vrom().read::<u32>(i as u32 * 16 + 4).unwrap(), // next_fp (slot 4)
                ((i + 1) * 16) as u32                                  // next_fp_val
            );
            assert_eq!(
                traces.vrom().read::<u32>(i as u32 * 16 + 2).unwrap(), // n (slot 2)
                cur_val                                                // n_val
            );

            if cur_val % 2 == 0 {
                cur_val /= 2;
            } else {
                cur_val = 3 * cur_val + 1;
            }
        }

        // Check return value.
        assert_eq!(traces.vrom().read::<u32>(3).unwrap(), 1);
    }
}
