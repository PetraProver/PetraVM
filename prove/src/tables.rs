//! Tables for the zCrayVM M3 circuit.
//!
//! This module contains the definitions of all the arithmetic tables needed
//! to represent the zCrayVM execution in the M3 arithmetization system.

use binius_field::{BinaryField, BinaryField32b, Field, as_packed_field::PackScalar};
use binius_m3::builder::{
    B32, ConstraintSystem, TableId, TableFiller, TableWitnessIndexSegment, Col
};
use bytemuck::Pod;

use crate::{
    channels::ZkVMChannels,
    model::{Instruction, LdiEvent, RetEvent},
};

/// PROM (Program ROM) table for storing program instructions.
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
    /// Create a new PROM table.
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
pub struct VromTable {
    /// Table ID
    pub id: TableId,
    /// Address column
    pub addr: Col<B32, 1>,
    /// Value column
    pub value: Col<B32, 1>,
}

impl VromTable {
    /// Create a new VROM table.
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

/// LDI (Load Immediate) table.
pub struct LdiTable {
    /// Table ID
    pub id: TableId,
    /// PC column
    pub pc: Col<B32, 1>,
    /// Frame pointer column
    pub fp: Col<B32, 1>,
    /// Destination register column
    pub dst: Col<B32, 1>,
    /// Immediate value column
    pub imm: Col<B32, 1>,
}

impl LdiTable {
    /// Create a new LDI table.
    pub fn new(cs: &mut ConstraintSystem, channels: &ZkVMChannels) -> Self {
        let mut table = cs.add_table("ldi_table");
        
        // Add columns for PC, FP, and other instruction components
        let pc = table.add_committed("pc");
        let fp = table.add_committed("fp");
        let dst = table.add_committed("dst");
        let imm = table.add_committed("imm");
        
        // Pull from state channel (get current state)
        table.pull(channels.state_channel, [pc, fp]);
        
        // Pull from PROM channel (get opcode and arguments)
        let instr_pc = table.add_committed::<B32, 1>("instr_pc");
        let instr_opcode = table.add_committed::<B32, 1>("instr_opcode");
        let instr_dst = table.add_committed::<B32, 1>("instr_dst");
        let instr_imm_low = table.add_committed::<B32, 1>("instr_imm_low");
        let instr_imm_high = table.add_committed::<B32, 1>("instr_imm_high");
        
        // Get instruction from PROM
        table.push(channels.prom_channel, [instr_pc, instr_opcode, instr_dst, instr_imm_low, instr_imm_high]);
        
        // Verify PC matches instruction PC
        table.assert_zero("pc_matches_instruction", (pc - instr_pc).into());
        
        // Verify this is a LDI instruction (opcode = 0x0f)
        let ldi_opcode = table.add_constant("ldi_opcode", [B32::from(0x0f)]);
        table.assert_zero("is_ldi", (instr_opcode - ldi_opcode).into());
        
        // Verify dst matches instruction dst
        table.assert_zero("dst_matches_instruction", (dst - instr_dst).into());
        
        // Compute imm = imm_low + (imm_high << 16)
        let imm_high_shifted = table.add_computed("imm_high_shifted", instr_imm_high * B32::from(65536));
        let computed_imm = table.add_computed("computed_imm", instr_imm_low + imm_high_shifted);
        table.assert_zero("imm_computation_correct", (imm - computed_imm).into());
        
        // Push value to VROM (addr = fp + dst, value = imm)
        let addr = table.add_computed("addr", fp + dst);
        table.push(channels.vrom_channel, [addr, imm]);
        
        // Update state: PC = PC * G (moves to next instruction)
        let next_pc = table.add_computed("next_pc", pc * B32::from(BinaryField32b::MULTIPLICATIVE_GENERATOR));
        table.push(channels.state_channel, [next_pc, fp]);
        
        Self {
            id: table.id(),
            pc,
            fp,
            dst,
            imm,
        }
    }
}

impl<U> TableFiller<U> for LdiTable
where
    U: Pod + PackScalar<B32>,
{
    type Event = LdiEvent;
    
    fn id(&self) -> TableId {
        self.id
    }
    
    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event>,
        witness: &'a mut TableWitnessIndexSegment<U>,
    ) -> anyhow::Result<()> {
        let mut pc_col = witness.get_mut_as(self.pc)?;
        let mut fp_col = witness.get_mut_as(self.fp)?;
        let mut dst_col = witness.get_mut_as(self.dst)?;
        let mut imm_col = witness.get_mut_as(self.imm)?;
        
        for (i, event) in rows.enumerate() {
            pc_col[i] = event.pc;
            fp_col[i] = event.fp;
            dst_col[i] = event.dst as u32;
            imm_col[i] = event.imm;
        }
        
        Ok(())
    }
}

/// RET (Return) table.
pub struct RetTable {
    /// Table ID
    pub id: TableId,
    /// PC column
    pub pc: Col<B32, 1>,
    /// Frame pointer column
    pub fp: Col<B32, 1>,
    /// Return PC value from VROM[fp+0]
    pub fp_0_val: Col<B32, 1>,
    /// Return FP value from VROM[fp+1]
    pub fp_1_val: Col<B32, 1>,
}

impl RetTable {
    /// Create a new RET table.
    pub fn new(cs: &mut ConstraintSystem, channels: &ZkVMChannels) -> Self {
        let mut table = cs.add_table("ret_table");
        
        // Add columns for PC, FP, and return values
        let pc = table.add_committed("pc");
        let fp = table.add_committed("fp");
        let fp_0_val = table.add_committed("fp_0_val");
        let fp_1_val = table.add_committed("fp_1_val");
        
        // Pull from state channel
        table.pull(channels.state_channel, [pc, fp]);
        
        // Pull from PROM channel
        let instr_pc = table.add_committed::<B32, 1>("instr_pc");
        let instr_opcode = table.add_committed::<B32, 1>("instr_opcode");
        let zero_arg = table.add_constant("zero_arg", [B32::ZERO]);
        
        // Get instruction from PROM
        table.push(channels.prom_channel, [instr_pc, instr_opcode, zero_arg, zero_arg, zero_arg]);
        
        // Verify PC matches instruction PC
        table.assert_zero("pc_matches_instruction", (pc - instr_pc).into());
        
        // Verify this is a RET instruction (opcode = 0x0b)
        let ret_opcode = table.add_constant("ret_opcode", [B32::from(0x0b)]);
        table.assert_zero("is_ret", (instr_opcode - ret_opcode).into());
        
        // Compute addresses for return PC and FP
        let addr_0 = table.add_computed("addr_0", fp + B32::ZERO);
        let addr_1 = table.add_computed("addr_1", fp + B32::ONE);
        
        // Get return PC and FP from VROM
        table.push(channels.vrom_channel, [addr_0, fp_0_val]);
        table.push(channels.vrom_channel, [addr_1, fp_1_val]);
        
        // Push updated state (new PC and FP)
        table.push(channels.state_channel, [fp_0_val, fp_1_val]);
        
        Self {
            id: table.id(),
            pc,
            fp,
            fp_0_val,
            fp_1_val,
        }
    }
}

impl<U> TableFiller<U> for RetTable
where
    U: Pod + PackScalar<B32>,
{
    type Event = RetEvent;
    
    fn id(&self) -> TableId {
        self.id
    }
    
    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event>,
        witness: &'a mut TableWitnessIndexSegment<U>,
    ) -> anyhow::Result<()> {
        let mut pc_col = witness.get_mut_as(self.pc)?;
        let mut fp_col = witness.get_mut_as(self.fp)?;
        let mut fp_0_val_col = witness.get_mut_as(self.fp_0_val)?;
        let mut fp_1_val_col = witness.get_mut_as(self.fp_1_val)?;
        
        for (i, event) in rows.enumerate() {
            pc_col[i] = event.pc;
            fp_col[i] = event.fp;
            fp_0_val_col[i] = event.fp_0_val;
            fp_1_val_col[i] = event.fp_1_val;
        }
        
        Ok(())
    }
}