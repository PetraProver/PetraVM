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

const LDI_OPCODE: u32 = 0x0f;
const G: BinaryField32b = BinaryField32b::MULTIPLICATIVE_GENERATOR;

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
    /// Immediate value low bits column
    pub imm_low: Col<B32, 1>,
    /// Immediate value high bits column
    pub imm_high: Col<B32, 1>,
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
        let pc = table.add_committed::<B32, 1>("pc");
        let fp = table.add_committed::<B32, 1>("cur_fp");
        let dst = table.add_committed::<B32, 1>("dst");
        let imm_low = table.add_committed::<B32, 1>("imm_low");
        let imm_high = table.add_committed::<B32, 1>("imm_high");

        // Pull from state channel (get current state)
        table.pull(channels.state_channel, [pc, fp]);

        let ldi_opcode_const = table.add_constant("ldi_opcode", [B32::from(LDI_OPCODE)]);

        // Pull from PROM channel in the same order as PromTable::fill
        table.pull(
            channels.prom_channel,
            [
                pc,
                ldi_opcode_const,
                dst,      // Arg1 = dst
                imm_low,  // Arg2 = imm_low
                imm_high, // Arg3 = imm_high
            ],
        );

        let abs_addr = table.add_computed::<B32, 1>("abs_addr", fp + dst);
        let computed_imm = table.add_computed::<B32, 1>("computed_imm", imm_low + imm_high); // TODO: FIX IT

        // Push value to VROM write table using absolute address
        table.pull(channels.vrom_channel, [abs_addr, computed_imm]);

        // Update state: PC = PC * G (moves to next instruction multiplicatively)
        let g_const = table.add_constant("generator", [G]);
        let next_pc = table.add_computed::<B32, 1>("next_pc", pc * g_const);
        table.push(channels.state_channel, [next_pc, fp]); // FP remains the same

        Self {
            id: table.id(),
            pc,
            fp,
            dst,
            imm_low,
            imm_high,
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
        let mut imm_low_col = witness.get_mut_as(self.imm_low)?;
        let mut imm_high_col = witness.get_mut_as(self.imm_high)?;

        for (i, event) in rows.enumerate() {
            pc_col[i] = event.pc;
            fp_col[i] = event.fp;
            dst_col[i] = event.dst as u32;

            // Split the immediate value into low and high parts
            imm_low_col[i] = event.imm & 0xFFFF;
            imm_high_col[i] = (event.imm >> 16) & 0xFFFF;

            dbg!(
                "Ldi fill",
                &pc_col[i],
                &fp_col[i],
                &dst_col[i],
                &imm_low_col[i],
                &imm_high_col[i]
            );
        }

        Ok(())
    }
}
