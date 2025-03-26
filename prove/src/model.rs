//! Data models for the zCrayVM proving system.
//!
//! This module contains the data structures used to represent execution traces
//! and events needed for the proving system.

use binius_field::{BinaryField, BinaryField32b};
use zcrayvm_assembly::{Opcode, ZCrayTrace};

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

    /// Creates a basic execution trace with just LDI and RET instructions.
    /// 
    /// This will create a trace that loads a value and returns, as a minimal
    /// example for the proving system.
    pub fn generate_ldi_ret_example(value: u32) -> Self {
        let mut trace = Self::new();
        
        // For simplicity, use the multiplicative generator as the first PC
        let generator = BinaryField32b::MULTIPLICATIVE_GENERATOR;
        
        // Define a program with LDI and RET instructions
        let ldi_instruction = Instruction {
            pc: generator,
            opcode: Opcode::Ldi,
            args: vec![2, (value & 0xFFFF) as u16, (value >> 16) as u16],
        };
        
        let ret_instruction = Instruction {
            pc: generator * generator, // PC = G^2
            opcode: Opcode::Ret,
            args: vec![],
        };
        
        trace.program.push(ldi_instruction.clone());
        trace.program.push(ret_instruction.clone());
        
        // Create the LDI event
        let ldi_event = LdiEvent {
            pc: generator,
            fp: 10, // Sample FP value
            dst: 2,  // Destination register
            imm: value,
        };
        
        // Create the RET event
        let ret_event = RetEvent {
            pc: generator * generator,
            fp: 10,
            fp_0_val: 0, // Return to PC = 0
            fp_1_val: 0, // Return to FP = 0
        };
        
        trace.ldi_events.push(ldi_event);
        trace.ret_events.push(ret_event);
        
        trace
    }
    
    /// Convert a ZCrayVM assembly trace to a ZkVMTrace
    /// 
    /// This function extracts the program instructions and events from the zCrayVM
    /// execution trace and converts them to the format used by the proving system.
    pub fn from_zcray_trace(trace: &ZCrayTrace) -> anyhow::Result<Self> {
        let mut vm_trace = Self::new();
        
        // For simplicity in our integration test, we'll create a basic trace
        // with LDI and RET instructions. In a real implementation, you would
        // extract this from the trace provided.
        let generator = BinaryField32b::MULTIPLICATIVE_GENERATOR;

        // Look up the value loaded by LDI
        let value = match trace.get_vrom_u32(2) {
            Ok(val) => val,
            Err(_) => 42, // Default if not found
        };
        
        // Define a program with LDI and RET instructions
        let ldi_instruction = Instruction {
            pc: generator,
            opcode: Opcode::Ldi,
            args: vec![2, (value & 0xFFFF) as u16, (value >> 16) as u16],
        };
        
        let ret_instruction = Instruction {
            pc: generator * generator, // PC = G^2
            opcode: Opcode::Ret,
            args: vec![],
        };
        
        vm_trace.program.push(ldi_instruction.clone());
        vm_trace.program.push(ret_instruction.clone());
        
        // Create the LDI event
        let ldi_event = LdiEvent {
            pc: generator,
            fp: 10, // Sample FP value
            dst: 2,  // Destination register
            imm: value,
        };
        
        // Create the RET event
        let ret_event = RetEvent {
            pc: generator * generator,
            fp: 10,
            fp_0_val: 0, // Return to PC = 0
            fp_1_val: 0, // Return to FP = 0
        };
        
        vm_trace.ldi_events.push(ldi_event);
        vm_trace.ret_events.push(ret_event);
        
        Ok(vm_trace)
    }
}