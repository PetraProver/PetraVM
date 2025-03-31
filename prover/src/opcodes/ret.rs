//! RET (Return) table implementation for the zCrayVM M3 circuit.
//!
//! This module contains the RET table which handles return operations
//! in the zCrayVM execution.

use binius_field::{as_packed_field::PackScalar, Field};
use binius_m3::builder::{
    upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1, B128, B32,
};
use bytemuck::Pod;
use zcrayvm_assembly::RetEvent;

use crate::{channels::ZkVMChannels, utils::pack_prom_entry_b128};

const RET_OPCODE: u32 = 0x0b;

/// RET (Return) table.
///
/// This table handles the Return instruction, which returns from a function
/// call by loading the return PC and FP from the current frame.
///
/// Logic:
/// 1. Load the current PC and FP from the state channel
/// 2. Get the instruction from PROM channel
/// 3. Verify this is a RET instruction
/// 4. Load the return PC and FP from VROM at addresses FP+0 and FP+1
/// 5. Update the state with the new PC and FP
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
    /// PROM channel pull value
    pub prom_pull: Col<B128, 1>,
    /// FP + 1 column
    pub fp_plus_one: Col<B32, 1>,
}

impl RetTable {
    /// Create a new RET table with the given constraint system and channels.
    ///
    /// # Arguments
    /// * `cs` - Constraint system to add the table to
    /// * `channels` - Channel IDs for communication with other tables
    pub fn new(cs: &mut ConstraintSystem, channels: &ZkVMChannels) -> Self {
        let mut table = cs.add_table("ret");

        // Add columns for PC, FP, and return values
        let pc = table.add_committed("pc");
        let fp = table.add_committed("fp");
        let fp_0_val = table.add_committed("fp_0_val");
        let fp_1_val = table.add_committed("fp_1_val");

        // Pull from state channel
        table.pull(channels.state_channel, [pc, fp]);

        // Pack instruction for PROM channel pull
        let prom_pull = table.add_computed(
            "prom_pull",
            upcast_expr(pc.into()) * B128::from(1u128 << 64) + B128::new(RET_OPCODE as u128),
        );

        // Pull instruction from PROM channel
        table.pull(channels.prom_channel, [prom_pull]);

        // Compute address for fp+1
        let fp_plus_one = table.add_computed("fp_plus_one", fp + B32::ONE);

        // Pull return PC and FP values from VROM channel
        table.pull(channels.vrom_channel, [fp, fp_0_val]);
        table.pull(channels.vrom_channel, [fp_plus_one, fp_1_val]);

        // Push updated state (new PC and FP)
        table.push(channels.state_channel, [fp_0_val, fp_1_val]);

        Self {
            id: table.id(),
            pc,
            fp,
            fp_0_val,
            fp_1_val,
            prom_pull,
            fp_plus_one,
        }
    }
}

impl<U> TableFiller<U> for RetTable
where
    U: Pod + PackScalar<B1>,
{
    type Event = RetEvent;

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
        let mut fp_0_val_col = witness.get_mut_as(self.fp_0_val)?;
        let mut fp_1_val_col = witness.get_mut_as(self.fp_1_val)?;
        let mut prom_pull_col = witness.get_mut_as(self.prom_pull)?;
        let mut fp_plus_one_col = witness.get_mut_as(self.fp_plus_one)?;

        for (i, event) in rows.enumerate() {
            pc_col[i] = event.pc;
            fp_col[i] = event.fp;
            fp_0_val_col[i] = event.fp_0_val;
            fp_1_val_col[i] = event.fp_1_val;
            prom_pull_col[i] = pack_prom_entry_b128(pc_col[i].val(), RET_OPCODE as u16, 0, 0, 0);
            fp_plus_one_col[i] = fp_col[i] + 1;
        }

        Ok(())
    }
}
