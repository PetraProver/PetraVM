use binius_field::{
    as_packed_field::PackScalar, underlier::UnderlierType,
    ExtensionField,
};
use binius_m3::builder::{
    upcast_col, upcast_expr, Col, ConstraintSystem, Expr, TableFiller, TableId,
    TableWitnessIndexSegment, B1, B32, B64,
};
use bytemuck::Pod;
use zcrayvm_assembly::{BnzEvent, BzEvent, Opcode};

use super::{cpu::{CpuColumns, CpuColumnsOptions, CpuEvent, NextPc}, util::pack_b32_into_b64};
use crate::channels::ZkVMChannels;

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
    cond_val: Col<B32>,
    vrom_push: Col<B64>, // Virtual;
}

impl BnzTable {
    pub fn new(cs: &mut ConstraintSystem, channels: &ZkVMChannels) -> Self {
        let ZkVMChannels {
            state_channel,
            prom_channel,
            vrom_channel,
            vrom_addr_space_channel: _,
        } = *channels;
        let mut table = cs.add_table("bnz");
        let cond_val = table.add_committed("cond_val");

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

        let cond = cpu_cols.arg0;
        
        let vrom_push = table.add_computed(
            "vrom_push",
            pack_b32_into_b64([upcast_col(cond).into(), cond_val.into()]),
        );
        // Read cond_val
        table.push(vrom_channel, [vrom_push]);

        Self {
            id: table.id(),
            cpu_cols,
            cond_val,
            vrom_push,
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
        witness: &'a mut TableWitnessIndexSegment<U>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut cond_val = witness.get_mut_as(self.cond_val)?;
            let mut vrom_push = witness.get_mut_as(self.vrom_push)?;
            for (i, event) in rows.clone().enumerate() {
                cond_val[i] = event.cond_val;
                vrom_push[i] = (event.cond_val as u64) << 32 | event.cond as u64;
            }
        }
        let cpu_rows = rows.map(|event| CpuEvent {
            pc: event.pc.val(),
            next_pc: Some((event.target_high.val() as u32) << 16 | event.target_low.val() as u32),
            fp: event.fp,
            next_fp: None,
            arg0: event.cond,
            arg1: event.target_low.val(),
            arg2: event.target_high.val(),
        });
        self.cpu_cols.populate(witness, cpu_rows)?;
        Ok(())
    }
}

pub(crate) struct BzTable {
    id: TableId,
    cpu_cols: CpuColumns<{ Opcode::Bnz as u16 }>,
    vrom_push: Col<B64>, // Virtual
}

impl BzTable {
    pub fn new(cs: &mut ConstraintSystem, channels: &ZkVMChannels) -> Self {
        let ZkVMChannels {
            state_channel,
            prom_channel,
            vrom_channel,
            vrom_addr_space_channel: _,
        } = *channels;
        let mut table = cs.add_table("bz");

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

        // TODO: Load this from some utility module
        let b64_basis: [_; 2] = std::array::from_fn(|i| {
            <B64 as ExtensionField<B32>>::basis(i).expect("i in range 0..2; extension degree is 2")
        });

        let vrom_push = table.add_computed(
            "vrom_push",
            // cond_val is zero in this case
            upcast_expr(cond.into()) * b64_basis[0],
        );
        // Read cond_val
        table.push(vrom_channel, [vrom_push]);

        Self {
            id: table.id(),
            cpu_cols,
            vrom_push,
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
        witness: &'a mut TableWitnessIndexSegment<U>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut vrom_push = witness.get_mut_as(self.vrom_push)?;
            for (i, event) in rows.clone().enumerate() {
                vrom_push[i] = event.cond as u64; //  cond_val is 0
            }
        }
        let cpu_rows = rows.map(|event| CpuEvent {
            pc: event.pc.val(),
            next_pc: None,
            fp: event.fp,
            next_fp: None,
            arg0: event.cond,
            arg1: event.target_low.val(),
            arg2: event.target_high.val(),
        });
        self.cpu_cols.populate(witness, cpu_rows)
    }
}
