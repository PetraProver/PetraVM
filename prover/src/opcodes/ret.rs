//! RET (Return) table implementation for the zCrayVM M3 circuit.
//!
//! This module contains the RET table which handles return operations
//! in the zCrayVM execution.

use binius_field::Field;
use binius_m3::builder::{
    Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B128, B16, B32,
};
use zcrayvm_assembly::{opcodes::Opcode, RetEvent};

use crate::{
    channels::Channels,
    types::CommonTableBounds,
    utils::{pack_instruction_b128, pack_instruction_no_args},
};

const RET_OPCODE: u16 = Opcode::Ret as u16;

/// RET (Return) table.
///
/// This table handles the Return instruction, which returns from a function
/// call by loading the return PC and FP from the current frame.
///
/// Logic:
/// 1. Load the current PC and FP from the state channel
/// 2. Get the instruction from PROM channel
/// 3. Verify this is a RET instruction
/// 4. Load the return PC from VROM[fp+0] and return FP from VROM[fp+1]
/// 5. Update the state with the new PC and FP values
pub struct RetTable {
    /// Table ID
    pub id: TableId,
    /// PC column
    pub pc: Col<B32>,
    /// Frame pointer column
    pub fp: Col<B32>,
    /// Return PC value from VROM[fp+0]
    pub next_pc: Col<B32>,
    /// Return FP value from VROM[fp+1]
    pub next_fp: Col<B32>,
    /// PROM channel pull value
    pub prom_pull: Col<B128>,
    /// FP + 1 column
    pub fp_plus_one: Col<B32>,
}

impl RetTable {
    /// Create a new RET table with the given constraint system and channels.
    ///
    /// # Arguments
    /// * `cs` - Constraint system to add the table to
    /// * `channels` - Channel IDs for communication with other tables
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("ret");

        // Add columns for PC, FP, and return values
        let pc = table.add_committed("pc");
        let fp = table.add_committed("fp");
        let next_pc = table.add_committed("next_pc");
        let next_fp = table.add_committed("next_fp");

        // Pull from state channel
        table.pull(channels.state_channel, [pc, fp]);

        // Pack instruction for PROM channel pull
        let prom_pull = pack_instruction_no_args(&mut table, "prom_pull", pc, RET_OPCODE);

        // Pull instruction from PROM channel
        table.pull(channels.prom_channel, [prom_pull]);

        // Compute address for fp+1
        let fp_plus_one = table.add_computed("fp_plus_one", fp + B32::ONE);

        // Pull return PC and FP values from VROM channel
        table.pull(channels.vrom_channel, [fp, next_pc]);
        table.pull(channels.vrom_channel, [fp_plus_one, next_fp]);

        // Push updated state (new PC and FP)
        table.push(channels.state_channel, [next_pc, next_fp]);

        Self {
            id: table.id(),
            pc,
            fp,
            next_pc,
            next_fp,
            prom_pull,
            fp_plus_one,
        }
    }
}

impl<U> TableFiller<U> for RetTable
where
    U: CommonTableBounds,
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
        let mut pc_col = witness.get_scalars_mut(self.pc)?;
        let mut fp_col = witness.get_scalars_mut(self.fp)?;
        let mut next_pc_col = witness.get_scalars_mut(self.next_pc)?;
        let mut next_fp_col = witness.get_scalars_mut(self.next_fp)?;
        let mut prom_pull_col = witness.get_scalars_mut(self.prom_pull)?;
        let mut fp_plus_one_col = witness.get_scalars_mut(self.fp_plus_one)?;

        for (i, event) in rows.enumerate() {
            pc_col[i] = event.pc;
            fp_col[i] = B32::new(*event.fp);
            next_pc_col[i] = B32::new(event.pc_next);
            next_fp_col[i] = B32::new(event.fp_next);
            prom_pull_col[i] = pack_instruction_b128(
                pc_col[i],
                B16::new(RET_OPCODE),
                B16::ZERO,
                B16::ZERO,
                B16::ZERO,
            );
            fp_plus_one_col[i] = B32::new(event.fp.addr(1u32));
        }

        Ok(())
    }
}
