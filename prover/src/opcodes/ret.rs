//! RET (Return) table implementation for the PetraVM M3 circuit.
//!
//! This module contains the RET table which handles return operations
//! in the PetraVM execution.

use binius_field::Field;
use binius_m3::builder::{Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B32};
use petravm_asm::{opcodes::Opcode, RetEvent};

use crate::gadgets::state::{NextPc, StateColumns, StateColumnsOptions};
use crate::{
    channels::Channels, gadgets::state::StateGadget, table::Table, types::ProverPackedField,
};
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
    id: TableId,
    /// State columns
    state_cols: StateColumns<{ Opcode::Ret as u16 }>,
    fp_xor_1: Col<B32>, // Virtual
    next_pc: Col<B32>,
    next_fp: Col<B32>,
}

impl Table for RetTable {
    type Event = RetEvent;

    fn name(&self) -> &'static str {
        "RetTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("ret");
        let next_pc = table.add_committed("next_pc");
        let next_fp = table.add_committed("next_fp");

        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Target(next_pc),
                next_fp: Some(next_fp),
            },
        );

        let fp0 = state_cols.fp;
        let fp_xor_1 = table.add_computed("fp_xor_1", fp0 + B32::ONE);

        // Read the next_pc
        table.pull(channels.vrom_channel, [fp0, next_pc]);

        // Read the next_fp
        table.pull(channels.vrom_channel, [fp_xor_1, next_fp]);

        Self {
            id: table.id(),
            state_cols,
            fp_xor_1,
            next_pc,
            next_fp,
        }
    }
}

impl TableFiller<ProverPackedField> for RetTable {
    type Event = RetEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut fp_xor_1 = witness.get_scalars_mut(self.fp_xor_1)?;
            let mut next_pc = witness.get_scalars_mut(self.next_pc)?;
            let mut next_fp = witness.get_scalars_mut(self.next_fp)?;
            for (i, event) in rows.clone().enumerate() {
                fp_xor_1[i] = B32::new(event.fp.addr(1u32));
                next_pc[i] = B32::new(event.pc_next);
                next_fp[i] = B32::new(event.fp_next);
            }
        }
        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.into(),
            next_pc: Some(event.pc_next),
            fp: *event.fp,
            ..Default::default()
        });
        self.state_cols.populate(witness, state_rows)
    }
}
