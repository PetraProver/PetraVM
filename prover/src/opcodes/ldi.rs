//! LDI (Load Immediate) table implementation for the zCrayVM M3 circuit.
//!
//! This module contains the LDI table which handles loading immediate values
//! into VROM locations in the zCrayVM execution.

use binius_field::{as_packed_field::PackScalar, BinaryField};
use binius_m3::builder::{
    upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1, B128, B16,
    B32,
};
use bytemuck::Pod;
use zcrayvm_assembly::{opcodes::Opcode, LDIEvent};

use crate::{
    channels::Channels,
    utils::{pack_instruction_with_32bits_imm, pack_instruction_with_32bits_imm_b128},
};

const LDI_OPCODE: u32 = Opcode::Ldi as u32;

/// LDI (Load Immediate) table.
///
/// This table handles the Load Immediate instruction, which loads a 32-bit
/// immediate value into a VROM location.
///
/// Logic:
/// 1. Load the current PC and FP from the state channel
/// 2. Get the instruction from PROM channel
/// 3. Verify this is an LDI instruction
/// 4. Store the 32-bit immediate value at FP + dst in VROM
/// 5. Update PC to move to the next instruction
pub struct LdiTable {
    /// Table ID
    pub id: TableId,
    /// PC column
    pub pc: Col<B32, 1>,
    /// Frame pointer column
    pub fp: Col<B32, 1>,
    /// Destination VROM offset column
    pub dst: Col<B16, 1>,
    /// Immediate value
    pub imm: Col<B32, 1>,
    /// PROM channel pull value
    pub prom_pull: Col<B128, 1>,
    /// Next PC column
    pub next_pc: Col<B32, 1>,
    /// VROM absolute address column
    pub vrom_abs_addr: Col<B32, 1>,
}

impl LdiTable {
    /// Create a new LDI table with the given constraint system and channels.
    ///
    /// # Arguments
    /// * `cs` - Constraint system to add the table to
    /// * `channels` - Channel IDs for communication with other tables
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("ldi");

        // Add columns for PC, FP, and other instruction components
        let pc = table.add_committed("pc");
        let fp = table.add_committed("cur_fp");
        let dst = table.add_committed("dst");
        let imm = table.add_committed("imm");

        // Pull from state channel (get current state)
        table.pull(channels.state_channel, [pc, fp]);

        // Pack instruction for PROM channel pull
        let prom_pull =
            pack_instruction_with_32bits_imm(&mut table, "prom_pull", pc, LDI_OPCODE, dst, imm);

        // Pull from PROM channel
        table.pull(channels.prom_channel, [prom_pull]);

        // Compute absolute address for VROM
        let vrom_abs_addr = table.add_computed::<B32, 1>("abs_addr", fp + upcast_expr(dst.into()));

        // Pull from VROM channel
        table.pull(channels.vrom_channel, [vrom_abs_addr, imm]);

        // Compute next PC
        let next_pc = table.add_computed::<B32, 1>("next_pc", pc * B32::MULTIPLICATIVE_GENERATOR);

        // Push to state channel
        table.push(channels.state_channel, [next_pc, fp]);

        Self {
            id: table.id(),
            pc,
            fp,
            dst,
            imm,
            prom_pull,
            next_pc,
            vrom_abs_addr,
        }
    }
}

impl<U> TableFiller<U> for LdiTable
where
    U: Pod + PackScalar<B1>,
{
    type Event = LDIEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event>,
        witness: &'a mut TableWitnessSegment<U>,
    ) -> anyhow::Result<()> {
        let mut pc_col = witness.get_mut_as(self.pc)?;
        let mut fp_col = witness.get_mut_as(self.fp)?;
        let mut dst_col = witness.get_mut_as(self.dst)?;
        let mut imm_col = witness.get_mut_as(self.imm)?;
        let mut next_pc_col = witness.get_mut_as(self.next_pc)?;
        let mut prom_pull_col = witness.get_mut_as(self.prom_pull)?;
        let mut vrom_abs_addr_col = witness.get_mut_as(self.vrom_abs_addr)?;

        for (i, event) in rows.enumerate() {
            pc_col[i] = event.pc;
            fp_col[i] = B32::new(*event.fp); // Convert to B32 using deref
            dst_col[i] = event.dst;
            imm_col[i] = event.imm;

            next_pc_col[i] = pc_col[i] * B32::MULTIPLICATIVE_GENERATOR;
            prom_pull_col[i] = pack_instruction_with_32bits_imm_b128(
                pc_col[i].val(),
                LDI_OPCODE as u16,
                dst_col[i],
                imm_col[i],
            );
            vrom_abs_addr_col[i] = B32::new(event.fp.addr(dst_col[i] as u32));
        }

        Ok(())
    }
}
