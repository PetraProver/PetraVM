use binius_field::Field;
use binius_m3::builder::{
    upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B32,
};
use petravm_asm::{BnzEvent, BzEvent, Opcode};

use crate::gadgets::state::{NextPc, StateColumns, StateColumnsOptions, StateGadget};
use crate::{channels::Channels, table::Table, types::ProverPackedField};

/// Table for BNZ in the non-zero case.
///
/// Asserts that the argument is not zero and that the program jumps to the
/// target address.
pub struct BnzTable {
    id: TableId,
    state_cols: StateColumns<{ Opcode::Bnz as u16 }>,
    cond_abs: Col<B32>, // Virtual
    cond_val: Col<B32>,
}

impl Table for BnzTable {
    type Event = BnzEvent;

    fn name(&self) -> &'static str {
        "BnzTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("bnz");
        let cond_val = table.add_committed("cond_val");
        table.assert_nonzero(cond_val);

        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Immediate,
                next_fp: None,
            },
        );

        let cond_abs = table.add_computed("cond_abs", state_cols.fp + upcast_col(state_cols.arg2));

        // Read cond_val
        table.pull(channels.vrom_channel, [upcast_col(cond_abs), cond_val]);

        Self {
            id: table.id(),
            state_cols,
            cond_abs,
            cond_val,
        }
    }
}

impl TableFiller<ProverPackedField> for BnzTable {
    type Event = BnzEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            let mut cond_abs = witness.get_scalars_mut(self.cond_abs)?;
            let mut cond_val = witness.get_scalars_mut(self.cond_val)?;
            for (i, event) in rows.clone().enumerate() {
                cond_abs[i] = B32::new(event.fp.addr(event.cond));
                cond_val[i] = B32::new(event.cond_val);
            }
        }
        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.val(),
            next_pc: Some(event.target.val()),
            fp: *event.fp,
            arg0: event.target.val() as u16,
            arg1: (event.target.val() >> 16) as u16,
            arg2: event.cond,
        });
        self.state_cols.populate(witness, state_rows)?;
        Ok(())
    }
}

pub struct BzTable {
    id: TableId,
    state_cols: StateColumns<{ Opcode::Bnz as u16 }>,
    cond_abs: Col<B32>, // Virtual
}

impl Table for BzTable {
    type Event = BzEvent;

    fn name(&self) -> &'static str {
        "BzTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("bz");

        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        let cond_abs = table.add_computed("cond_abs", state_cols.fp + upcast_col(state_cols.arg2));
        let zero = table.add_constant("zero", [B32::ZERO]);

        table.pull(channels.vrom_channel, [cond_abs, zero]);

        Self {
            id: table.id(),
            state_cols,
            cond_abs,
        }
    }
}

impl TableFiller<ProverPackedField> for BzTable {
    type Event = BzEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut cond_abs = witness.get_scalars_mut(self.cond_abs)?;
            for (i, event) in rows.clone().enumerate() {
                cond_abs[i] = B32::new(event.fp.addr(event.cond));
            }
        }
        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.val(),
            next_pc: None,
            fp: *event.fp,
            arg0: event.target.val() as u16,
            arg1: (event.target.val() >> 16) as u16,
            arg2: event.cond,
        });
        self.state_cols.populate(witness, state_rows)
    }
}
