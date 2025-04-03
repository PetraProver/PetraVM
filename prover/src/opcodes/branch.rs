use binius_field::{as_packed_field::PackScalar, underlier::UnderlierType, ExtensionField, Field};
use binius_m3::builder::{
    upcast_col, upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1,
    B32, B64,
};
use bytemuck::Pod;
use zcrayvm_assembly::{BnzEvent, BzEvent, Opcode};

use super::cpu::{CpuColumns, CpuColumnsOptions, CpuEvent, NextPc};
use crate::channels::Channels;

/// Table for BNZ.
///
/// Performs a branching to the target address if the argument is not zero.
///
/// Logic:
///   1. if FP[cond] <> 0, then PC = target
///   2. if FP[cond] == 0, then increment PC
pub struct BnzTable {
    id: TableId,
    cpu_cols: CpuColumns<{ Opcode::Bnz as u16 }>,
    cond_abs: Col<B32>, // Virtual
    cond_val: Col<B32>,
}

impl BnzTable {
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("bnz");
        let cond_val = table.add_committed("cond_val");

        let Channels {
            state_channel,
            prom_channel,
            vrom_channel,
            ..
        } = *channels;

        // TODO: Assert cond_val is != 0

        let cpu_cols = CpuColumns::new(
            &mut table,
            state_channel,
            prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Immediate,
                next_fp: None,
            },
        );

        let cond_abs = table.add_computed("cond_abs", cpu_cols.fp + upcast_col(cpu_cols.arg0));

        // Read cond_val
        table.pull(vrom_channel, [upcast_col(cond_abs).into(), cond_val.into()]);

        Self {
            id: table.id(),
            cpu_cols,
            cond_abs,
            cond_val,
        }
    }
}

impl<U: UnderlierType> TableFiller<U> for BnzTable
where
    U: Pod + PackScalar<B1>,
{
    type Event = BnzEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<U>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut cond_abs = witness.get_mut_as(self.cond_abs)?;
            let mut cond_val = witness.get_mut_as(self.cond_val)?;
            for (i, event) in rows.clone().enumerate() {
                cond_abs[i] = *event.fp ^ (event.cond as u32);
                cond_val[i] = event.cond_val;
                dbg!("Bnz fill", cond_val[i]);
            }
        }
        let cpu_rows = rows.map(|event| CpuEvent {
            pc: event.pc.val(),
            next_pc: Some((event.target_high.val() as u32) << 16 | event.target_low.val() as u32),
            fp: *event.fp,
            next_fp: None,
            arg0: event.cond,
            arg1: event.target_low.val(),
            arg2: event.target_high.val(),
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

impl BzTable {
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("bz");

        let Channels {
            state_channel,
            prom_channel,
            vrom_channel,
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

        let cond = cpu_cols.arg0;
        let cond_abs = table.add_computed("cond_abs", cpu_cols.fp + upcast_col(cpu_cols.arg0));
        let zero = table.add_constant("zero", [B32::ZERO]);

        table.pull(vrom_channel, [cond_abs.into(), zero]);

        Self {
            id: table.id(),
            cpu_cols,
            cond_abs,
        }
    }
}

impl<U: UnderlierType> TableFiller<U> for BzTable
where
    U: Pod + PackScalar<B1>,
{
    type Event = BzEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<U>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut cond_abs = witness.get_mut_as(self.cond_abs)?;
            for (i, event) in rows.clone().enumerate() {
                cond_abs[i] = *event.fp ^ (event.cond as u32);
            }
        }
        let cpu_rows = rows.map(|event| CpuEvent {
            pc: event.pc.val(),
            next_pc: None,
            fp: *event.fp,
            next_fp: None,
            arg0: event.cond,
            arg1: event.target_low.val(),
            arg2: event.target_high.val(),
        });
        self.cpu_cols.populate(witness, cpu_rows)
    }
}
