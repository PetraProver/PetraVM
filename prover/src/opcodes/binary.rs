//! Binary field operation tables for the zCrayVM M3 circuit.
//!
//! This module contains tables for binary field arithmetic operations.

use std::any::Any;

use binius_field::BinaryField;
use binius_m3::builder::{
    upcast_col, upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1,
    B16, B32,
};
use zcrayvm_assembly::{
    opcodes::Opcode, AndEvent, AndiEvent, B32MulEvent, B32MuliEvent, OrEvent, OriEvent, XorEvent,
    XoriEvent,
};

use crate::{
    channels::Channels,
    gadgets::cpu::{CpuColumns, CpuColumnsOptions, CpuGadget, NextPc},
    table::Table,
    types::ProverPackedField,
    utils::pack_b16_into_b32,
};

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

        // Pull source values from VROM channel
        let src1_abs_addr = table.add_computed("src1_addr", fp + upcast_expr(src1.into()));
        let src2_abs_addr = table.add_computed("src2_addr", fp + upcast_expr(src2.into()));
        table.pull(channels.vrom_channel, [src1_abs_addr, src1_val]);
        table.pull(channels.vrom_channel, [src2_abs_addr, src2_val]);

        // Compute the result
        let dst_val = table.add_computed("b32_mul_dst_val", src1_val * src2_val);

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

pub struct OriTable {
    /// Table ID
    id: TableId,
    /// CPU columns
    cpu_cols: CpuColumns<{ Opcode::Ori as u16 }>,
    /// Source value
    pub src_val: Col<B32>,
    /// Source value, unpacked
    src_val_unpacked: Col<B1, 32>,
    // TODO: `imm` and `imm_32b_unpacked` should not need to be part of this table as fetched
    // directly from the CPU gadget. Revamp this once a new version of `ZeroPadding` is implemented
    // on the binius side.
    /// Immediate value
    imm: Col<B1, 16>,
    /// Immediate value, unpacked
    imm_32b_unpacked: Col<B1, 32>,
    /// Result value
    pub dst_val: Col<B32>,
    /// Result value, unpacked
    dst_val_unpacked: Col<B1, 32>,
    /// PROM channel pull value
    pub src_abs_addr: Col<B32>,
    /// Destination absolute address
    pub dst_abs_addr: Col<B32>,
}

impl Table for OriTable {
    type Event = OriEvent;

    fn name(&self) -> &'static str {
        "OriTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("ori");
        let src_val_unpacked: Col<B1, 32> = table.add_committed("src_val");
        let src_val = table.add_packed("src_val", src_val_unpacked);
        let imm_32b_unpacked: Col<B1, 32> = table.add_committed("imm_32b");

        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions::default(),
        );

        let dst_abs_addr =
            table.add_computed("dst_abs_addr", cpu_cols.fp + upcast_col(cpu_cols.arg0));
        let src_abs_addr =
            table.add_computed("src_abs_addr", cpu_cols.fp + upcast_col(cpu_cols.arg1));

        let imm: Col<B1, 16> = table.add_selected_block("imm", imm_32b_unpacked, 0);
        table.assert_zero("imm_check", imm - cpu_cols.arg2_unpacked);

        let imm_high: Col<B1, 16> = table.add_selected_block("imm_high", imm_32b_unpacked, 1);
        table.assert_zero("imm_high_check", imm_high.into());

        let dst_val_unpacked = table.add_computed(
            "dst_val_unpacked",
            // DeMorgan Law: a | b == a + b + (a * b)
            src_val_unpacked + imm_32b_unpacked + (src_val_unpacked * imm_32b_unpacked),
        );
        let dst_val = table.add_packed("dst_val", dst_val_unpacked);

        // Read src_val
        table.pull(channels.vrom_channel, [src_abs_addr, src_val]);

        // Read dst_val
        table.pull(channels.vrom_channel, [dst_abs_addr, dst_val]);

        Self {
            id: table.id(),
            cpu_cols,
            src_abs_addr,
            src_val,
            src_val_unpacked,
            imm,
            imm_32b_unpacked,
            dst_abs_addr,
            dst_val,
            dst_val_unpacked,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TableFiller<ProverPackedField> for OriTable {
    type Event = OriEvent;

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
            let mut src_abs_addr = witness.get_mut_as(self.src_abs_addr)?;
            let mut src_val_unpacked = witness.get_mut_as(self.src_val_unpacked)?;
            let mut imm_32b_unpacked = witness.get_mut_as(self.imm_32b_unpacked)?;
            let mut imm = witness.get_mut_as(self.imm)?;

            for (i, event) in rows.clone().enumerate() {
                dst_abs_addr[i] = event.fp.addr(event.dst);
                dst_val_unpacked[i] = event.dst_val;
                src_abs_addr[i] = event.fp.addr(event.src);
                src_val_unpacked[i] = event.src_val;
                imm[i] = event.imm;
                imm_32b_unpacked[i] = event.imm as u32;
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

/// B32_MULI (Binary Field Multiplication with Immediate) table.
///
/// This table handles the B32_MULI instruction, which performs multiplication
/// in the binary field GF(2^32) with a 32-bit immediate value.
/// This operation is special as it spans two instructions, with the immediate
/// split across them.
pub struct B32MuliTable {
    /// Table ID
    pub id: TableId,
    /// CPU columns for first instruction
    cpu_cols_first: CpuColumns<{ Opcode::B32Muli as u16 }>,
    /// CPU columns for second instruction
    cpu_cols_second: CpuColumns<{ Opcode::B32Muli as u16 }>,
    /// Source value
    pub src_val: Col<B32>,
    /// Immediate value (32-bit constructed from two 16-bit values)
    pub imm_val: Col<B32>,
    /// Result value
    pub dst_val: Col<B32>,
    /// Source absolute address
    pub src_abs_addr: Col<B32>,
    /// Destination absolute address
    pub dst_abs_addr: Col<B32>,
}

impl Table for B32MuliTable {
    type Event = B32MuliEvent;

    fn name(&self) -> &'static str {
        "B32MuliTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("b32_muli");

        // First instruction - captures the initial opcode, dst, src, and imm_low
        let cpu_cols_first = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        // Second instruction - captures the opcode continuation with imm_high
        let cpu_cols_second = CpuColumns::new(
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
            arg1: src,
            arg2: imm_low_col,
            ..
        } = cpu_cols_first;

        let CpuColumns {
            arg0: imm_high_col, ..
        } = cpu_cols_second;

        // Create columns for values
        let src_val = table.add_committed("b32_muli_src_val");

        // Construct the 32-bit immediate from the two 16-bit parts
        let imm_val = table.add_computed(
            "b32_muli_imm_val",
            pack_b16_into_b32([imm_low_col.into(), imm_high_col.into()]),
        );

        // Pull source value from VROM channel
        let src_abs_addr = table.add_computed("src_addr", fp + upcast_expr(src.into()));
        table.pull(channels.vrom_channel, [src_abs_addr, src_val]);

        // Compute the result
        let dst_val = table.add_computed("b32_muli_dst_val", src_val * imm_val);

        // Pull result from VROM channel
        let dst_abs_addr = table.add_computed("dst_addr", fp + upcast_expr(dst.into()));
        table.pull(channels.vrom_channel, [dst_abs_addr, dst_val]);

        Self {
            id: table.id(),
            cpu_cols_first,
            cpu_cols_second,
            src_val,
            imm_val,
            dst_val,
            src_abs_addr,
            dst_abs_addr,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TableFiller<ProverPackedField> for B32MuliTable {
    type Event = B32MuliEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            let mut src_val_col = witness.get_scalars_mut(self.src_val)?;
            let mut imm_val_col = witness.get_scalars_mut(self.imm_val)?;
            let mut dst_val_col = witness.get_scalars_mut(self.dst_val)?;
            let mut src_abs_addr_col = witness.get_scalars_mut(self.src_abs_addr)?;
            let mut dst_abs_addr_col = witness.get_scalars_mut(self.dst_abs_addr)?;

            for (i, event) in rows.clone().enumerate() {
                src_val_col[i] = B32::new(event.src_val);
                imm_val_col[i] = B32::new(event.imm);
                dst_val_col[i] = B32::new(event.dst_val);
                src_abs_addr_col[i] = B32::new(event.fp.addr(event.src));
                dst_abs_addr_col[i] = B32::new(event.fp.addr(event.dst));
            }
        }

        // Populate the first instruction CPU rows
        let cpu_rows_first = rows.clone().map(|event| CpuGadget {
            pc: event.pc.val(),
            next_pc: None, // NextPc::Increment handles this
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.src,
            arg2: event.imm as u16, // imm_low
        });

        // Populate the second instruction CPU rows
        let cpu_rows_second = rows.map(|event| CpuGadget {
            pc: (event.pc * B32::MULTIPLICATIVE_GENERATOR).val(), // PC for the second instruction
            next_pc: None,                                        // NextPc::Increment handles this
            fp: *event.fp,
            arg0: (event.imm >> 16) as u16, // imm_high
            arg1: 0,                        /* These args should be 0 according to
                                             * B32MuliEvent::generate */
            arg2: 0, // These args should be 0 according to B32MuliEvent::generate
        });

        self.cpu_cols_first.populate(witness, cpu_rows_first)?;
        self.cpu_cols_second.populate(witness, cpu_rows_second)?;

        Ok(())
    }
}
