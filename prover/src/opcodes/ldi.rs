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
        let pc = table.add_committed::<B32, 1>("pc");
        let cur_fp = table.add_committed::<B32, 1>("cur_fp"); // Current frame pointer
        let dst = table.add_committed::<B32, 1>("dst"); // Destination VROM offset
        let imm = table.add_committed::<B32, 1>("imm");

        // Pull from state channel (get current state)
        table.pull(channels.state_channel, [pc, cur_fp]);

        // Pull from PROM channel (get opcode and arguments)
        let instr_pc = table.add_committed::<B32, 1>("instr_pc");
        let instr_opcode = table.add_committed::<B32, 1>("instr_opcode");
        let instr_dst = table.add_committed::<B32, 1>("instr_dst"); // Represents dst offset
        let instr_imm_low = table.add_committed::<B32, 1>("instr_imm_low"); // Represents lower 16 bits of imm
        let instr_imm_high = table.add_committed::<B32, 1>("instr_imm_high"); // Represents upper 16 bits of imm

        // Pull from PROM channel in the same order as PromTable::fill
        table.pull(
            channels.prom_channel,
            [
                instr_pc,
                instr_opcode,
                instr_dst,      // Arg1 = dst
                instr_imm_low,  // Arg2 = imm_low
                instr_imm_high, // Arg3 = imm_high
            ],
        );

        // Compute absolute address: cur_fp + instr_dst
        // Since arithmetic is XOR in binary fields, this is cur_fp ^ instr_dst
        let abs_addr = table.add_computed::<B32, 1>("abs_addr", cur_fp + instr_dst);

        // Verify all constraints in one go
        let ldi_opcode_const = table.add_constant("ldi_opcode", [B32::from(LDI_OPCODE)]);

        // Verify PC matches instruction PC
        // Note: PC advances multiplicatively (pc = G^(int_pc - 1))
        // We expect instr_pc to match the current pc from the witness
        // table.assert_zero("pc_matches_instruction", (pc - instr_pc).into());

        // Verify this is a LDI instruction
        table.assert_zero("is_ldi", instr_opcode - ldi_opcode_const);

        // Verify witness destination offset matches instruction destination offset
        // table.assert_zero("dst_matches", (dst - instr_dst).into());

        // TODO: Compute imm = imm_low + (imm_high << 16)
        // table.assert_zero("imm_computation_correct", (imm - computed_imm).into());

        // Push value to VROM write table using absolute address
        table.pull(channels.vrom_channel, [abs_addr, imm]);

        // Update state: PC = PC * G (moves to next instruction multiplicatively)
        let g_const = table.add_constant("generator", [G]);
        let next_pc = table.add_computed::<B32, 1>("next_pc", pc * g_const);
        table.push(channels.state_channel, [next_pc, cur_fp]); // FP remains the same

        Self {
            id: table.id(),
            pc,
            fp: cur_fp, // Store current frame pointer
            dst,        // Store destination offset
            imm,        // Store final immediate value
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
