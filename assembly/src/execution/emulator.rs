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
use tracing::instrument;

use crate::{
    assembler::LabelsFrameSizes,
    context::EventContext,
    execution::{PetraTrace, StateChannel},
    isa::{GenericISA, ISA},
    memory::{Memory, MemoryError},
    mv::MVInfo,
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

/// Main program executor, used to build a [`PetraTrace`] from a program's PROM.
///
/// The interpreter manages control flow, memory accesses, instruction execution
/// and state updates.
#[derive(Debug)]
pub struct Interpreter {
    /// The Instruction Set Architecture [`ISA`] to be supported for this
    /// [`Interpreter`] instance.
    pub isa: Box<dyn ISA>,
    /// The integer PC represents to the exponent of the actual field
    /// PC (which starts at `B32::ONE` and iterate over the
    /// multiplicative group). Since we need to have a value for 0 as well
    /// (which is not in the multiplicative group), we shift all powers by
    /// 1, and 0 can be the halting value.
    pub(crate) pc: u32,
    pub(crate) prom_index: u32,
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
    // and their associated PROM index and integer PC.
    pc_field_to_index_pc: HashMap<B32, (u32, u32)>,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self {
            isa: Box::new(GenericISA),
            pc: 1, // default starting value for PC
            prom_index: 0,
            fp: FramePointer(0),
            timestamp: 0,
            frames: HashMap::new(),
            pc_field_to_index_pc: HashMap::new(),
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
    /// Optional advice. Used for providing the PROM index and the discrete
    /// logarithm in base `B32::MULTIPLICATIVE_GENERATOR` of some group
    /// element defined by the instruction arguments.
    pub advice: Option<(u32, u32)>,
    pub prover_only: bool,
}

impl InterpreterInstruction {
    pub const fn new(
        instruction: Instruction,
        field_pc: B32,
        advice: Option<(u32, u32)>,
        prover_only: bool,
    ) -> Self {
        Self {
            instruction,
            field_pc,
            advice,
            prover_only,
        }
    }

    pub fn opcode(&self) -> Opcode {
        Opcode::try_from(self.instruction[0].val()).unwrap_or(Opcode::Invalid)
    }

    /// Get the arguments of this instruction.
    pub const fn args(&self) -> [B16; 3] {
        [
            self.instruction[1],
            self.instruction[2],
            self.instruction[3],
        ]
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InterpreterError {
    #[error("The opcode is not a valid one.")]
    InvalidOpcode,
    #[error("The opcode {0} is not supported by this instruction set.")]
    UnsupportedOpcode(Opcode),
    #[error("The Program Counter is incorrect.")]
    BadPc,
    #[error("The arguments to this opcode are invalid.")]
    InvalidInput,
    #[error("A memory access failed with error {0}")]
    MemoryError(MemoryError),
    #[error("The instruction requires an advice, but none was provided.")]
    MissingAdvice(Opcode),
    #[error("An exception occurred.")]
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
        isa: Box<dyn ISA>,
        frames: LabelsFrameSizes,
        pc_field_to_index_pc: HashMap<B32, (u32, u32)>,
    ) -> Self {
        Self {
            isa,
            pc: 1,
            prom_index: 0,
            fp: FramePointer(0),
            timestamp: 0,
            frames,
            pc_field_to_index_pc,
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
    pub(crate) const fn incr_prom_index(&mut self) {
        self.prom_index += 1;
    }

    #[inline(always)]
    /// Jump to a specific target in the PROM, given as a field element
    pub(crate) fn jump_to(&mut self, target: B32) {
        if target == B32::zero() {
            self.pc = 0;
        } else {
            let (prom_index, pc) = *self
                .pc_field_to_index_pc
                .get(&target)
                .expect("This target should have been parsed.");
            debug_assert!(G.pow(pc as u64 - 1) == target);
            self.prom_index = prom_index;
            self.pc = pc;
        }
    }

    #[inline(always)]
    /// Jump to a specific target in the PROM given as the discrete
    /// logarithm of the field pc.
    pub(crate) fn jump_to_u32(&mut self, target: B32, advice: (u32, u32)) {
        let (prom_index, pc) = advice;
        debug_assert!(
            target == B32::MULTIPLICATIVE_GENERATOR.pow((pc - 1) as u64),
            "The advice must be the discrete logarithm of the target address in base `B32::MULTIPLICATIVE_GENERATOR`"
        );
        self.prom_index = prom_index;
        self.pc = pc;
    }

    #[inline(always)]
    pub(crate) const fn is_halted(&self) -> bool {
        self.pc == 0 // The real PC should be 0, which is outside of the
    }

    #[instrument(level = "info", skip_all)]
    pub fn run(&mut self, memory: Memory) -> Result<PetraTrace, InterpreterError> {
        let mut trace = PetraTrace::new(memory);

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
                            panic!("{critical_error:?}");
                        } //TODO: properly format error
                    }
                }
            }
            if self.is_halted() {
                return Ok(trace);
            }
        }
    }

    pub fn step(&mut self, trace: &mut PetraTrace) -> Result<(), InterpreterError> {
        if (self.prom_index as usize >= trace.prom().len())
            || (self.pc as usize - 1 > trace.prom().len())
        {
            return Err(InterpreterError::BadPc);
        }
        let InterpreterInstruction {
            instruction,
            field_pc,
            advice,
            prover_only,
        } = trace.prom()[self.prom_index as usize];
        let [opcode, arg0, arg1, arg2] = instruction;
        if !prover_only {
            trace.record_instruction(field_pc);
            // Special handling for B32Muli
            if opcode == Opcode::B32Muli.get_field_elt() {
                let next_pc = trace.prom()[self.prom_index as usize + 1].field_pc;
                trace.record_instruction(next_pc);
            }
        }

        debug_assert_eq!(field_pc, G.pow(self.pc as u64 - 1));

        let opcode = Opcode::try_from(opcode.val()).map_err(|_| InterpreterError::InvalidOpcode)?;
        #[cfg(debug_assertions)]
        {
            if !self.isa.is_supported(opcode) {
                return Err(InterpreterError::UnsupportedOpcode(opcode));
            }
            if opcode.is_verifier_only() && prover_only {
                panic!("{opcode:?} cannot be prover-only.");
            }
            if (opcode == Opcode::Alloci || opcode == Opcode::Allocv) && !prover_only {
                panic!("{opcode:?} must be prover-only.");
            }
        }

        let mut ctx = EventContext {
            interpreter: self,
            trace,
            field_pc,
            advice,
            prover_only,
        };

        opcode.generate_event(&mut ctx, arg0, arg1, arg2)
    }

    pub(crate) fn allocate_new_frame(
        &self,
        trace: &mut PetraTrace,
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
    use crate::test_util::{code_to_prom, collatz_orbits, get_binary_slot};
    use crate::util::init_logger;
    use crate::ValueRom;

    #[test]
    fn test_petra() {
        let zero = B16::zero();
        let code = vec![[Opcode::Ret.get_field_elt(), zero, zero, zero]];
        let prom = code_to_prom(&code);
        let memory = Memory::new(prom, ValueRom::new_with_init_vals(&[0, 0]));

        let mut frames = HashMap::new();
        frames.insert(B32::ONE, 12);

        let (trace, boundary_values) =
            PetraTrace::generate(Box::new(GenericISA), memory, frames, HashMap::new())
                .expect("Ouch!");
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
        // labels with their corresponding discrete logarithms
        let collatz_prom_index = 0;
        let collatz_advice = 1;
        let collatz = B16::ONE;
        let case_recurse_prom_index = 4;
        let case_recurse_advice = 5;
        let case_recurse =
            ExtensionField::<B16>::iter_bases(&G.pow((case_recurse_advice - 1) as u64))
                .collect::<Vec<B16>>();
        let case_odd_prom_index = 10;
        let case_odd_advice = 11;
        let case_odd = ExtensionField::<B16>::iter_bases(&G.pow((case_odd_advice - 1) as u64))
            .collect::<Vec<B16>>();

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
                case_recurse[0],
                case_recurse[1],
                get_binary_slot(5),
            ], //  1G: BNZ case_recurse, @5
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
                case_odd[0],
                case_odd[1],
                get_binary_slot(6),
            ], //  5G: BNZ case_odd, @6
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

        let mut prom = code_to_prom(&instructions);
        // Set the expected advice for BNZ
        prom[1].advice = Some((case_recurse_prom_index, case_recurse_advice));
        // Set the expected advice for the second BNZ
        prom[5].advice = Some((case_odd_prom_index, case_odd_advice));
        // Set the expected advice for the first TAILI
        prom[9].advice = Some((collatz_prom_index, collatz_advice));
        // Set the expected advice for the second TAILI
        prom[14].advice = Some((collatz_prom_index, collatz_advice));

        // return PC = 0, return FP = 0, n = 5
        let vrom = ValueRom::new_with_init_vals(&[0, 0, initial_val]);

        let memory = Memory::new(prom, vrom);

        // TODO: We could build this with compiler hints.
        let mut frames_args_size = HashMap::new();
        frames_args_size.insert(B32::ONE, 10);

        let (traces, boundary_values) = PetraTrace::generate(
            Box::new(GenericISA),
            memory,
            frames_args_size,
            HashMap::new(), // We only need the advice for the instructions
        )
        .expect("Trace generation should not fail.");

        traces.validate(boundary_values);

        assert!(
            traces.srli.len() == expected_evens.len(),
            "Generated an incorrect number of even cases."
        );
        for (i, &even) in expected_evens.iter().enumerate() {
            assert!(
                traces.srli[i].src_val == even,
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
