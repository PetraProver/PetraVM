//! TAILI (Tail Call Immediate) table implementation for the zCrayVM M3 circuit.
//!
//! This module contains the TAILI table which handles tail calls to immediate
//! addresses in the zCrayVM execution.

use binius_m3::builder::{
    upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B32,
};
use zcrayvm_assembly::{opcodes::Opcode, TailiEvent};

use crate::gadgets::cpu::{CpuColumns, CpuColumnsOptions, CpuGadget, NextPc};
use crate::{channels::Channels, types::ProverPackedField};

/// TAILI (Tail Call Immediate) table.
///
/// This table handles the Tail Call Immediate instruction, which performs a
/// tail call to a target address given by an immediate.
///
/// Logic:
/// 1. Load the current PC and FP from the state channel
/// 2. Get the instruction from PROM channel
/// 3. Verify this is a TAILI instruction
/// 4. Set up the next frame by preserving return address (FP[0]) and old frame
///    pointer (FP[1])
/// 5. Update PC to the target address
/// 6. Update FP to the next frame pointer
pub struct TailiTable {
    /// Table ID
    pub id: TableId,
    /// CPU columns
    cpu_cols: CpuColumns<{ Opcode::Taili as u16 }>,
    next_fp_abs_addr: Col<B32>, // Virtual
    target: Col<B32>,           // Virtual
}

impl TailiTable {
    /// Create a new TAILI table with the given constraint system and channels.
    ///
    /// # Arguments
    /// * `cs` - Constraint system to add the table to
    /// * `channels` - Channel IDs for communication with other tables
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("taili");

        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Immediate,
                next_fp: None,
            },
        );

        let CpuColumns {
            arg0: _target_low,
            arg1: _target_high,
            arg2: next_fp,
            ..
        } = cpu_cols;

        // Compute the absolute address for the next frame pointer
        let next_fp_abs_addr = table.add_computed("next_fp_abs_addr", upcast_expr(next_fp.into()));

        // Calculate the target address from low and high parts
        // The target address is committed as a single value but is constructed from
        // low 16 bits (arg0) and high 16 bits (arg1) in the witness generation
        let target = table.add_committed("target");

        // Push the target and next_fp_abs_addr to the state channel
        table.push(channels.state_channel, [target, next_fp_abs_addr]);

        Self {
            id: table.id(),
            cpu_cols,
            next_fp_abs_addr,
            target,
        }
    }
}

impl TableFiller<ProverPackedField> for TailiTable {
    type Event = TailiEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            let mut next_fp_abs_addr = witness.get_scalars_mut(self.next_fp_abs_addr)?;
            let mut target = witness.get_scalars_mut(self.target)?;
            for (i, event) in rows.clone().enumerate() {
                // Ensure the next_fp_val is valid before using it
                // Note: Additional validation could be added here if needed
                next_fp_abs_addr[i] = B32::new(event.next_fp_val);
                target[i] = B32::new(event.target);
            }
        }
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.val(),
            next_pc: Some(event.target),
            fp: *event.fp,
            arg0: (event.target & 0xFFFF) as u16,
            arg1: (event.target >> 16) as u16,
            arg2: event.next_fp,
        });
        self.cpu_cols.populate(witness, cpu_rows)
    }
}
