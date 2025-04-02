use binius_field::{as_packed_field::PackScalar, underlier::UnderlierType, Field};
use binius_m3::builder::{
    Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1, B32, B64,
};
use bytemuck::Pod;
use zcrayvm_assembly::{Opcode, RetEvent};

use super::{
    cpu::{CpuColumns, CpuColumnsOptions, CpuEvent, NextPc},
    util::pack_b32_into_b64,
};
use crate::channels::ZkVMChannels;
pub struct RetTable {
    id: TableId,
    cpu_cols: CpuColumns<{ Opcode::Ret as u16 }>,
    fp_xor_1: Col<B32>, // Virtual
    next_pc: Col<B32>,
    next_fp: Col<B32>,

    vrom_next_pc: Col<B64>, // Virtual
    vrom_next_fp: Col<B64>, // Virtual
}

impl RetTable {
    pub fn new(cs: &mut ConstraintSystem, channels: &ZkVMChannels) -> Self {
        let mut table = cs.add_table("ret");
        let next_pc = table.add_committed("next_pc");
        let next_fp = table.add_committed("next_fp");

        let ZkVMChannels {
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
                next_pc: NextPc::Target(next_pc),
                next_fp: Some(next_fp),
            },
        );

        let fp0 = cpu_cols.fp;
        let fp_xor_1 = table.add_computed("fp_xor_1", fp0 + B32::ONE);

        // Read the next_pc
        let vrom_next_pc = table.add_computed(
            "fp0_next_pc",
            pack_b32_into_b64([next_pc.into(), fp0.into()]),
        );
        table.pull(vrom_channel, [vrom_next_pc]);

        //Read the next_fp
        let vrom_next_fp = table.add_computed(
            "vrom_next_fp",
            pack_b32_into_b64([fp_xor_1.into(), next_fp.into()]),
        );
        table.pull(vrom_channel, [vrom_next_fp]);

        Self {
            id: table.id(),
            cpu_cols,
            fp_xor_1,
            next_pc,
            next_fp,
            vrom_next_pc,
            vrom_next_fp,
        }
    }
}

impl<U: UnderlierType> TableFiller<U> for RetTable
where
    U: Pod + PackScalar<B1>,
{
    type Event = RetEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<U>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut fp_xor_1 = witness.get_mut_as(self.fp_xor_1)?;
            let mut next_pc = witness.get_mut_as(self.next_pc)?;
            let mut next_fp = witness.get_mut_as(self.next_fp)?;
            let mut vrom_next_pc = witness.get_mut_as(self.vrom_next_pc)?;
            let mut vrom_next_fp = witness.get_mut_as(self.vrom_next_fp)?;
            for (i, event) in rows.clone().enumerate() {
                fp_xor_1[i] = event.fp ^ 1;
                next_pc[i] = event.fp_0_val;
                next_fp[i] = event.fp_1_val;
                vrom_next_pc[i] = (event.fp_0_val as u64) << 32 | event.fp as u64;
                vrom_next_fp[i] = (event.fp_1_val as u64) << 32 | event.fp as u64 ^ 1;
            }
        }
        let cpu_rows = rows.map(|event| CpuEvent {
            pc: event.pc.into(),
            next_pc: Some(event.fp_0_val),
            fp: event.fp,
            next_fp: Some(event.fp_1_val),
            ..Default::default()
        });
        self.cpu_cols.populate(witness, cpu_rows)
    }
}
