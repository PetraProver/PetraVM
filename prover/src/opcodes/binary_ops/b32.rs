use std::any::Any;

use binius_m3::builder::{
    upcast_col, upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1,
    B16, B32,
};
use zcrayvm_assembly::{AndiEvent, Opcode, XoriEvent};

use crate::{
    channels::Channels,
    gadgets::cpu::{CpuColumns, CpuColumnsOptions, CpuGadget},
    table::Table,
    types::ProverPackedField,
};

pub struct XoriTable {
    id: TableId,
    cpu_cols: CpuColumns<{ Opcode::Xori as u16 }>,
    dst_abs: Col<B32>, // Virtual
    dst_val: Col<B32>, // Virtual
    src_abs: Col<B32>, // Virtual
    src_val: Col<B32>,
}

impl Table for XoriTable {
    type Event = XoriEvent;

    fn name(&self) -> &'static str {
        "XoriTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("xori");
        let src_val = table.add_committed("src_val");

        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions::default(),
        );
        let dst_abs = table.add_computed("dst_abs", cpu_cols.fp + upcast_col(cpu_cols.arg0));
        let src_abs = table.add_computed("src_abs", cpu_cols.fp + upcast_col(cpu_cols.arg1));
        let imm = cpu_cols.arg2;

        let dst_val = table.add_computed("dst_val", src_val + upcast_expr(imm.into()));

        // Read dst_val
        table.pull(channels.vrom_channel, [dst_abs, dst_val]);

        // Read src_val
        table.pull(channels.vrom_channel, [src_abs, src_val]);

        Self {
            id: table.id(),
            cpu_cols,
            dst_abs,
            dst_val,
            src_abs,
            src_val,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TableFiller<ProverPackedField> for XoriTable {
    type Event = XoriEvent;

    fn id(&self) -> TableId {
        self.id
    }

    // TODO: This implementation might be very similar for all immediate binary
    // operations
    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut dst_abs = witness.get_mut_as(self.dst_abs)?;
            let mut dst_val = witness.get_mut_as(self.dst_val)?;
            let mut src_abs = witness.get_mut_as(self.src_abs)?;
            let mut src_val = witness.get_mut_as(self.src_val)?;
            for (i, event) in rows.clone().enumerate() {
                dst_abs[i] = event.fp.addr(event.dst);
                dst_val[i] = event.dst_val;
                src_abs[i] = event.fp.addr(event.src);
                src_val[i] = event.src_val;
            }
        }
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.into(),
            next_pc: None,
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.src,
            arg2: event.imm,
        });
        self.cpu_cols.populate(witness, cpu_rows)
    }
}

pub struct AndiTable {
    id: TableId,
    cpu_cols: CpuColumns<{ Opcode::Andi as u16 }>,
    dst_abs: Col<B32>,             // Virtual
    src_abs: Col<B32>,             // Virtual
    dst_val_unpacked: Col<B1, 16>, // Virtual
    src_val_unpacked: Col<B1, 32>,
    src_val: Col<B32>, // Virtual
    dst_val: Col<B16>, // Virtual
    /// The lower 16 bits of src_val.
    src_val_low: Col<B1, 16>,
}

impl Table for AndiTable {
    type Event = AndiEvent;

    fn name(&self) -> &'static str {
        "AndiTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("andi");
        let src_val_unpacked: Col<B1, 32> = table.add_committed("src_val");
        let src_val = table.add_packed("src_val", src_val_unpacked);

        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions::default(),
        );

        let dst_abs = table.add_computed("dst_abs", cpu_cols.fp + upcast_col(cpu_cols.arg0));
        let src_abs = table.add_computed("src_abs", cpu_cols.fp + upcast_col(cpu_cols.arg1));
        let imm = cpu_cols.arg2_unpacked;

        let src_val_low: Col<B1, 16> = table.add_selected_block("src_val_low", src_val_unpacked, 0);

        let dst_val_unpacked = table.add_computed("dst_val", src_val_low * imm);
        let dst_val = table.add_packed("dst_val", dst_val_unpacked);

        // Read dst_val
        table.pull(channels.vrom_channel, [dst_abs, upcast_col(dst_val)]);

        // Read src_val
        table.pull(channels.vrom_channel, [src_abs, src_val]);

        Self {
            id: table.id(),
            cpu_cols,
            dst_abs,
            src_abs,
            dst_val,
            src_val,
            dst_val_unpacked,
            src_val_unpacked,
            src_val_low,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TableFiller<ProverPackedField> for AndiTable {
    type Event = AndiEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut dst_abs = witness.get_mut_as(self.dst_abs)?;
            let mut dst_val_unpacked = witness.get_mut_as(self.dst_val_unpacked)?;
            let mut src_abs = witness.get_mut_as(self.src_abs)?;
            let mut src_val_unpacked = witness.get_mut_as(self.src_val_unpacked)?;
            let mut src_val_low = witness.get_mut_as(self.src_val_low)?;
            for (i, event) in rows.clone().enumerate() {
                dst_abs[i] = event.fp.addr(event.dst);
                dst_val_unpacked[i] = event.dst_val as u16;
                src_abs[i] = event.fp.addr(event.src);
                src_val_unpacked[i] = event.src_val;
                src_val_low[i] = event.src_val as u16;
            }
        }
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.into(),
            next_pc: None,
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.src,
            arg2: event.imm,
        });
        self.cpu_cols.populate(witness, cpu_rows)
    }
}
