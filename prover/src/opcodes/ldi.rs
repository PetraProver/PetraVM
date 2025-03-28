//! LDI (Load Immediate) table implementation for the zCrayVM M3 circuit.
//!
//! This module contains the LDI table which handles loading immediate values
//! into VROM locations in the zCrayVM execution.

use binius_field::{as_packed_field::PackScalar, BinaryField};
use binius_m3::builder::{
    upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessIndexSegment, B1, B16,
    B32,
};
use bytemuck::Pod;
use zcrayvm_assembly::LDIEvent;

use crate::{
    channel_utils::{pack_b32_into_b64, pack_prom_entry, pack_state_b32_into_b128},
    channels::ZkVMChannels,
};

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
    pub dst: Col<B16, 1>,
    /// Immediate value low bits column
    pub imm_low: Col<B16, 1>,
    /// Immediate value high bits column
    pub imm_high: Col<B16, 1>,
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
        let dst = table.add_committed::<B16, 1>("dst");
        let imm_low = table.add_committed::<B16, 1>("imm_low");
        let imm_high = table.add_committed::<B16, 1>("imm_high");

        // Pack FP and PC for state channel pull
                let state_pull = pack_state_b32_into_b128(&mut table, "state_pull", pc, fp);

        // Pull from state channel (get current state)
        table.pull(channels.state_channel, [state_pull]);

        let ldi_opcode_const = table.add_constant("ldi_opcode", [B16::from(0x0f)]);

        // Pack instruction for PROM channel pull
        let prom_pull = pack_prom_entry(
            &mut table,
            "prom_pull",
            pc,
            ldi_opcode_const,
            [dst, imm_low, imm_high],
        );

        // Pull from PROM channel
        table.pull(channels.prom_channel, [prom_pull]);

        // Compute the immediate value (combine low and high parts)
        let computed_imm = table.add_computed::<B32, 1>(
            "computed_imm",
            upcast_expr(imm_low.into()) + upcast_expr(imm_high.into()) * B32::from(65536), // 2^16
        );

        // Compute absolute address for VROM
        let abs_addr = table.add_computed::<B32, 1>("abs_addr", fp + upcast_expr(dst.into()));

        // Pack address and value for VROM channel push
        // Format: [addr (lower 32 bits), value (upper 32 bits)]
        let vrom_push = pack_b32_into_b64(&mut table, "vrom_push", abs_addr, computed_imm);

        // Pull from VROM channel
        table.pull(channels.vrom_channel, [vrom_push]);

        // Compute next PC
        let G =  B32::MULTIPLICATIVE_GENERATOR;
        let next_pc = table.add_computed::<B32, 1>("next_pc", pc * G);

        // Pack FP and next PC for state channel push
        let state_push = pack_state_b32_into_b128(&mut table, "state_push", next_pc, fp);

        // Push to state channel
        table.push(channels.state_channel, [state_push]);

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
    U: Pod + PackScalar<B1>,
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
            dst_col[i] = event.dst;

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
