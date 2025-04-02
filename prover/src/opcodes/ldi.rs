//! LDI (Load Immediate) table implementation for the zCrayVM M3 circuit.
//!
//! This module contains the LDI table which handles loading immediate values
//! into VROM locations in the zCrayVM execution.

use binius_field::{
    as_packed_field::PackScalar, underlier::UnderlierType, BinaryField, BinaryField32b,
};
use binius_m3::builder::{
    upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1, B32, B64,
};
use bytemuck::Pod;
use zcrayvm_assembly::{LDIEvent, Opcode};

use super::{
    cpu::{CpuColumns, CpuColumnsOptions, CpuEvent, NextPc},
    util::{pack_b16_into_b32, pack_b32_into_b64},
};
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
    /// CPU columns
    cpu_cols: CpuColumns<{ Opcode::Ldi as u16 }>,
    vrom_push: Col<B64>, // Virtual
    abs_addr: Col<B32>,  // Virtual
}

impl LdiTable {
    /// Create a new LDI table with the given constraint system and channels.
    ///
    /// # Arguments
    /// * `cs` - Constraint system to add the table to
    /// * `channels` - Channel IDs for communication with other tables
    pub fn new(cs: &mut ConstraintSystem, channels: &ZkVMChannels) -> Self {
        let mut table = cs.add_table("ldi");

        let ZkVMChannels {
            state_channel,
            prom_channel,
            ..
        } = *channels;

        let cpu_cols = CpuColumns::new(
            &mut table,
            state_channel,
            prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        let CpuColumns {
            fp,
            arg0: dst,
            arg1: imm_low,
            arg2: imm_high,
            ..
        } = cpu_cols;

        let abs_addr = table.add_computed("abs_addr", fp + upcast_col(dst));

        // Push value to VROM write table using absolute address

        // let computed_imm = table.add_computed::<B32, 1>("computed_imm", imm_low.int()
        // + imm_high); // TODO: FIX IT
        let vrom_push = table.add_computed(
            "vrom_push",
            pack_b32_into_b64([
                abs_addr.into(),
                pack_b16_into_b32([imm_low.into(), imm_high.into()]),
            ]),
        );
        table.pull(channels.vrom_channel, [vrom_push]);

        Self {
            id: table.id(),
            cpu_cols,
            vrom_push,
            abs_addr,
        }
    }
}

impl<U: UnderlierType> TableFiller<U> for LdiTable
where
    U: Pod + PackScalar<B1>,
{
    type Event = LDIEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<U>,
    ) -> anyhow::Result<()> {
        {
            let mut vrom_push = witness.get_mut_as(self.vrom_push)?;
            let mut abs_addr = witness.get_mut_as(self.abs_addr)?;
            for (i, event) in rows.clone().enumerate() {
                vrom_push[i] = (event.imm as u64) << 32 | event.fp as u64 + event.dst as u64;
                abs_addr[i] = event.fp ^ (event.dst as u32);
                dbg!("Ldi fill", &vrom_push[i]);
            }
        }
        let cpu_rows = rows.map(|event| CpuEvent {
            pc: event.pc.val(),
            next_pc: None,
            fp: event.fp,
            next_fp: None,
            arg0: event.dst,
            arg1: event.imm as u16 & 0xFFFF,
            arg2: (event.imm >> 16) as u16,
        });
        self.cpu_cols.populate(witness, cpu_rows)
    }
}
