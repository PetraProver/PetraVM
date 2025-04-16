//! MVV.W (Move Value to Value) table implementation for the zCrayVM M3 circuit.
//!
//! This module contains the MVV.W table which handles moving values between
//! VROM locations in the zCrayVM execution.
//!
//! Note: The assembly implementation of MVV.W (in assembly/src/event/mv.rs)
//! includes a complex system for handling cases where source or destination
//! addresses might not be available yet, using "pending updates" and
//! "delegate_move". This prover implementation only deals with the successfully
//! generated events where all addresses and values were available.

use binius_m3::builder::{
    upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B32,
};
use zcrayvm_assembly::{opcodes::Opcode, MVVWEvent};

// TODO: Implement tables for other move operations that exist in the assembly implementation:
// - MVV.L (Move Value Long - 128-bit)
// - MVI.H (Move Immediate Half-word)
// - LDI (Load Immediate)
use crate::gadgets::cpu::{CpuColumns, CpuColumnsOptions, CpuGadget, NextPc};
use crate::{channels::Channels, types::ProverPackedField};

/// MVV.W (Move Value to Value) table.
///
/// This table handles the Move Value to Value instruction, which moves a 32-bit
/// value from one VROM location to another.
///
/// Logic:
/// 1. Load the current PC and FP from the state channel
/// 2. Get the instruction from PROM channel
/// 3. Verify this is an MVV.W instruction
/// 4. Get the destination address (FP[dst] + offset)
/// 5. Get the source value from VROM
/// 6. Store the source value at the destination address
/// 7. Update PC to move to the next instruction
pub struct MvvwTable {
    /// Table ID
    pub id: TableId,
    /// CPU columns
    cpu_cols: CpuColumns<{ Opcode::Mvvw as u16 }>,
    dst_abs_addr: Col<B32>,   // Destination address
    src_abs_addr: Col<B32>,   // Source address
    final_dst_addr: Col<B32>, // Destination address with offset
    offset_col: Col<B32>,     // Offset
    src_val: Col<B32>,        // Value to be moved
}

impl MvvwTable {
    /// Create a new MVV.W table with the given constraint system and channels.
    ///
    /// # Arguments
    /// * `cs` - Constraint system to add the table to
    /// * `channels` - Channel IDs for communication with other tables
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("mvvw");

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
            arg2: src,
            ..
        } = cpu_cols;

        // Compute the absolute addresses for destination and source
        let dst_abs_addr = table.add_computed("dst_abs_addr", fp + upcast_expr(dst.into()));
        let src_abs_addr = table.add_computed("src_abs_addr", fp + upcast_expr(src.into()));
        let offset_col = table.add_computed("offset", upcast_expr(offset.into()));

        // Compute final destination address with offset
        // In binary fields, addition (+) is equivalent to XOR (^)
        // This matches the XOR operation used in the witness generation
        // The XOR operation is used for address calculation to combine base address
        // with offset rather than simple addition, which is a specific feature
        // of the zCrayVM memory model
        let final_dst_addr = table.add_computed("final_dst_addr", dst_abs_addr + offset_col);

        // Source value to be moved
        let src_val = table.add_committed("src_val");

        // Pull source value from VROM
        table.pull(channels.vrom_channel, [src_abs_addr, src_val]);

        // Write source value to destination using VROM write channel
        table.pull(channels.vrom_channel, [final_dst_addr, src_val]);

        Self {
            id: table.id(),
            cpu_cols,
            dst_abs_addr,
            src_abs_addr,
            final_dst_addr,
            offset_col,
            src_val,
        }
    }
}

impl TableFiller<ProverPackedField> for MvvwTable {
    type Event = MVVWEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        // This fill method populates the witness with MVV.W events
        // In the assembly implementation, MVV.W events are generated through a complex
        // process that handles cases where source or destination addresses
        // might not be available yet Here we're only concerned with filling in
        // values for events that have been successfully generated (with all
        // required addresses and values available)
        {
            let mut dst_abs_addr = witness.get_scalars_mut(self.dst_abs_addr)?;
            let mut src_abs_addr = witness.get_scalars_mut(self.src_abs_addr)?;
            let mut final_dst_addr = witness.get_scalars_mut(self.final_dst_addr)?;
            let mut offset_col = witness.get_scalars_mut(self.offset_col)?;
            let mut src_val = witness.get_scalars_mut(self.src_val)?;

            for (i, event) in rows.clone().enumerate() {
                dst_abs_addr[i] = B32::new(event.dst_addr);
                src_abs_addr[i] = B32::new(event.fp.addr(event.src));
                offset_col[i] = B32::new(event.offset as u32);
                // XOR operation here matches the + operation in the constraint
                // since in binary fields, addition is equivalent to XOR
                final_dst_addr[i] = B32::new(event.dst_addr ^ event.offset as u32);
                src_val[i] = B32::new(event.src_val);
            }
        }

        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.val(),
            next_pc: None, // NextPc::Increment
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.offset,
            arg2: event.src,
        });

        self.cpu_cols.populate(witness, cpu_rows)
    }
}
