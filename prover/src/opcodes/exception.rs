//! RET (Return) table implementation for the PetraVM M3 circuit.
//!
//! This module contains the RET table which handles return operations
//! in the PetraVM execution.

use binius_field::Field;
use binius_m3::builder::{
    upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B32,
};
use petravm_asm::{opcodes::Opcode, TrapEvent};

use crate::gadgets::state::{NextPc, StateColumns, StateColumnsOptions};
use crate::utils::pull_vrom_channel;
use crate::{
    channels::Channels, gadgets::state::StateGadget, table::Table, types::ProverPackedField,
};

/// RET (Return) table.
///
/// This table handles the Return instruction, which returns from a function
/// call by loading the return PC and FP from the current frame.
///
/// Logic:
/// 1. Load the current PC and FP from the state channel
/// 2. Get the instruction from PROM channel
/// 3. Verify this is a TRAP instruction
/// 4. Verify that the exception frame is correctly set:
///    - First slot: PC when the trap was hit.
///    - Second slot: FP where the trap was hit.
///    - Third slot: Exception code.
/// 5. Update the state with the PC zero and exception FP.
pub struct TrapTable {
    /// Table ID
    id: TableId,
    /// State columns
    state_cols: StateColumns<{ Opcode::Trap as u16 }>,
    exception_fp: Col<B32>,       // Virtual
    exception_fp_xor_1: Col<B32>, // Virtual
    exception_fp_xor_2: Col<B32>, // Virtual
}

impl Table for TrapTable {
    type Event = TrapEvent;

    fn name(&self) -> &'static str {
        "TrapTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("ret");
        let exception_fp = table.add_committed("exception_fp");
        let exception_fp_xor_1 = table.add_computed("exception_fp_xor_1", exception_fp + B32::ONE);
        let exception_fp_xor_2 =
            table.add_computed("exception_fp_xor_2", exception_fp + B32::new(2));
        let zero = table.add_constant("zero", [B32::ZERO]);

        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Target(zero),
                next_fp: Some(exception_fp),
            },
        );

        let fp = state_cols.fp;

        // Write the PC.
        pull_vrom_channel(
            &mut table,
            channels.vrom_channel,
            [exception_fp, state_cols.pc],
        );
        // Write the FP
        pull_vrom_channel(&mut table, channels.vrom_channel, [exception_fp_xor_1, fp]);
        // Write the exception code.
        pull_vrom_channel(
            &mut table,
            channels.vrom_channel,
            [exception_fp_xor_2, upcast_col(state_cols.arg0)],
        );

        Self {
            id: table.id(),
            state_cols,
            exception_fp,
            exception_fp_xor_1,
            exception_fp_xor_2,
        }
    }
}

impl TableFiller<ProverPackedField> for TrapTable {
    type Event = TrapEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut exception_fp = witness.get_scalars_mut(self.exception_fp)?;
            let mut exception_fp_xor_1 = witness.get_scalars_mut(self.exception_fp_xor_1)?;
            let mut exception_fp_xor_2 = witness.get_scalars_mut(self.exception_fp_xor_2)?;
            for (i, event) in rows.clone().enumerate() {
                exception_fp[i] = B32::new(event.exception_fp);
                exception_fp_xor_1[i] = B32::new(event.exception_fp ^ 1);
                exception_fp_xor_2[i] = B32::new(event.exception_fp ^ 2);
            }
        }
        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.into(),
            next_pc: Some(0),
            fp: *event.fp,
            arg0: event.exception_code.val(),
            ..Default::default()
        });
        self.state_cols.populate(witness, state_rows)
    }
}
