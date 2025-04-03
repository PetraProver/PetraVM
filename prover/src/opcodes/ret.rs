use binius_field::Field;
use binius_m3::builder::{Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B32};
use zcrayvm_assembly::{Opcode, RetEvent};

use super::cpu::{CpuColumns, CpuColumnsOptions, CpuEvent, NextPc};
use crate::{channels::Channels, types::CommonTableBounds};
pub struct RetTable {
    id: TableId,
    cpu_cols: CpuColumns<{ Opcode::Ret as u16 }>,
    fp_xor_1: Col<B32>, // Virtual
    next_pc: Col<B32>,
    next_fp: Col<B32>,
}

impl RetTable {
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("ret");
        let next_pc = table.add_committed("next_pc");
        let next_fp = table.add_committed("next_fp");

        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Target(next_pc),
                next_fp: Some(next_fp),
            },
        );

        let fp0 = cpu_cols.fp;
        let fp_xor_1 = table.add_computed("fp_xor_1", fp0 + B32::ONE);

        // Read the next_pc
        table.pull(channels.vrom_channel, [next_pc, fp0]);

        //Read the next_fp
        table.pull(channels.vrom_channel, [fp_xor_1, next_fp]);

        Self {
            id: table.id(),
            cpu_cols,
            fp_xor_1,
            next_pc,
            next_fp,
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
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<U>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut fp_xor_1 = witness.get_mut_as(self.fp_xor_1)?;
            let mut next_pc = witness.get_mut_as(self.next_pc)?;
            let mut next_fp = witness.get_mut_as(self.next_fp)?;
            for (i, event) in rows.clone().enumerate() {
                fp_xor_1[i] = *event.fp ^ 1;
                next_pc[i] = event.pc_next;
                next_fp[i] = event.fp_next;
            }
        }
        let cpu_rows = rows.map(|event| CpuEvent {
            pc: event.pc.into(),
            next_pc: Some(event.pc_next),
            fp: *event.fp,
            ..Default::default()
        });
        self.cpu_cols.populate(witness, cpu_rows)
    }
}
