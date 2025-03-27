//! Tables for the zCrayVM M3 circuit.
//!
//! This module contains the definitions of all the arithmetic tables needed
//! to represent the zCrayVM execution in the M3 arithmetization system.

use binius_field::as_packed_field::PackScalar;
use binius_m3::builder::{
    B32, ConstraintSystem, TableId, TableFiller, TableWitnessIndexSegment, Col
};
use bytemuck::Pod;

use crate::{
    channels::ZkVMChannels,
    model::Instruction,
};

// Re-export instruction-specific tables
pub use crate::opcodes::{RetTable, LdiTable};

/// PROM (Program ROM) table for storing program instructions.
///
/// This table stores all the instructions in the program and makes them
/// available to the instruction-specific tables.
///
/// Format: [PC, Opcode, Arg1, Arg2, Arg3]
pub struct PromTable {
    /// Table ID
    pub id: TableId,
    /// PC column
    pub pc: Col<B32, 1>,
    /// Opcode column
    pub opcode: Col<B32, 1>,
    /// Argument 1 column
    pub arg1: Col<B32, 1>,
    /// Argument 2 column
    pub arg2: Col<B32, 1>,
    /// Argument 3 column
    pub arg3: Col<B32, 1>,
}

impl PromTable {
    /// Create a new PROM table with the given constraint system and channels.
    ///
    /// # Arguments
    /// * `cs` - Constraint system to add the table to
    /// * `channels` - Channel IDs for communication with other tables
    pub fn new(cs: &mut ConstraintSystem, channels: &ZkVMChannels) -> Self {
        let mut table = cs.add_table("prom");
        
        // Add columns for PC and instruction components
        let pc = table.add_committed("pc");
        let opcode = table.add_committed("opcode");
        let arg1 = table.add_committed("arg1");
        let arg2 = table.add_committed("arg2");
        let arg3 = table.add_committed("arg3");
        
        // Push to the PROM channel
        table.push(channels.prom_channel, [pc, opcode, arg1, arg2, arg3]);
        
        Self {
            id: table.id(),
            pc,
            opcode,
            arg1,
            arg2,
            arg3,
        }
    }
}

impl<U> TableFiller<U> for PromTable
where
    U: Pod + PackScalar<B32>,
{
    type Event = Instruction;
    
    fn id(&self) -> TableId {
        self.id
    }
    
    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event>,
        witness: &'a mut TableWitnessIndexSegment<U>,
    ) -> anyhow::Result<()> {
        let mut pc_col = witness.get_mut_as(self.pc)?;
        let mut opcode_col = witness.get_mut_as(self.opcode)?;
        let mut arg1_col = witness.get_mut_as(self.arg1)?;
        let mut arg2_col = witness.get_mut_as(self.arg2)?;
        let mut arg3_col = witness.get_mut_as(self.arg3)?;
        
        for (i, instr) in rows.enumerate() {
            pc_col[i] = instr.pc;
            opcode_col[i] = instr.opcode as u16 as u32;
            
            // Fill arguments, using 0 if the argument doesn't exist
            arg1_col[i] = instr.args.get(0).copied().unwrap_or(0) as u32;
            arg2_col[i] = instr.args.get(1).copied().unwrap_or(0) as u32;
            arg3_col[i] = instr.args.get(2).copied().unwrap_or(0) as u32;
        }
        
        Ok(())
    }
}

/// VROM (Value ROM) table for storing memory values.
///
/// This table models the zCrayVM's memory, storing address-value pairs
/// for all memory accesses.
///
/// Format: [Address, Value]
pub struct VromTable {
    /// Table ID
    pub id: TableId,
    /// Address column
    pub addr: Col<B32, 1>,
    /// Value column
    pub value: Col<B32, 1>,
}

impl VromTable {
    /// Create a new VROM table with the given constraint system and channels.
    ///
    /// # Arguments
    /// * `cs` - Constraint system to add the table to
    /// * `channels` - Channel IDs for communication with other tables
    pub fn new(cs: &mut ConstraintSystem, channels: &ZkVMChannels) -> Self {
        let mut table = cs.add_table("vrom");
        
        // Add columns for address and value
        let addr = table.add_committed("addr");
        let value = table.add_committed("value");
        
        // Connect to VROM channel
        table.pull(channels.vrom_channel, [addr, value]);
        
        Self {
            id: table.id(),
            addr,
            value,
        }
    }
}