//! LDI (Load Immediate) table implementation for the zCrayVM M3 circuit.
//!
//! This module contains the LDI table which handles loading immediate values
//! into VROM locations in the zCrayVM execution.

use binius_field::{as_packed_field::PackScalar, BinaryField, BinaryField32b};
use binius_m3::builder::{
    Col, ConstraintSystem, TableFiller, TableId, TableWitnessIndexSegment, B32,
};
use bytemuck::Pod;
use zcrayvm_assembly::LDIEvent;

use crate::channels::ZkVMChannels;

/// LDI (Load Immediate) table.
///
/// This table handles the Load Immediate instruction, which loads a 32-bit
/// immediate value into a VROM location.
///
/// Logic:
/// 1. Load the current PC and FP from the state channel
/// 2. Get the instruction from PROM channel
/// 3. Verify this is an LDI instruction
/// 4. Compute the immediate value from the low and high parts
/// 5. Store the immediate value at FP + dst in VROM
/// 6. Update PC to move to the next instruction
pub struct LdiTable {
    /// Table ID
    pub id: TableId,
    /// PC column
    pub pc: Col<B32, 1>,
    /// Frame pointer column
    pub fp: Col<B32, 1>,
    /// Destination VROM offset column
    pub dst: Col<B32, 1>,
    /// Immediate value column
    pub imm: Col<B32, 1>,
}

impl LdiTable {
    /// Create a new LDI table with the given constraint system and channels.
    ///
    /// # Arguments
    /// * `cs` - Constraint system to add the table to
    /// * `channels` - Channel IDs for communication with other tables
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
        table.push(
            channels.prom_channel,
            [
                instr_pc,
                instr_opcode,
                instr_dst,
                instr_imm_low,
                instr_imm_high,
            ],
        );

        // Verify PC matches instruction PC
        table.assert_zero("pc_matches_instruction", (pc - instr_pc).into());

        // Verify this is a LDI instruction (opcode = 0x0f)
        let ldi_opcode = table.add_constant("ldi_opcode", [B32::from(0x0f)]);
        table.assert_zero("is_ldi", (instr_opcode - ldi_opcode).into());

        // Verify dst matches instruction dst
        table.assert_zero("dst_matches_instruction", (dst - instr_dst).into());

        // Compute imm = imm_low + (imm_high << 16)
        let shift_amount = table.add_constant("shift_16", [B32::from(65536)]); // 2^16
        let imm_high_shifted =
            table.add_computed("imm_high_shifted", instr_imm_high * shift_amount);
        let computed_imm = table.add_computed("computed_imm", instr_imm_low + imm_high_shifted);
        table.assert_zero("imm_computation_correct", (imm - computed_imm).into());

        // Push value to VROM (addr = fp + dst, value = imm)
        let addr = table.add_computed("addr", fp + dst);
        table.push(channels.vrom_channel, [addr, imm]);

        // Update state: PC = PC * G (moves to next instruction)
        let g = table.add_constant(
            "generator",
            [B32::from(BinaryField32b::MULTIPLICATIVE_GENERATOR)],
        );
        let next_pc = table.add_computed("next_pc", pc * g);
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
    type Event = LDIEvent;

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
