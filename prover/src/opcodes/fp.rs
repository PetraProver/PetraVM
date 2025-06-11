use binius_m3::builder::{
    upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B32,
};
use petravm_asm::{FpEvent, Opcode};

use crate::gadgets::state::{NextPc, StateColumns, StateColumnsOptions, StateGadget};
use crate::utils::pull_vrom_channel;
use crate::{channels::Channels, table::Table, types::ProverPackedField};

/// Table for FP instruction.
///
/// Implements the dumping of the current FP (plus an offset) to the
/// destination.
/// Logic: FP[dst] = FP + imm
pub struct FpTable {
    id: TableId,
    state_cols: StateColumns<{ Opcode::Fp as u16 }>,
    dst_abs: Col<B32>, // Virtual
    dst_val: Col<B32>, // Virtual
}

impl Table for FpTable {
    type Event = FpEvent;

    fn name(&self) -> &'static str {
        "FpTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("fp");

        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        let dst_abs = table.add_computed("dst_abs", state_cols.fp + upcast_col(state_cols.arg0));
        let dst_val = table.add_computed("dst_val", state_cols.fp + upcast_col(state_cols.arg1));

        // Read dst_val
        pull_vrom_channel(&mut table, channels.vrom_channel, [dst_abs, dst_val]);

        Self {
            id: table.id(),
            state_cols,
            dst_abs,
            dst_val,
        }
    }
}

impl TableFiller<ProverPackedField> for FpTable {
    type Event = FpEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            // Get mutable references to witness columns
            let mut dst_abs_addr = witness.get_scalars_mut(self.dst_abs)?;
            let mut dst_val = witness.get_scalars_mut(self.dst_val)?;

            // Fill the witness columns with values from each event
            for (i, event) in rows.clone().enumerate() {
                dst_abs_addr[i] = B32::new(event.fp.addr(event.dst));
                dst_val[i] = B32::new(event.fp.addr(event.imm));
            }
        }

        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.val(),
            next_pc: None,
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.imm,
            ..Default::default()
        });
        self.state_cols.populate(witness, state_rows)
    }
}
