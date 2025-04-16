use std::any::Any;

use binius_field::Field;
use binius_m3::builder::{
    upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B32,
};
use zcrayvm_assembly::{BnzEvent, BzEvent, Opcode};

use crate::gadgets::cpu::{CpuColumns, CpuColumnsOptions, CpuGadget, NextPc};
use crate::{channels::Channels, table::Table, types::ProverPackedField};

/// Table for BNZ in the non-zero case.
///
/// Asserts that the argument is not zero and that the program jumps to the
/// target address.
pub struct BnzTable {
    id: TableId,
    cpu_cols: CpuColumns<{ Opcode::Bnz as u16 }>,
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

        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Immediate,
                next_fp: None,
            },
        );

        let cond_abs = table.add_computed("cond_abs", cpu_cols.fp + upcast_col(cpu_cols.arg0));

        // Read cond_val
        table.pull(channels.vrom_channel, [upcast_col(cond_abs), cond_val]);

        Self {
            id: table.id(),
            cpu_cols,
            cond_abs,
            cond_val,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
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
            let mut cond_abs = witness.get_mut_as(self.cond_abs)?;
            let mut cond_val = witness.get_mut_as(self.cond_val)?;
            for (i, event) in rows.clone().enumerate() {
                cond_abs[i] = event.fp.addr(event.cond);
                cond_val[i] = event.cond_val;
            }
        }
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.val(),
            next_pc: Some(event.target.val()),
            fp: *event.fp,
            arg0: event.cond,
            arg1: event.target.val() as u16,
            arg2: (event.target.val() >> 16) as u16,
        });
        self.cpu_cols.populate(witness, cpu_rows)?;
        Ok(())
    }
}

pub struct BzTable {
    id: TableId,
    cpu_cols: CpuColumns<{ Opcode::Bnz as u16 }>,
    cond_abs: Col<B32>, // Virtual
}

impl Table for BzTable {
    type Event = BzEvent;

    fn name(&self) -> &'static str {
        "BzTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("bz");

        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        let cond_abs = table.add_computed("cond_abs", cpu_cols.fp + upcast_col(cpu_cols.arg0));
        let zero = table.add_constant("zero", [B32::ZERO]);

        table.pull(channels.vrom_channel, [cond_abs, zero]);

        Self {
            id: table.id(),
            cpu_cols,
            cond_abs,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
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
            let mut cond_abs = witness.get_mut_as(self.cond_abs)?;
            for (i, event) in rows.clone().enumerate() {
                cond_abs[i] = event.fp.addr(event.cond);
            }
        }
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.val(),
            next_pc: None,
            fp: *event.fp,
            arg0: event.cond,
            arg1: event.target.val() as u16,
            arg2: (event.target.val() >> 16) as u16,
        });
        self.cpu_cols.populate(witness, cpu_rows)
    }
}
