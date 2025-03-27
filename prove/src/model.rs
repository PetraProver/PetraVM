//! Data models for the zCrayVM proving system.
//!
//! This module contains the data structures used to represent execution traces
//! and events needed for the proving system.

use binius_field::BinaryField32b;
use zcrayvm_assembly::{
    Opcode,
    InterpreterInstruction,
    ZCrayTrace,
    LDIEvent,
    RetEvent,
};

/// High-level representation of a zCrayVM instruction with its PC and arguments.
#[derive(Debug, Clone)]
pub struct Instruction {
    /// PC value as a field element
    pub pc: BinaryField32b,
    /// Opcode of the instruction
    pub opcode: Opcode,
    /// Arguments to the instruction (up to 3)
    pub args: Vec<u16>,
}

impl From<InterpreterInstruction> for Instruction {
    fn from(instr: InterpreterInstruction) -> Self {
        let args_array = instr.args();
        Self {
            pc: instr.field_pc,
            opcode: instr.opcode(),
            args: args_array.iter().map(|arg| arg.val()).collect(),
        }
    }
}

/// Execution trace containing a program and all execution events.
/// This is a wrapper around ZCrayTrace that provides a simplified interface
/// for the proving system.
#[derive(Debug)]
pub struct ZkVMTrace {
    /// The underlying ZCrayTrace
    pub trace: ZCrayTrace,
    /// Program instructions in a more convenient format for the proving system
    pub program: Vec<Instruction>,
}

impl ZkVMTrace {
    /// Creates a new empty execution trace.
    pub fn new() -> Self {
        Self {
            trace: ZCrayTrace::default(),
            program: Vec::new(),
        }
    }
    
    /// Creates a ZkVMTrace from an existing ZCrayTrace.
    pub fn from_zcray_trace(trace: ZCrayTrace) -> Self {
        Self { 
            trace,
            program: Vec::new(),
        }
    }
    
    /// Returns a reference to the LDI events.
    pub fn ldi_events(&self) -> &Vec<LDIEvent> {
        &self.trace.ldi
    }
    
    /// Returns a reference to the RET events.
    pub fn ret_events(&self) -> &Vec<RetEvent> {
        &self.trace.ret
    }
    
    /// Add an interpreter instruction to the program.
    pub fn add_instruction(&mut self, instr: InterpreterInstruction) {
        self.program.push(instr.into());
    }
    
    /// Add multiple interpreter instructions to the program.
    pub fn add_instructions<I>(&mut self, instructions: I)
    where
        I: IntoIterator<Item = InterpreterInstruction>
    {
        for instr in instructions {
            self.add_instruction(instr);
        }
    }
}