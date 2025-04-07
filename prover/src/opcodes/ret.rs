use binius_field::Field;
use binius_m3::builder::{Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B32};
use zcrayvm_assembly::{Opcode, RetEvent};

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
use super::cpu::{CpuColumns, CpuColumnsOptions, CpuEvent, NextPc};
use crate::{channels::Channels, types::ProverPackedField};
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
        table.pull(channels.vrom_channel, [fp0, next_pc]);

        // Read the next_fp
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
            let mut fp_xor_1 = witness.get_mut_as(self.fp_xor_1)?;
            let mut next_pc = witness.get_mut_as(self.next_pc)?;
            let mut next_fp = witness.get_mut_as(self.next_fp)?;
            for (i, event) in rows.clone().enumerate() {
                fp_xor_1[i] = event.fp.addr(1u32);
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
