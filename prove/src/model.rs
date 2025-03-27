//! Data models for the zCrayVM proving system.
//!
//! This module contains the data structures used to represent execution traces
//! and events needed for the proving system.

use binius_field::BinaryField32b;
use zcrayvm_assembly::Opcode;

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

/// Represents a load immediate (LDI) instruction event.
#[derive(Debug, Clone)]
pub struct LdiEvent {
    /// PC value
    pub pc: BinaryField32b,
    /// Frame pointer
    pub fp: u32,
    /// Destination register
    pub dst: u16,
    /// Immediate value to load
    pub imm: u32,
}

/// Represents a return (RET) instruction event.
#[derive(Debug, Clone)]
pub struct RetEvent {
    /// PC value
    pub pc: BinaryField32b,
    /// Frame pointer
    pub fp: u32,
    /// Value at frame pointer offset 0 (return PC)
    pub fp_0_val: u32,
    /// Value at frame pointer offset 1 (caller's frame pointer)
    pub fp_1_val: u32,
}

/// Execution trace containing a program and all execution events.
#[derive(Debug, Default)]
pub struct ZkVMTrace {
    /// Program instructions
    pub program: Vec<Instruction>,
    /// LDI instruction events
    pub ldi_events: Vec<LdiEvent>,
    /// RET instruction events
    pub ret_events: Vec<RetEvent>,
    // More event types will be added as we implement more opcodes
}

impl ZkVMTrace {
    /// Creates a new empty execution trace.
    pub fn new() -> Self {
        Self::default()
    }
}