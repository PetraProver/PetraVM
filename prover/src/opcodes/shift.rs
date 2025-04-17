use binius_core::oracle::ShiftVariant;
use binius_m3::builder::{
    upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1, B32,
};
use zcrayvm_assembly::{Opcode, SrliEvent};

use crate::{
    channels::Channels,
    gadgets::{
        barrel_shifter::{BarrelShifter, BarrelShifterFlags},
        cpu::{CpuColumns, CpuColumnsOptions, CpuGadget},
    },
    table::Table,
    types::ProverPackedField,
};

/// Maximum number of bits of the shift amount, i.e. 0 < shift_ammount < 1 <<
/// SHIFT_MAX_BITS - 1 = 31 where dst_val = src_val >> shift_amount or dst_val =
/// src_val << shift_amount
const MAX_SHIFT_BITS: usize = 5;

/// Table for the SRLI (Shift Right Logical Immediate) instruction. It
/// constraints the values src_val  to be equal to dst_val << shift_amount. The
/// shift amount is given as an immediate. In addition to the standard CPU
/// columns and src, dst columns, it also includes `MAX_SHIFT_BITS` partial
/// shift columns to constraint intermediate results of the shift operation.
pub struct SrliTable {
    id: TableId,
    cpu_cols: CpuColumns<{ Opcode::Srli as u16 }>,
    shifter: BarrelShifter,
    dst_abs: Col<B32>, // Virtual
    dst_val: Col<B32>, // Virtual
    src_abs: Col<B32>, // Virtual
    src_val: Col<B32>, // Virtual
}

impl Table for SrliTable {
    type Event = SrliEvent;

    fn name(&self) -> &'static str {
        "SrliTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("srli");
        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions::default(),
        );

        let src_val_unpacked: Col<B1, 32> = table.add_committed("src_val_unpacked");
        let src_val: Col<B32> = table.add_packed("src_val", src_val_unpacked);
        let dst_abs = table.add_computed("dst_abs", cpu_cols.fp + upcast_col(cpu_cols.arg0));
        let src_abs = table.add_computed("src_abs", cpu_cols.fp + upcast_col(cpu_cols.arg1));

        let shifter = BarrelShifter::new(
            &mut table,
            src_val_unpacked,
            cpu_cols.arg2_unpacked,
            BarrelShifterFlags {
                variant: ShiftVariant::LogicalRight,
                commit_output: false,
            },
        );

        let dst_val = table.add_packed("dst_val", shifter.output);

        table.pull(channels.vrom_channel, [dst_abs, dst_val]);
        table.pull(channels.vrom_channel, [src_abs, src_val]);

        Self {
            id: table.id(),
            cpu_cols,
            shifter,
            dst_abs,
            dst_val,
            src_abs,
            src_val,
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl TableFiller<ProverPackedField> for SrliTable {
    type Event = SrliEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            let mut src_val = witness.get_mut_as(self.src_val)?;
            let mut dst_abs = witness.get_mut_as(self.dst_abs)?;
            let mut src_abs = witness.get_mut_as(self.src_abs)?;

            for (i, event) in rows.clone().enumerate() {
                src_val[i] = event.src_val;
                dst_abs[i] = event.fp.addr(event.dst as u32);
                src_abs[i] = event.fp.addr(event.src as u32);
            }
        }
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.val(),
            next_pc: None,
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.src,
            arg2: event.shift_amount as u16,
            ..Default::default()
        });
        self.cpu_cols.populate(witness, cpu_rows)?;
        self.shifter.populate(witness)?;
        Ok(())
    }
}
