//! RET (Return) table implementation for the zCrayVM M3 circuit.
//!
//! This module contains the RET table which handles return operations
//! in the zCrayVM execution.

use binius_field::{as_packed_field::PackScalar, Field};
use binius_m3::builder::{
    Col, ConstraintSystem, TableFiller, TableId, TableWitnessIndexSegment, B32,
};
use bytemuck::Pod;
use zcrayvm_assembly::RetEvent;

use crate::channels::ZkVMChannels;

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
}

impl RetTable {
    /// Create a new RET table with the given constraint system and channels.
    ///
    /// # Arguments
    /// * `cs` - Constraint system to add the table to
    /// * `channels` - Channel IDs for communication with other tables
    pub fn new(cs: &mut ConstraintSystem, channels: &ZkVMChannels) -> Self {
        let mut table = cs.add_table("ret_table");

        // Add columns for PC, FP, and return values
        let pc = table.add_committed("pc");
        let fp = table.add_committed("fp");
        let fp_0_val = table.add_committed("fp_0_val");
        let fp_1_val = table.add_committed("fp_1_val");

        // Pull from state channel
        table.pull(channels.state_channel, [pc, fp]);

        let ret_opcode = table.add_constant("ret_opcode", [B32::from(0x0b)]);
        let zero_arg = table.add_constant("zero_arg", [B32::ZERO]);

        // Pull instruction from PROM channel
        table.pull(
            channels.prom_channel,
            [pc, ret_opcode, zero_arg, zero_arg, zero_arg],
        );

        // Compute addresses for return PC and FP
        let one = table.add_constant("one", [B32::ONE]);
        let addr_1 = table.add_computed("addr_1", fp + one);

        // Pull return PC and FP values from VROM channel (instead of pushing)
        table.pull(channels.vrom_channel, [fp, fp_0_val]);
        table.pull(channels.vrom_channel, [addr_1, fp_1_val]);

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
            dbg!(
                "Ret fill",
                &pc_col[i],
                &fp_col[i],
                &fp_0_val_col[i],
                &fp_1_val_col[i]
            );
        }

        Ok(())
    }
}
