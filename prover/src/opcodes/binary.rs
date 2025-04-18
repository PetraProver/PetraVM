//! Binary field operation tables for the zCrayVM M3 circuit.
//!
//! This module contains tables for binary field arithmetic operations.

use std::any::Any;

use binius_m3::builder::{
    upcast_col, upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1,
    B32,
};
use zcrayvm_assembly::{opcodes::Opcode, AndEvent, B32MulEvent, OrEvent, XorEvent};

use crate::{
    channels::Channels,
    gadgets::cpu::{CpuColumns, CpuColumnsOptions, CpuGadget, NextPc},
    table::Table,
    types::ProverPackedField,
};

// Constants for opcodes
const B32_MUL_OPCODE: u16 = Opcode::B32Mul as u16;

/// B32_MUL (Binary Field Multiplication) table.
///
/// This table handles the B32_MUL instruction, which performs multiplication
/// in the binary field GF(2^32).
pub struct B32MulTable {
    /// Table ID
    pub id: TableId,
    /// CPU columns
    cpu_cols: CpuColumns<{ Opcode::B32Mul as u16 }>,
    /// First source value
    pub src1_val: Col<B32>,
    /// Second source value
    pub src2_val: Col<B32>,
    /// Result value
    pub dst_val: Col<B32>,
    /// PROM channel pull value
    pub src1_abs_addr: Col<B32>,
    /// Second source absolute address
    pub src2_abs_addr: Col<B32>,
    /// Destination absolute address
    pub dst_abs_addr: Col<B32>,
}

impl Table for B32MulTable {
    type Event = B32MulEvent;

    fn name(&self) -> &'static str {
        "B32MulTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("b32_mul");

        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        let CpuColumns {
            fp,
            arg0: dst,
            arg1: src1,
            arg2: src2,
            ..
        } = cpu_cols;

        let src1_val = table.add_committed("b32_mul_src1_val");
        let src2_val = table.add_committed("b32_mul_src2_val");
        let dst_val = table.add_committed("b32_mul_dst_val");

        // Pull source values from VROM channel
        let src1_abs_addr = table.add_computed("src1_addr", fp + upcast_expr(src1.into()));
        let src2_abs_addr = table.add_computed("src2_addr", fp + upcast_expr(src2.into()));
        table.pull(channels.vrom_channel, [src1_abs_addr, src1_val]);
        table.pull(channels.vrom_channel, [src2_abs_addr, src2_val]);

        table.assert_zero("check_b32_mul_result", src1_val * src2_val - dst_val);

        // Pull result from VROM channel
        let dst_abs_addr = table.add_computed("dst_addr", fp + upcast_expr(dst.into()));
        table.pull(channels.vrom_channel, [dst_abs_addr, dst_val]);

        Self {
            id: table.id(),
            cpu_cols,
            src1_val,
            src2_val,
            dst_val,
            src1_abs_addr,
            src2_abs_addr,
            dst_abs_addr,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TableFiller<ProverPackedField> for B32MulTable {
    type Event = B32MulEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            let mut src1_val_col = witness.get_scalars_mut(self.src1_val)?;
            let mut src2_val_col = witness.get_scalars_mut(self.src2_val)?;
            let mut dst_val_col = witness.get_scalars_mut(self.dst_val)?;
            let mut src1_abs_addr_col = witness.get_scalars_mut(self.src1_abs_addr)?;
            let mut src2_abs_addr_col = witness.get_scalars_mut(self.src2_abs_addr)?;
            let mut dst_abs_addr_col = witness.get_scalars_mut(self.dst_abs_addr)?;

            for (i, event) in rows.clone().enumerate() {
                src1_val_col[i] = B32::new(event.src1_val);
                src2_val_col[i] = B32::new(event.src2_val);
                dst_val_col[i] = B32::new(event.dst_val);
                src1_abs_addr_col[i] = B32::new(event.fp.addr(event.src1));
                src2_abs_addr_col[i] = B32::new(event.fp.addr(event.src2));
                dst_abs_addr_col[i] = B32::new(event.fp.addr(event.dst));
            }
        }
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.val(),
            next_pc: None,
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.src1,
            arg2: event.src2,
        });
        self.cpu_cols.populate(witness, cpu_rows)
    }
}

pub struct XorTable {
    /// Table ID
    id: TableId,
    /// CPU columns
    cpu_cols: CpuColumns<{ Opcode::Xor as u16 }>,
    /// First source value
    pub src1_val: Col<B32>,
    /// Second source value
    pub src2_val: Col<B32>,
    /// Result value
    pub dst_val: Col<B32>,
    /// PROM channel pull value
    pub src1_abs_addr: Col<B32>,
    /// Second source absolute address
    pub src2_abs_addr: Col<B32>,
    /// Destination absolute address
    pub dst_abs_addr: Col<B32>,
}

impl Table for XorTable {
    type Event = XorEvent;

    fn name(&self) -> &'static str {
        "XorTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("xor");
        let src1_val = table.add_committed("src1_val");
        let src2_val = table.add_committed("src2_val");

        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions::default(),
        );
        let dst_abs_addr =
            table.add_computed("dst_abs_addr", cpu_cols.fp + upcast_col(cpu_cols.arg0));
        let src1_abs_addr =
            table.add_computed("src1_abs_addr", cpu_cols.fp + upcast_col(cpu_cols.arg1));
        let src2_abs_addr =
            table.add_computed("src2_abs_addr", cpu_cols.fp + upcast_col(cpu_cols.arg2));

        let dst_val = table.add_computed("dst_val", src1_val + src2_val);

        // Read src1_val and src2_val
        table.pull(channels.vrom_channel, [src1_abs_addr, src1_val]);
        table.pull(channels.vrom_channel, [src2_abs_addr, src2_val]);

        // Read dst_val
        table.pull(channels.vrom_channel, [dst_abs_addr, dst_val]);

        Self {
            id: table.id(),
            cpu_cols,
            src1_abs_addr,
            src1_val,
            src2_abs_addr,
            src2_val,
            dst_abs_addr,
            dst_val,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TableFiller<ProverPackedField> for XorTable {
    type Event = XorEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut dst_abs_addr = witness.get_mut_as(self.dst_abs_addr)?;
            let mut dst_val = witness.get_mut_as(self.dst_val)?;
            let mut src1_abs_addr = witness.get_mut_as(self.src1_abs_addr)?;
            let mut src1_val = witness.get_mut_as(self.src1_val)?;
            let mut src2_abs_addr = witness.get_mut_as(self.src2_abs_addr)?;
            let mut src2_val = witness.get_mut_as(self.src2_val)?;
            for (i, event) in rows.clone().enumerate() {
                dst_abs_addr[i] = event.fp.addr(event.dst);
                dst_val[i] = event.dst_val;
                src1_abs_addr[i] = event.fp.addr(event.src1);
                src1_val[i] = event.src1_val;
                src2_abs_addr[i] = event.fp.addr(event.src2);
                src2_val[i] = event.src2_val;
            }
        }
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.into(),
            next_pc: None,
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.src1,
            arg2: event.src2,
        });
        self.cpu_cols.populate(witness, cpu_rows)
    }
}

pub struct AndTable {
    /// Table ID
    id: TableId,
    /// CPU columns
    cpu_cols: CpuColumns<{ Opcode::And as u16 }>,
    /// First source value
    pub src1_val: Col<B32>,
    /// First source value, unpacked
    src1_val_unpacked: Col<B1, 32>,
    /// Second source value
    pub src2_val: Col<B32>,
    /// Second source value, unpacked
    src2_val_unpacked: Col<B1, 32>,
    /// Result value
    pub dst_val: Col<B32>,
    /// Result value, unpacked
    dst_val_unpacked: Col<B1, 32>,
    /// PROM channel pull value
    pub src1_abs_addr: Col<B32>,
    /// Second source absolute address
    pub src2_abs_addr: Col<B32>,
    /// Destination absolute address
    pub dst_abs_addr: Col<B32>,
}

impl Table for AndTable {
    type Event = AndEvent;

    fn name(&self) -> &'static str {
        "AndTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("and");
        let src1_val_unpacked: Col<B1, 32> = table.add_committed("src1_val");
        let src1_val = table.add_packed("src1_val", src1_val_unpacked);
        let src2_val_unpacked: Col<B1, 32> = table.add_committed("src2_val");
        let src2_val = table.add_packed("src2_val", src2_val_unpacked);

        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions::default(),
        );

        let dst_abs_addr =
            table.add_computed("dst_abs_addr", cpu_cols.fp + upcast_col(cpu_cols.arg0));
        let src1_abs_addr =
            table.add_computed("src1_abs_addr", cpu_cols.fp + upcast_col(cpu_cols.arg1));
        let src2_abs_addr =
            table.add_computed("src2_abs_addr", cpu_cols.fp + upcast_col(cpu_cols.arg2));

        let dst_val_unpacked =
            table.add_computed("dst_val_unpacked", src1_val_unpacked * src2_val_unpacked);
        let dst_val = table.add_packed("dst_val", dst_val_unpacked);

        // Read src1_val and src2_val
        table.pull(channels.vrom_channel, [src1_abs_addr, src1_val]);
        table.pull(channels.vrom_channel, [src2_abs_addr, src2_val]);

        // Read dst_val
        table.pull(channels.vrom_channel, [dst_abs_addr, dst_val]);

        Self {
            id: table.id(),
            cpu_cols,
            src1_abs_addr,
            src1_val,
            src1_val_unpacked,
            src2_abs_addr,
            src2_val,
            src2_val_unpacked,
            dst_abs_addr,
            dst_val,
            dst_val_unpacked,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TableFiller<ProverPackedField> for AndTable {
    type Event = AndEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut dst_abs_addr = witness.get_mut_as(self.dst_abs_addr)?;
            let mut dst_val_unpacked = witness.get_mut_as(self.dst_val_unpacked)?;
            let mut src1_abs_addr = witness.get_mut_as(self.src1_abs_addr)?;
            let mut src1_val_unpacked = witness.get_mut_as(self.src1_val_unpacked)?;
            let mut src2_abs_addr = witness.get_mut_as(self.src2_abs_addr)?;
            let mut src2_val_unpacked = witness.get_mut_as(self.src2_val_unpacked)?;
            for (i, event) in rows.clone().enumerate() {
                dst_abs_addr[i] = event.fp.addr(event.dst);
                dst_val_unpacked[i] = event.dst_val;
                src1_abs_addr[i] = event.fp.addr(event.src1);
                src1_val_unpacked[i] = event.src1_val;
                src2_abs_addr[i] = event.fp.addr(event.src2);
                src2_val_unpacked[i] = event.src2_val;
            }
        }
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.into(),
            next_pc: None,
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.src1,
            arg2: event.src2,
        });
        self.cpu_cols.populate(witness, cpu_rows)
    }
}

pub struct OrTable {
    /// Table ID
    id: TableId,
    /// CPU columns
    cpu_cols: CpuColumns<{ Opcode::Or as u16 }>,
    /// First source value
    pub src1_val: Col<B32>,
    /// First source value, unpacked
    src1_val_unpacked: Col<B1, 32>,
    /// Second source value
    pub src2_val: Col<B32>,
    /// Second source value, unpacked
    src2_val_unpacked: Col<B1, 32>,
    /// Result value
    pub dst_val: Col<B32>,
    /// Result value, unpacked
    dst_val_unpacked: Col<B1, 32>,
    /// PROM channel pull value
    pub src1_abs_addr: Col<B32>,
    /// Second source absolute address
    pub src2_abs_addr: Col<B32>,
    /// Destination absolute address
    pub dst_abs_addr: Col<B32>,
}

impl Table for OrTable {
    type Event = OrEvent;

    fn name(&self) -> &'static str {
        "OrTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("or");
        let src1_val_unpacked: Col<B1, 32> = table.add_committed("src1_val");
        let src1_val = table.add_packed("src1_val", src1_val_unpacked);
        let src2_val_unpacked: Col<B1, 32> = table.add_committed("src2_val");
        let src2_val = table.add_packed("src2_val", src2_val_unpacked);

        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions::default(),
        );

        let dst_abs_addr =
            table.add_computed("dst_abs_addr", cpu_cols.fp + upcast_col(cpu_cols.arg0));
        let src1_abs_addr =
            table.add_computed("src1_abs_addr", cpu_cols.fp + upcast_col(cpu_cols.arg1));
        let src2_abs_addr =
            table.add_computed("src2_abs_addr", cpu_cols.fp + upcast_col(cpu_cols.arg2));

        let dst_val_unpacked = table.add_computed(
            "dst_val_unpacked",
            // DeMorgan Law: a | b == a + b + (a * b)
            src1_val_unpacked + src2_val_unpacked + (src1_val_unpacked * src2_val_unpacked),
        );
        let dst_val = table.add_packed("dst_val", dst_val_unpacked);

        // Read src1_val and src2_val
        table.pull(channels.vrom_channel, [src1_abs_addr, src1_val]);
        table.pull(channels.vrom_channel, [src2_abs_addr, src2_val]);

        // Read dst_val
        table.pull(channels.vrom_channel, [dst_abs_addr, dst_val]);

        Self {
            id: table.id(),
            cpu_cols,
            src1_abs_addr,
            src1_val,
            src1_val_unpacked,
            src2_abs_addr,
            src2_val,
            src2_val_unpacked,
            dst_abs_addr,
            dst_val,
            dst_val_unpacked,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TableFiller<ProverPackedField> for OrTable {
    type Event = OrEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut dst_abs_addr = witness.get_mut_as(self.dst_abs_addr)?;
            let mut dst_val_unpacked = witness.get_mut_as(self.dst_val_unpacked)?;
            let mut src1_abs_addr = witness.get_mut_as(self.src1_abs_addr)?;
            let mut src1_val_unpacked = witness.get_mut_as(self.src1_val_unpacked)?;
            let mut src2_abs_addr = witness.get_mut_as(self.src2_abs_addr)?;
            let mut src2_val_unpacked = witness.get_mut_as(self.src2_val_unpacked)?;

            for (i, event) in rows.clone().enumerate() {
                dst_abs_addr[i] = event.fp.addr(event.dst);
                dst_val_unpacked[i] = event.dst_val;
                src1_abs_addr[i] = event.fp.addr(event.src1);
                src1_val_unpacked[i] = event.src1_val;
                src2_abs_addr[i] = event.fp.addr(event.src2);
                src2_val_unpacked[i] = event.src2_val;
            }
        }
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.into(),
            next_pc: None,
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.src1,
            arg2: event.src2,
        });
        self.cpu_cols.populate(witness, cpu_rows)
    }
}
