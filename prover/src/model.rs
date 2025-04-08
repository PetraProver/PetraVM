//! Data models for the zCrayVM proving system.
//!
//! This module contains the data structures used to represent execution traces
//! and events needed for the proving system.

use anyhow::Result;
use binius_m3::builder::B32;
use zcrayvm_assembly::{
    B32MulEvent, BnzEvent, BzEvent, InterpreterInstruction, LDIEvent, Opcode, RetEvent, ZCrayTrace,
};

/// Macro to generate event accessors
macro_rules! impl_event_accessor {
    ($name:ident, $event_type:ty, $field:ident) => {
        impl Trace {
            /// Returns a reference to the $name events from the trace.
            ///
            /// These events represent each $name instruction executed during the trace.
            pub fn $name(&self) -> &Vec<$event_type> {
                &self.trace.$field
            }
        }
    };
}

/// High-level representation of a zCrayVM instruction with its PC and
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
}

impl From<InterpreterInstruction> for Instruction {
    fn from(instr: InterpreterInstruction) -> Self {
        // Extract arguments from the interpreter instruction
        let args_array = instr.args();

        Self {
            pc: instr.field_pc,
            opcode: instr.opcode(),
            args: args_array.iter().map(|arg| arg.val()).collect(),
        }
    }
}

/// Execution trace containing a program and all execution events.
///
/// This is a wrapper around ZCrayTrace that provides a simplified interface
/// for the proving system. It contains:
/// 1. The program instructions in a format optimized for the prover
/// 2. The original ZCrayTrace with all execution events and memory state
/// 3. A list of VROM writes (address, value) pairs
#[derive(Debug)]
pub struct Trace {
    /// The underlying ZCrayTrace containing all execution events
    pub trace: ZCrayTrace,
    /// Program instructions in a more convenient format for the proving system
    pub program: Vec<Instruction>,
    /// List of VROM writes (address, value, multiplicity) pairs
    pub vrom_writes: Vec<(u32, u32, u32)>,
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
            trace: ZCrayTrace::default(),
            program: Vec::new(),
            vrom_writes: Vec::new(),
        }
    }

    /// Creates a Trace from an existing ZCrayTrace.
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
    pub fn from_zcray_trace(trace: ZCrayTrace) -> Self {
        Self {
            trace,
            program: Vec::new(),
            vrom_writes: Vec::new(),
        }
    }

    /// Add an interpreter instruction to the program.
    ///
    /// This converts the interpreter instruction to our simplified format.
    pub fn add_instruction(&mut self, instr: InterpreterInstruction) {
        self.program.push(instr.into());
    }

    /// Add multiple interpreter instructions to the program.
    ///
    /// # Arguments
    /// * `instructions` - An iterator of InterpreterInstructions to add
    pub fn add_instructions<I>(&mut self, instructions: I)
    where
        I: IntoIterator<Item = InterpreterInstruction>,
    {
        for instr in instructions {
            self.add_instruction(instr);
        }
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

    /// Ensures the trace has enough data for proving.
    ///
    /// This will verify that:
    /// 1. The program has at least one instruction
    /// 2. The trace has at least one LDI event
    /// 3. The trace has at least one RET event
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

// Generate event accessors
impl_event_accessor!(ldi_events, LDIEvent, ldi);
impl_event_accessor!(ret_events, RetEvent, ret);
impl_event_accessor!(b32_mul_events, B32MulEvent, b32_mul);
impl_event_accessor!(bnz_events, BnzEvent, bnz);
impl_event_accessor!(bz_events, BzEvent, bz);
