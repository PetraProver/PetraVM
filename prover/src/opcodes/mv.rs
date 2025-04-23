//! Move Value tables implementation for the zCrayVM M3 circuit.

use std::any::Any;

use binius_m3::builder::{
    upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B32,
};
use zcrayvm_assembly::MvihEvent;
use zcrayvm_assembly::{opcodes::Opcode, MvvwEvent};

use crate::gadgets::cpu::{CpuColumns, CpuColumnsOptions, CpuGadget, NextPc};
use crate::table::Table;
use crate::{channels::Channels, types::ProverPackedField};

/// MVV.W (Move Value to Value) table implementation.
///
/// This table verifies the Move Value to Value (word) instruction, which moves
/// a 32-bit value from one VROM location to another, with optional offset
/// addressing.
pub struct MvvwTable {
    /// Table identifier
    pub id: TableId,
    /// CPU-related columns for instruction handling
    cpu_cols: CpuColumns<{ Opcode::Mvvw as u16 }>,
    /// Base destination address (FP + dst)
    dst_abs_addr: Col<B32>,
    /// Base source address (FP + src)
    src_abs_addr: Col<B32>,
    /// Final destination address with offset (dst_addr + offset)
    final_dst_addr: Col<B32>,
    /// Destination address value from VROM
    dst_addr: Col<B32>,
    /// Value to be moved (from src_abs_addr)
    src_val: Col<B32>,
}

impl Table for MvvwTable {
    type Event = MvvwEvent;

    fn name(&self) -> &'static str {
        "MvvwTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("mvvw");

        // Set up CPU columns with standard instruction handling
        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        // Extract instruction arguments from CPU columns
        let CpuColumns {
            fp,
            arg0: dst,
            arg1: offset,
            arg2: src,
            ..
        } = cpu_cols;

        // Compute absolute addresses for source and destination
        let dst_abs_addr = table.add_computed("dst_abs_addr", fp + upcast_expr(dst.into()));
        let src_abs_addr = table.add_computed("src_abs_addr", fp + upcast_expr(src.into()));

        // Value to be moved from source
        let src_val = table.add_committed("src_val");

        // Read the value at dst_abs_addr (this is the base address for final
        // destination)
        let dst_addr = table.add_committed("dst_addr");
        table.pull(channels.vrom_channel, [dst_abs_addr, dst_addr]);

        // Compute final destination address with offset
        let final_dst_addr =
            table.add_computed("final_dst_addr", dst_addr + upcast_expr(offset.into()));

        // Read source value from VROM
        table.pull(channels.vrom_channel, [src_abs_addr, src_val]);

        // Verify the source value is written to the final destination address
        table.pull(channels.vrom_channel, [final_dst_addr, src_val]);

        Self {
            id: table.id(),
            cpu_cols,
            dst_abs_addr,
            src_abs_addr,
            final_dst_addr,
            dst_addr,
            src_val,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TableFiller<ProverPackedField> for MvvwTable {
    type Event = MvvwEvent;

    fn id(&self) -> TableId {
        self.id
    }

    /// Fill the table witness with data from MVV.W events
    ///
    /// This populates the witness data based on the execution events from
    /// the corresponding assembly MVV.W operations.
    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            // Get mutable references to witness columns
            let mut dst_abs_addr = witness.get_scalars_mut(self.dst_abs_addr)?;
            let mut src_abs_addr = witness.get_scalars_mut(self.src_abs_addr)?;
            let mut final_dst_addr = witness.get_scalars_mut(self.final_dst_addr)?;
            let mut dst_addr = witness.get_scalars_mut(self.dst_addr)?;
            let mut src_val = witness.get_scalars_mut(self.src_val)?;

            // Fill the witness columns with values from each event
            for (i, event) in rows.clone().enumerate() {
                dst_abs_addr[i] = B32::new(event.fp.addr(event.dst));
                src_abs_addr[i] = B32::new(event.fp.addr(event.src));
                dst_addr[i] = B32::new(event.dst_addr);
                final_dst_addr[i] = B32::new(event.dst_addr ^ event.offset as u32);
                src_val[i] = B32::new(event.src_val);
            }
        }

        // Create CPU gadget rows from events
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.val(),
            next_pc: None, // NextPc::Increment handled by CPU columns
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.offset,
            arg2: event.src,
        });

        // Populate CPU columns with the gadget rows
        self.cpu_cols.populate(witness, cpu_rows)
    }
}

/// MVI.H (Move Immediate Half‐word) table implementation.
///
/// VROM[ fp[dst] + offset ] = zero_extend(imm)
pub struct MvihTable {
    pub id: TableId,
    cpu_cols: CpuColumns<{ Opcode::Mvih as u16 }>,
    dst_abs_addr: Col<B32>,
    dst_addr: Col<B32>,
    final_dst_addr: Col<B32>,
    imm_val: Col<B32>,
}

impl Table for MvihTable {
    type Event = MvihEvent;

    fn name(&self) -> &'static str {
        "MvihTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("mvih");

        // CPU columns (pc, fp, args)
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
            arg1: offset,
            arg2: imm,
            ..
        } = cpu_cols;

        // Compute base address
        let dst_abs_addr = table.add_computed("dst_abs_addr", fp + upcast_expr(dst.into()));

        // Pull the base pointer from VROM
        let dst_addr = table.add_committed("dst_addr");
        table.pull(channels.vrom_channel, [dst_abs_addr, dst_addr]);

        // Compute actual destination slot
        let final_dst_addr =
            table.add_computed("final_dst_addr", dst_addr + upcast_expr(offset.into()));

        // Lift the 16‑bit immediate to 32 bits
        let imm_val = table.add_computed("imm_val", upcast_expr(imm.into()));

        // Verify the immediate write into VROM
        table.pull(channels.vrom_channel, [final_dst_addr, imm_val]);

        Self {
            id: table.id(),
            cpu_cols,
            dst_abs_addr,
            dst_addr,
            final_dst_addr,
            imm_val,
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl TableFiller<ProverPackedField> for MvihTable {
    type Event = MvihEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            // Fill VROM reads/writes
            let mut dst_abs_addr_col = witness.get_scalars_mut(self.dst_abs_addr)?;
            let mut dst_addr_col = witness.get_scalars_mut(self.dst_addr)?;
            let mut final_dst_addr_col = witness.get_scalars_mut(self.final_dst_addr)?;
            let mut imm_col = witness.get_scalars_mut(self.imm_val)?;

            for (i, ev) in rows.clone().enumerate() {
                dst_abs_addr_col[i] = B32::new(ev.fp.addr(ev.dst));
                dst_addr_col[i] = B32::new(ev.dst_addr);
                final_dst_addr_col[i] = B32::new(ev.dst_addr ^ ev.offset as u32);
                imm_col[i] = B32::new(ev.imm as u32);
            }
        }

        // Fill CPU‐side columns (pc, fp, dst, offset, imm)
        let cpu_rows = rows.map(|ev| CpuGadget {
            pc: ev.pc.val(),
            next_pc: None,
            fp: *ev.fp,
            arg0: ev.dst,
            arg1: ev.offset,
            arg2: ev.imm,
        });

        self.cpu_cols.populate(witness, cpu_rows)
    }
}
