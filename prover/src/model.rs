//! Data models for the PetraVM proving system.
//!
//! This module contains the data structures used to represent execution traces
//! and events needed for the proving system.

use std::iter::repeat_n;

use anyhow::Result;
use binius_m3::builder::B32;
use paste::paste;
use petravm_asm::{event::*, InterpreterInstruction, Opcode, PetraTrace};

use crate::table::*;

/// Implements the [`TableInfo`] trait that lifts
/// [`InstructionInfo`](petravm_asm::InstructionInfo) and maps events to
/// their corresponding field in the [`PetraTrace`], as well as corresponding
/// event accessors for the main [`Trace`].
///
/// It will also implement the mapping between an [`Opcode`] and its associated
/// [`Table`].
///
/// # Example
///
/// ```ignore
/// define_table_registry_and_accessors!(
///     (ldi, Ldi),
///     (ret, Ret),
/// );
/// ```
macro_rules! define_table_registry_and_accessors {
    (
        $(($func_name:ident, $opcode_variant:ident)),* $(,)?
    ) => {
        $(
            paste! {
                impl Trace {
                    #[doc = concat!("Returns a reference to the logged `", stringify!([<$opcode_variant Event>]), "`s from the trace.")]
                    pub fn [<$func_name _events>](&self) -> &[ [<$opcode_variant Event>] ] {
                        &self.trace.$func_name
                    }
                }

                impl TableInfo for [<$opcode_variant Event>] {
                    type Table = [<$opcode_variant Table>];

                    fn accessor() -> fn(&Trace) -> &[< [<$opcode_variant Table>] as Table>::Event] {
                        Trace::[<$func_name _events>]
                    }
                }
            }
        )*

        paste! {
            pub fn build_table_for_opcode(
                opcode: Opcode,
                cs: &mut binius_m3::builder::ConstraintSystem,
                channels: &$crate::channels::Channels,
            ) -> Option<Box<dyn $crate::table::FillableTable>> {
                use $crate::table::Table;
                match opcode {
                    $(
                        Opcode::$opcode_variant => {
                            Some(Box::new($crate::table::TableEntry {
                                table: Box::new(<[<$opcode_variant Table>]>::new(cs, channels)),
                                get_events: <[<$opcode_variant Event>] as $crate::table::TableInfo>::accessor(),
                            }))
                        }
                    )*
                    _ => None,
                }
            }
        }
    };
}

/// High-level representation of a PetraVM instruction with its PC and
/// arguments.
///
/// This is a simplified representation of the instruction format used in the
/// proving system, where the arguments are stored in a more convenient form for
/// the prover.
#[derive(Debug, Clone)]
pub struct Instruction {
    /// PC value as a field element
    pub pc: B32,
    /// Opcode of the instruction
    pub opcode: Opcode,
    /// Arguments to the instruction (up to 3)
    pub args: Vec<u16>,
    /// Optional advice. Used for providing the PROM index and the discrete
    /// logarihm in base `B32::MULTIPLICATIVE_GENERATOR` of a group element
    /// defined by the instruction arguments.
    pub advice: Option<(u32, u32)>,
}

impl From<InterpreterInstruction> for Instruction {
    fn from(instr: InterpreterInstruction) -> Self {
        // Extract arguments from the interpreter instruction
        let args_array = instr.args();

        Self {
            pc: instr.field_pc,
            opcode: instr.opcode(),
            args: args_array.iter().map(|arg| arg.val()).collect(),
            advice: instr.advice,
        }
    }
}

/// Execution trace containing a program and all execution events.
///
/// This is a wrapper around PetraTrace that provides a simplified interface
/// for the proving system. It contains:
/// 1. The program instructions in a format optimized for the prover
/// 2. The original PetraTrace with all execution events and memory state
/// 3. A list of VROM writes (address, value) pairs
#[derive(Debug)]
pub struct Trace {
    /// The underlying PetraTrace containing all execution events
    pub trace: PetraTrace,
    /// Program instructions in a more convenient format for the proving system
    pub program: Vec<(Instruction, u32)>,
    /// List of VROM writes (address, value, multiplicity) pairs
    pub vrom_writes: Vec<(u32, u32, u32)>,
    /// Maximum VROM address in the trace
    pub max_vrom_addr: usize,
}

impl Default for Trace {
    fn default() -> Self {
        Self::new()
    }
}

impl Trace {
    /// Creates a new empty execution trace.
    pub fn new() -> Self {
        Self {
            trace: PetraTrace::default(),
            program: Vec::new(),
            vrom_writes: Vec::new(),
            max_vrom_addr: 0,
        }
    }

    /// Creates a Trace from an existing PetraTrace.
    ///
    /// This is useful when you have a trace from the interpreter and want
    /// to convert it to the proving format.
    ///
    /// Note: This creates an empty program vector. You'll need to populate
    /// the program instructions separately using add_instructions().
    ///
    /// TODO: Refactor this approach to directly obtain the zkVMTrace from
    /// program emulation rather than requiring separate population of
    /// program instructions.
    pub fn from_petra_trace(program: Vec<InterpreterInstruction>, mut trace: PetraTrace) -> Self {
        // Pad instruction_counter
        let instruction_counter_size = trace.instruction_counter.len();
        trace
            .instruction_counter
            .extend(repeat_n(0, program.len() - instruction_counter_size));

        // Add the program instructions to the trace
        let mut zkvm_trace = Self::new();
        zkvm_trace.add_instructions(program, &trace.instruction_counter);

        zkvm_trace.trace = trace;

        zkvm_trace
    }

    /// Add multiple interpreter instructions to the program.
    ///
    /// Instructions are added in descending order of their execution count.
    ///
    /// # Arguments
    /// * `instructions` - An iterator of InterpreterInstructions to add
    pub fn add_instructions(
        &mut self,
        instructions: Vec<InterpreterInstruction>,
        instruction_counter: &[u32],
    ) {
        // Collect all instructions with their counts
        let mut instructions_with_counts = instructions
            .into_iter()
            .zip(instruction_counter.iter())
            .map(|(instr, count)| (instr.into(), *count))
            .collect::<Vec<_>>();

        // Sort by count in descending order
        instructions_with_counts.sort_by(|(_, count_a), (_, count_b)| count_b.cmp(count_a));

        // Add instructions in sorted order
        self.program = instructions_with_counts;
    }

    /// Add a VROM write event.
    ///
    /// # Arguments
    /// * `addr` - The address to write to
    /// * `value` - The value to write
    /// * `multiplicity` - The multiplicity of pulls of this VROM write
    pub fn add_vrom_write(&mut self, addr: u32, value: u32, multiplicity: u32) {
        self.vrom_writes.push((addr, value, multiplicity));
    }

    /// Returns a reference to the right shift events from the trace.
    pub fn right_shift_events(&self) -> &[RightLogicShiftGadgetEvent] {
        &self.trace.right_logic_shift_gadget
    }

    /// Ensures the trace has enough data for proving.
    ///
    /// This will verify that:
    /// 1. The program has at least one instruction
    /// 2. The trace has at least one RET event
    /// 3. The trace has at least one VROM write
    ///
    /// # Returns
    /// * Ok(()) if the trace is valid, or an error with a description of what's
    ///   missing
    pub fn validate(&self) -> Result<()> {
        if self.program.is_empty() {
            return Err(anyhow::anyhow!(
                "Trace must contain at least one instruction"
            ));
        }

        if self.ret_events().is_empty() {
            return Err(anyhow::anyhow!("Trace must contain at least one RET event"));
        }

        if self.vrom_writes.is_empty() {
            return Err(anyhow::anyhow!(
                "Trace must contain at least one VROM write"
            ));
        }

        Ok(())
    }
}

// Generate event accessors and table info.
define_table_registry_and_accessors!(
    (ldi, Ldi),
    (ret, Ret),
    (bz, Bz),
    (bnz, Bnz),
    (fp, Fp),
    (b32_mul, B32Mul),
    (b32_muli, B32Muli),
    (b128_add, B128Add),
    (b128_mul, B128Mul),
    (andi, Andi),
    (xori, Xori),
    (add, Add),
    (addi, Addi),
    (sub, Sub),
    (mulu, Mulu),
    (mul, Mul),
    (muli, Muli),
    (mulsu, Mulsu),
    (taili, Taili),
    (tailv, Tailv),
    (calli, Calli),
    (callv, Callv),
    (mvvw, Mvvw),
    (mvih, Mvih),
    (mvvl, Mvvl),
    (and, And),
    (xor, Xor),
    (or, Or),
    (ori, Ori),
    (jumpi, Jumpi),
    (jumpv, Jumpv),
    (srli, Srli),
    (slli, Slli),
    (srl, Srl),
    (sll, Sll),
    (srai, Srai),
    (sra, Sra),
    (sltu, Sltu),
    (slt, Slt),
    (slti, Slti),
    (sle, Sle),
    (slei, Slei),
    (sltiu, Sltiu),
    (sleu, Sleu),
    (sleiu, Sleiu),
);
