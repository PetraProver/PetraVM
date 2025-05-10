//! Function call instructions for the PetraVM M3 circuit.

use binius_m3::builder::{
    upcast_col, upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B32,
};
use petravm_asm::{opcodes::Opcode, CalliEvent, CallvEvent, TailiEvent, TailvEvent};

use crate::gadgets::state::{NextPc, StateColumns, StateColumnsOptions, StateGadget};
use crate::table::Table;
use crate::{channels::Channels, opcodes::G, types::ProverPackedField};

/// TAILI (Tail Call Immediate) table implementation.
pub struct TailiTable {
    /// Table identifier
    pub id: TableId,
    /// State-related columns for instruction handling
    state_cols: StateColumns<{ Opcode::Taili as u16 }>,
    /// New frame pointer value
    next_fp_val: Col<B32>,
    /// Absolute address of the next frame pointer slot (FP + next_fp)
    next_fp_abs_addr: Col<B32>,
    /// Return address from caller
    return_addr: Col<B32>,
    /// Old frame pointer value
    old_fp_val: Col<B32>,
    /// Address of current frame slot 1 (FP + 1)
    fp_plus_1: Col<B32>,
    /// Address of new frame slot 1 (next_fp_val + 1)
    next_fp_plus_1: Col<B32>,
}

impl Table for TailiTable {
    type Event = TailiEvent;

    fn name(&self) -> &'static str {
        "TailiTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("taili");

        // Column for the new frame pointer value
        let next_fp_val = table.add_committed("next_fp_val");

        // Set up State columns with immediate PC update and new frame pointer
        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Immediate, // Jump directly to target address
                next_fp: Some(next_fp_val), // Update frame pointer
            },
        );

        // Extract relevant instruction arguments
        let StateColumns {
            fp: cur_fp,
            arg2: next_fp,
            ..
        } = state_cols;

        // Compute the absolute address for the next frame pointer
        let next_fp_abs_addr =
            table.add_computed("next_fp_abs_addr", cur_fp + upcast_expr(next_fp.into()));

        // Read the next frame pointer value from VROM
        table.pull(channels.vrom_channel, [next_fp_abs_addr, next_fp_val]);

        // Read current frame's return address and old frame pointer
        let return_addr = table.add_committed("return_addr"); // Return address at slot 0
        let fp_plus_1 = table.add_computed("fp_plus_1", cur_fp + B32::new(1)); // Address of slot 1
        let old_fp_val = table.add_committed("old_fp_val"); // Old frame pointer at slot 1

        // Pull values from current frame
        table.pull(channels.vrom_channel, [cur_fp, return_addr]);
        table.pull(channels.vrom_channel, [fp_plus_1, old_fp_val]);

        // Compute address of slot 1 in new frame
        let next_fp_plus_1 = table.add_computed("next_fp_plus_1", next_fp_val + B32::new(1));

        // Verify that return address and old frame pointer are correctly copied to new
        // frame
        table.pull(channels.vrom_channel, [next_fp_val, return_addr]);
        table.pull(channels.vrom_channel, [next_fp_plus_1, old_fp_val]);

        Self {
            id: table.id(),
            state_cols,
            next_fp_val,
            next_fp_abs_addr,
            return_addr,
            old_fp_val,
            fp_plus_1,
            next_fp_plus_1,
        }
    }
}

impl TableFiller<ProverPackedField> for TailiTable {
    type Event = TailiEvent;

    fn id(&self) -> TableId {
        self.id
    }

    /// Fill the table witness with data from TAILI events
    ///
    /// This populates the witness data based on the execution events from
    /// the corresponding assembly TAILI operations.
    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            // Get mutable references to witness columns
            let mut next_fp_val = witness.get_scalars_mut(self.next_fp_val)?;
            let mut next_fp_abs_addr = witness.get_scalars_mut(self.next_fp_abs_addr)?;
            let mut return_addr = witness.get_scalars_mut(self.return_addr)?;
            let mut old_fp_val = witness.get_scalars_mut(self.old_fp_val)?;
            let mut fp_plus_1 = witness.get_scalars_mut(self.fp_plus_1)?;
            let mut next_fp_plus_1 = witness.get_scalars_mut(self.next_fp_plus_1)?;

            // Fill the witness columns with values from each event
            for (i, event) in rows.clone().enumerate() {
                next_fp_val[i] = B32::new(event.next_fp_val);
                next_fp_abs_addr[i] = B32::new(event.fp.addr(event.next_fp));
                return_addr[i] = B32::new(event.return_addr);
                fp_plus_1[i] = B32::new(event.fp.addr(1u32));
                old_fp_val[i] = B32::new(event.old_fp_val as u32);
                next_fp_plus_1[i] = B32::new(event.next_fp_val + 1);
            }
        }

        // Create StateGadget rows from events
        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.val(),
            next_pc: Some(event.target), // Jump to target address
            fp: *event.fp,
            arg0: event.target as u16,         // target_low (lower 16 bits)
            arg1: (event.target >> 16) as u16, // target_high (upper 16 bits)
            arg2: event.next_fp,               // next_fp address
        });

        // Populate State columns with the gadget rows
        self.state_cols.populate(witness, state_rows)
    }
}

/// TAILV (Tail Call Variable) table implementation.
pub struct TailvTable {
    /// Table identifier
    pub id: TableId,
    /// State-related columns for instruction handling
    state_cols: StateColumns<{ Opcode::Tailv as u16 }>,
    /// New frame pointer value
    next_fp_val: Col<B32>,
    /// Absolute address of the next frame pointer slot (FP + next_fp)
    next_fp_abs_addr: Col<B32>,
    /// Address of the offset slot (FP + offset)
    offset_addr: Col<B32>,
    /// Target address value (read from VROM)
    target_val: Col<B32>,
    /// Return address from caller
    return_addr: Col<B32>,
    /// Old frame pointer value
    old_fp_val: Col<B32>,
    /// Address of current frame slot 1 (FP + 1)
    fp_plus_1: Col<B32>,
    /// Address of new frame slot 1 (next_fp_val + 1)
    next_fp_plus_1: Col<B32>,
}

impl Table for TailvTable {
    type Event = TailvEvent;

    fn name(&self) -> &'static str {
        "TailvTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("tailv");

        // Columns for committed values
        let target_val = table.add_committed("target_val");
        let next_fp_val = table.add_committed("next_fp_val");
        let return_addr = table.add_committed("return_addr");
        let old_fp_val = table.add_committed("old_fp_val");

        // Set up State columns with target-based PC update and new frame pointer
        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Target(target_val), // Jump to target address from VROM
                next_fp: Some(next_fp_val),          // Update frame pointer
            },
        );

        // Extract relevant instruction arguments
        let StateColumns {
            fp: cur_fp,
            arg0: offset,
            arg1: next_fp,
            ..
        } = state_cols;

        // Compute the absolute addresses
        let offset_addr = table.add_computed("offset_addr", cur_fp + upcast_col(offset));
        let next_fp_abs_addr =
            table.add_computed("next_fp_abs_addr", cur_fp + upcast_expr(next_fp.into()));
        let fp_plus_1 = table.add_computed("fp_plus_1", cur_fp + B32::new(1));
        let next_fp_plus_1 = table.add_computed("next_fp_plus_1", next_fp_val + B32::new(1));

        // Read values from VROM
        table.pull(channels.vrom_channel, [offset_addr, target_val]);
        table.pull(channels.vrom_channel, [next_fp_abs_addr, next_fp_val]);
        table.pull(channels.vrom_channel, [cur_fp, return_addr]);
        table.pull(channels.vrom_channel, [fp_plus_1, old_fp_val]);

        // Verify that return address and old frame pointer are correctly copied to new
        // frame
        table.pull(channels.vrom_channel, [next_fp_val, return_addr]);
        table.pull(channels.vrom_channel, [next_fp_plus_1, old_fp_val]);

        Self {
            id: table.id(),
            state_cols,
            next_fp_val,
            next_fp_abs_addr,
            offset_addr,
            target_val,
            return_addr,
            old_fp_val,
            fp_plus_1,
            next_fp_plus_1,
        }
    }
}

impl TableFiller<ProverPackedField> for TailvTable {
    type Event = TailvEvent;

    fn id(&self) -> TableId {
        self.id
    }

    /// Fill the table witness with data from TAILV events
    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            // Get mutable references to witness columns
            let mut next_fp_val = witness.get_scalars_mut(self.next_fp_val)?;
            let mut next_fp_abs_addr = witness.get_scalars_mut(self.next_fp_abs_addr)?;
            let mut offset_addr = witness.get_scalars_mut(self.offset_addr)?;
            let mut target_val = witness.get_scalars_mut(self.target_val)?;
            let mut return_addr = witness.get_scalars_mut(self.return_addr)?;
            let mut old_fp_val = witness.get_scalars_mut(self.old_fp_val)?;
            let mut fp_plus_1 = witness.get_scalars_mut(self.fp_plus_1)?;
            let mut next_fp_plus_1 = witness.get_scalars_mut(self.next_fp_plus_1)?;

            // Fill the witness columns with values from each event
            for (i, event) in rows.clone().enumerate() {
                next_fp_val[i] = B32::new(event.next_fp_val);
                next_fp_abs_addr[i] = B32::new(event.fp.addr(event.next_fp));
                offset_addr[i] = B32::new(event.fp.addr(event.offset));
                target_val[i] = B32::new(event.target);
                return_addr[i] = B32::new(event.return_addr);
                old_fp_val[i] = B32::new(event.old_fp_val as u32);
                fp_plus_1[i] = B32::new(event.fp.addr(1u32));
                next_fp_plus_1[i] = B32::new(event.next_fp_val + 1);
            }
        }

        // Create StateGadget rows from events
        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.val(),
            next_pc: Some(event.target), // Jump to target address
            fp: *event.fp,
            arg0: event.offset,  // offset for reading target
            arg1: event.next_fp, // next_fp address
            arg2: 0,             // unused
        });

        // Populate State columns with the gadget rows
        self.state_cols.populate(witness, state_rows)
    }
}

/// CALLI (Call Immediate) table implementation.
pub struct CalliTable {
    /// Table identifier
    pub id: TableId,
    /// State-related columns for instruction handling
    state_cols: StateColumns<{ Opcode::Calli as u16 }>,
    /// New frame pointer value
    next_fp_val: Col<B32>,
    /// Absolute address of the next frame pointer slot (FP + next_fp)
    next_fp_abs_addr: Col<B32>,
    /// Next PC value to be saved as return address (PC * G)
    next_pc_val: Col<B32>,
    /// Address of new frame slot 1 (old FP location)
    next_fp_slot_1: Col<B32>,
}

impl Table for CalliTable {
    type Event = CalliEvent;

    fn name(&self) -> &'static str {
        "CalliTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("calli");

        // Column for the new frame pointer value
        let next_fp_val = table.add_committed("next_fp_val");

        // Set up State columns with immediate PC update and new frame pointer
        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Immediate, // Jump directly to target address
                next_fp: Some(next_fp_val), // Update frame pointer
            },
        );

        // Extract relevant instruction arguments
        let StateColumns {
            pc,
            fp: cur_fp,
            arg2: next_fp,
            ..
        } = state_cols;

        // Next PC value (PC * G) to be saved as return address
        // We use the G constant (multiplicative generator) to get next PC
        let next_pc_val = table.add_computed("next_pc_val", pc * G);

        // Compute the absolute address for the next frame pointer
        let next_fp_abs_addr =
            table.add_computed("next_fp_abs_addr", cur_fp + upcast_expr(next_fp.into()));

        // Read the next frame pointer value from VROM
        table.pull(channels.vrom_channel, [next_fp_abs_addr, next_fp_val]);

        // Calculate addresses for the new frame's slots
        let next_fp_slot_1 = table.add_computed("next_fp_slot_1", next_fp_val + B32::new(1));

        // Verify return address (next_pc_val) is stored at slot 0 of new frame
        table.pull(channels.vrom_channel, [next_fp_val, next_pc_val]);

        // Verify current frame pointer is stored at slot 1 of new frame
        table.pull(channels.vrom_channel, [next_fp_slot_1, cur_fp]);

        Self {
            id: table.id(),
            state_cols,
            next_fp_val,
            next_fp_abs_addr,
            next_pc_val,
            next_fp_slot_1,
        }
    }
}

impl TableFiller<ProverPackedField> for CalliTable {
    type Event = CalliEvent;

    fn id(&self) -> TableId {
        self.id
    }

    /// Fill the table witness with data from CALLI events
    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            // Get mutable references to witness columns
            let mut next_fp_val = witness.get_scalars_mut(self.next_fp_val)?;
            let mut next_fp_abs_addr = witness.get_scalars_mut(self.next_fp_abs_addr)?;
            let mut next_pc_val = witness.get_scalars_mut(self.next_pc_val)?;
            let mut next_fp_slot_1 = witness.get_scalars_mut(self.next_fp_slot_1)?;

            // Fill the witness columns with values from each event
            for (i, event) in rows.clone().enumerate() {
                next_fp_val[i] = B32::new(event.next_fp_val);
                next_fp_abs_addr[i] = B32::new(event.fp.addr(event.next_fp));
                // Calculate next PC as return address
                next_pc_val[i] = event.pc * G;
                next_fp_slot_1[i] = B32::new(event.next_fp_val + 1);
            }
        }

        // Create StateGadget rows from events
        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.val(),
            next_pc: Some(event.target), // Jump to target address
            fp: *event.fp,
            arg0: event.target as u16,         // target_low (lower 16 bits)
            arg1: (event.target >> 16) as u16, // target_high (upper 16 bits)
            arg2: event.next_fp,               // next_fp address
        });

        // Populate State columns with the gadget rows
        self.state_cols.populate(witness, state_rows)
    }
}

/// CALLV (Call Variable) table implementation.
pub struct CallvTable {
    /// Table identifier
    pub id: TableId,
    /// State-related columns for instruction handling
    state_cols: StateColumns<{ Opcode::Callv as u16 }>,
    /// New frame pointer value
    next_fp_val: Col<B32>,
    /// Absolute address of the next frame pointer slot (FP + next_fp)
    next_fp_abs_addr: Col<B32>,
    /// Address of the offset slot (FP + offset)
    offset_abs_addr: Col<B32>,
    /// Target address value (read from VROM)
    target_val: Col<B32>,
    /// Next PC value to be saved as return address (PC * G)
    next_pc_val: Col<B32>,
    /// Address of new frame slot 1 (old FP location)
    next_fp_slot_1: Col<B32>,
}

impl Table for CallvTable {
    type Event = CallvEvent;

    fn name(&self) -> &'static str {
        "CallvTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("callv");

        // Columns for committed values
        let target_val = table.add_committed("target_val");
        let next_fp_val = table.add_committed("next_fp_val");

        // Set up State columns with target-based PC update and new frame pointer
        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Target(target_val), // Jump to target address from VROM
                next_fp: Some(next_fp_val),          // Update frame pointer
            },
        );

        // Extract relevant instruction arguments
        let StateColumns {
            pc,
            fp: cur_fp,
            arg0: offset,
            arg1: next_fp,
            ..
        } = state_cols;

        // Next PC value (PC * G) to be saved as return address
        let next_pc_val = table.add_computed("next_pc_val", pc * G);

        // Compute the absolute addresses
        let offset_abs_addr = table.add_computed("offset_abs_addr", cur_fp + upcast_col(offset));
        let next_fp_abs_addr =
            table.add_computed("next_fp_abs_addr", cur_fp + upcast_expr(next_fp.into()));

        // Read values from VROM
        table.pull(channels.vrom_channel, [offset_abs_addr, target_val]);
        table.pull(channels.vrom_channel, [next_fp_abs_addr, next_fp_val]);

        // Calculate addresses for the new frame's slots
        let next_fp_slot_1 = table.add_computed("next_fp_slot_1", next_fp_val + B32::new(1));

        // Verify return address (next_pc_val) is stored at slot 0 of new frame
        table.pull(channels.vrom_channel, [next_fp_val, next_pc_val]);

        // Verify current frame pointer is stored at slot 1 of new frame
        table.pull(channels.vrom_channel, [next_fp_slot_1, cur_fp]);

        Self {
            id: table.id(),
            state_cols,
            next_fp_val,
            next_fp_abs_addr,
            offset_abs_addr,
            target_val,
            next_pc_val,
            next_fp_slot_1,
        }
    }
}

impl TableFiller<ProverPackedField> for CallvTable {
    type Event = CallvEvent;

    fn id(&self) -> TableId {
        self.id
    }

    /// Fill the table witness with data from CALLV events
    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            // Get mutable references to witness columns
            let mut next_fp_val = witness.get_scalars_mut(self.next_fp_val)?;
            let mut next_fp_abs_addr = witness.get_scalars_mut(self.next_fp_abs_addr)?;
            let mut offset_abs_addr = witness.get_scalars_mut(self.offset_abs_addr)?;
            let mut target_val = witness.get_scalars_mut(self.target_val)?;
            let mut next_pc_val = witness.get_scalars_mut(self.next_pc_val)?;
            let mut next_fp_slot_1 = witness.get_scalars_mut(self.next_fp_slot_1)?;

            // Fill the witness columns with values from each event
            for (i, event) in rows.clone().enumerate() {
                next_fp_val[i] = B32::new(event.next_fp_val);
                next_fp_abs_addr[i] = B32::new(event.fp.addr(event.next_fp));
                offset_abs_addr[i] = B32::new(event.fp.addr(event.offset));
                target_val[i] = B32::new(event.target);
                next_pc_val[i] = event.pc * G;
                next_fp_slot_1[i] = B32::new(event.next_fp_val + 1);
            }
        }

        // Create StateGadget rows from events
        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.val(),
            next_pc: Some(event.target), // Jump to target address
            fp: *event.fp,
            arg0: event.offset,  // offset for reading target
            arg1: event.next_fp, // next_fp address
            arg2: 0,             // unused
        });

        // Populate State columns with the gadget rows
        self.state_cols.populate(witness, state_rows)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use binius_field::{BinaryField, PackedField};
    use petravm_asm::isa::GenericISA;

    use super::*;
    use crate::model::Trace;
    use crate::prover::Prover;
    use crate::test_utils::generate_trace;

    pub(crate) const G: B32 = B32::MULTIPLICATIVE_GENERATOR;

    /// Creates an execution trace for a simple program that uses the TAILI and
    /// TAILV instructions to test function call operations.
    fn generate_taili_tailv_trace() -> Result<Trace> {
        let pc_val = G.pow(4).val();
        // Create an assembly program that tests function call variants
        let asm_code = format!(
            "#[framesize(0x10)]\n\
            _start:\n\
                LDI.W @3, #{pc_val}\n\
                MVV.W @4[2], @2\n\
                MVI.H @4[3], #2\n\
                TAILV @3, @4\n\
            #[framesize(0x10)]\n\
            loop:\n\
                BNZ case_recurse, @3\n\
                LDI.W @2, #100\n\
                RET\n\
            case_recurse:\n\
                LDI.W @4, #0\n\
                MVV.W @5[2], @2\n\
                MVV.W @5[3], @4\n\
                TAILI loop, @5\n"
        );

        generate_trace(asm_code, None, None)
    }

    /// Creates an execution trace for a simple program that uses the CALLI and
    /// CALLV instructions to test function call operations.
    fn generate_calli_callv_trace() -> Result<Trace> {
        let pc_val = G.pow(5).val();
        // Create an assembly program that tests function call variants
        let asm_code = format!(
            "#[framesize(0x10)]\n\
            _start:\n\
                LDI.W @3, #{pc_val}\n\
                MVV.W @4[2], @2\n\
                MVI.H @4[3], #2\n\
                CALLV @3, @4\n\
                RET\n\
            #[framesize(0x10)]\n\
            loop:\n\
                BNZ case_recurse, @3\n\
                LDI.W @2, #100\n\
                RET\n\
            case_recurse:\n\
                LDI.W @4, #0\n\
                MVV.W @5[2], @2\n\
                MVV.W @5[3], @4\n\
                CALLI loop, @5\n\
                RET\n"
        );

        generate_trace(asm_code, None, None)
    }

    #[test]
    fn test_taili_tailv() -> Result<()> {
        let trace = generate_taili_tailv_trace()?;
        trace.validate()?;
        assert_eq!(trace.taili_events().len(), 1);
        assert_eq!(trace.tailv_events().len(), 1);
        Prover::new(Box::new(GenericISA)).validate_witness(&trace)
    }

    #[test]
    fn test_calli_callv() -> Result<()> {
        let trace = generate_calli_callv_trace()?;
        trace.validate()?;
        assert_eq!(trace.calli_events().len(), 1);
        assert_eq!(trace.callv_events().len(), 1);
        assert_eq!(trace.ret_events().len(), 3);
        Prover::new(Box::new(GenericISA)).validate_witness(&trace)
    }
}
