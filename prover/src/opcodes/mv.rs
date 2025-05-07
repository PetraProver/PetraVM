//! Move Value tables implementation for the PetraVM M3 circuit.

use std::any::Any;

use binius_field::underlier::Divisible;
use binius_m3::builder::B128;
use binius_m3::builder::{
    upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B32,
};
use petravm_asm::MvihEvent;
use petravm_asm::MvvlEvent;
use petravm_asm::{opcodes::Opcode, MvvwEvent};

use crate::gadgets::multiple_lookup::{MultipleLookupColumns, MultipleLookupGadget};
use crate::gadgets::state::{NextPc, StateColumns, StateColumnsOptions, StateGadget};
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
    /// State-related columns for instruction handling
    state_cols: StateColumns<{ Opcode::Mvvw as u16 }>,
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

        // Set up State columns with standard instruction handling
        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        // Extract instruction arguments from State columns
        let StateColumns {
            fp,
            arg0: dst,
            arg1: offset,
            arg2: src,
            ..
        } = state_cols;

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
            state_cols,
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

        // Create StateGadget rows from events
        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.val(),
            next_pc: None, // NextPc::Increment handled by State columns
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.offset,
            arg2: event.src,
        });

        // Populate State columns with the gadget rows
        self.state_cols.populate(witness, state_rows)
    }
}

/// MVI.H (Move Immediate Half‐word) table implementation.
///
/// VROM[ fp[dst] + offset ] = zero_extend(imm)
pub struct MvihTable {
    pub id: TableId,
    state_cols: StateColumns<{ Opcode::Mvih as u16 }>,
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

        // State columns (pc, fp, args)
        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        let StateColumns {
            fp,
            arg0: dst,
            arg1: offset,
            arg2: imm,
            ..
        } = state_cols;

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
            state_cols,
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

        // Fill State‐side columns (pc, fp, dst, offset, imm)
        let state_rows = rows.map(|ev| StateGadget {
            pc: ev.pc.val(),
            next_pc: None,
            fp: *ev.fp,
            arg0: ev.dst,
            arg1: ev.offset,
            arg2: ev.imm,
        });

        self.state_cols.populate(witness, state_rows)
    }
}

/// MVV.L (Move Value to Value Long) table implementation.
///
/// This table verifies the Move Value to Value (long) instruction, which moves
/// a 128-bit value from one VROM location to another, with optional offset
/// addressing.
pub struct MvvlTable {
    /// Table identifier
    pub id: TableId,
    /// State-related columns for instruction handling
    state_cols: StateColumns<{ Opcode::Mvvl as u16 }>,
    /// Base destination address (FP + dst)
    dst_abs_addr: Col<B32>,
    /// Base source address (FP + src)
    src_abs_addr: Col<B32>,
    /// Final destination address with offset (dst_addr + offset)
    final_dst_addr: Col<B32>,
    /// Destination address value from VROM
    dst_addr: Col<B32>,
    /// Source lookup columns for reading 128-bit value
    src_lookup: MultipleLookupColumns<4>,
    /// Destination lookup columns for writing 128-bit value
    dst_lookup: MultipleLookupColumns<4>,
    /// Source value
    src_val: Col<B32, 4>,
}

impl Table for MvvlTable {
    type Event = MvvlEvent;

    fn name(&self) -> &'static str {
        "MvvlTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("mvvl");

        // Set up State columns with standard instruction handling
        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        // Extract instruction arguments from State columns
        let StateColumns {
            fp,
            arg0: dst,
            arg1: offset,
            arg2: src,
            ..
        } = state_cols;

        // Compute absolute addresses for source and destination
        let dst_abs_addr = table.add_computed("dst_abs_addr", fp + upcast_expr(dst.into()));
        let src_abs_addr = table.add_computed("src_abs_addr", fp + upcast_expr(src.into()));

        // Read the destination address from VROM
        let dst_addr = table.add_committed("dst_addr");
        table.pull(channels.vrom_channel, [dst_abs_addr, dst_addr]);

        // Compute final destination address with offset
        let final_dst_addr =
            table.add_computed("final_dst_addr", dst_addr + upcast_expr(offset.into()));

        // Set up 128-bit source and destination value lookups
        let src_val = table.add_committed("src_val_unpacked");
        let src_lookup = MultipleLookupColumns::new(
            &mut table,
            channels.vrom_channel,
            src_abs_addr,
            src_val,
            "src",
        );

        // Use the same src_val for the destination lookup to enforce equality
        let dst_lookup = MultipleLookupColumns::new(
            &mut table,
            channels.vrom_channel,
            final_dst_addr,
            src_val,
            "dst",
        );

        Self {
            id: table.id(),
            state_cols,
            dst_abs_addr,
            src_abs_addr,
            final_dst_addr,
            dst_addr,
            src_lookup,
            dst_lookup,
            src_val,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TableFiller<ProverPackedField> for MvvlTable {
    type Event = MvvlEvent;

    fn id(&self) -> TableId {
        self.id
    }

    /// Fill the table witness with data from MVV.L events
    ///
    /// This populates the witness data based on the execution events from
    /// the corresponding assembly MVV.L operations.
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
            let mut src_val = witness.get_mut_as(self.src_val)?;

            // Fill the witness columns with values from each event
            for (i, event) in rows.clone().enumerate() {
                dst_abs_addr[i] = B32::new(event.fp.addr(event.dst));
                src_abs_addr[i] = B32::new(event.fp.addr(event.src));
                dst_addr[i] = B32::new(event.dst_addr);
                final_dst_addr[i] = B32::new(event.dst_addr ^ event.offset as u32);
                src_val[i] = B128::new(event.src_val);
            }
        }

        // Generate MultipleVromLookupGadget rows for source and destination
        let src_rows = rows.clone().map(|event| {
            let vals: [u32; 4] = <u128 as Divisible<u32>>::split_val(event.src_val);
            MultipleLookupGadget {
                addr: event.fp.addr(event.src),
                vals,
            }
        });
        self.src_lookup.populate(witness, src_rows)?;

        // Generate MultipleVromLookupGadget rows for destination
        let dst_rows = rows.clone().map(|event| {
            let vals: [u32; 4] = <u128 as Divisible<u32>>::split_val(event.src_val);
            MultipleLookupGadget {
                addr: event.dst_addr ^ event.offset as u32,
                vals,
            }
        });
        self.dst_lookup.populate(witness, dst_rows)?;

        // Create StateGadget rows from events
        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.val(),
            next_pc: None, // NextPc::Increment handled by State columns
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.offset,
            arg2: event.src,
        });

        // Populate State columns with the gadget rows
        self.state_cols.populate(witness, state_rows)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use binius_field::PackedField;
    use petravm_asm::isa::GenericISA;

    use crate::model::Trace;
    use crate::prover::Prover;
    use crate::table::G;
    use crate::test_utils::generate_trace;

    /// Creates an execution trace for a simple program that uses the MVV.L
    /// instruction to test 128-bit value movement.
    fn generate_mvvl_trace() -> Result<Trace> {
        // Create an assembly program that tests both known and unknown src values
        let asm_code = r#"
        #[framesize(0x10)]
        _start:
            ;; Test case 1: Known source value
            LDI.W @4, #1234       ;; Set low 32 bits of source
            LDI.W @5, #5678       ;; Set next 32 bits
            LDI.W @6, #9012       ;; Set next 32 bits
            LDI.W @7, #3456       ;; Set high 32 bits
            MVV.L @12[4], @4      ;; Move 128-bit value to final destination
            
            ;; Test case 2: unknown source value
            MVV.L @12[8], @8      ;; Move 128-bit value from final destination to source
            CALLI compute_value, @12
            RET
            
        #[framesize(0x10)]
        compute_value:
            LDI.W @8, #0123       ;; Set low 32 bits of source
            LDI.W @9, #4567       ;; Set next 32 bits
            LDI.W @10, #8901      ;; Set next 32 bits
            LDI.W @11, #2345      ;; Set high 32 bits
            RET
        "#
        .to_string();

        let calli_return_pc = G.pow(7).val();
        let vrom_writes = vec![
            // 2 MVV.L + 1 CALLI (next_fp)
            (12, 16, 3),
            // LDI.W + MVV.L
            (4, 1234, 2),
            (5, 5678, 2),
            (6, 9012, 2),
            (7, 3456, 2),
            // CALLI (return PC)
            (16, calli_return_pc, 2),
            // CALLI (return FP)
            (17, 0, 2),
            // MVV.L with unknown source value
            (24, 123, 2),
            (25, 4567, 2),
            (26, 8901, 2),
            (27, 2345, 2),
            // Initial values
            (0, 0, 1),
            (1, 0, 1),
            // MVV.L with unknown source value
            (8, 123, 1),
            (9, 4567, 1),
            (10, 8901, 1),
            (11, 2345, 1),
            // MVV.L with known source value
            (20, 1234, 1),
            (21, 5678, 1),
            (22, 9012, 1),
            (23, 3456, 1),
        ];
        generate_trace(asm_code, None, Some(vrom_writes))
    }

    #[test]
    fn test_mvvl() -> Result<()> {
        let trace = generate_mvvl_trace()?;
        trace.validate()?;
        assert_eq!(trace.trace.mvvl.len(), 2);
        assert_eq!(trace.trace.ret.len(), 2);
        assert_eq!(trace.trace.calli.len(), 1);
        Prover::new(Box::new(GenericISA)).validate_witness(&trace)
    }

    /// Creates an execution trace for a simple program that uses the MVV.L
    /// instruction to test 128-bit value movement and operations.
    fn generate_b128_mvvl_add_mul_trace() -> Result<Trace> {
        let asm_code = r#"
        #[framesize(0x10)]
        _start:
            ;; Test case: unknown source value
            MVV.L @12[8], @4      ;; Move 128-bit value from final destination to source
            MVV.L @12[4], @8      ;; Move 128-bit value from final destination to source
            CALLI compute_value, @12
            RET
        #[framesize(0x10)]
        compute_value:
            LDI.W @2, #1234
            LDI.W @3, #5678
            B128_MUL @4, @0, @0    ;; Multiply
            B128_ADD @8, @0, @4    ;; Add
            RET
        "#
        .to_string();

        generate_trace(asm_code, None, None)
    }

    #[test]
    fn test_b128_mvvl_add_mul() -> Result<()> {
        let trace = generate_b128_mvvl_add_mul_trace()?;
        trace.validate()?;
        assert_eq!(trace.trace.mvvl.len(), 2);
        assert_eq!(trace.trace.ldi.len(), 2);
        assert_eq!(trace.trace.b128_mul.len(), 1);
        assert_eq!(trace.trace.b128_add.len(), 1);
        assert_eq!(trace.trace.ret.len(), 2);
        assert_eq!(trace.trace.calli.len(), 1);
        Prover::new(Box::new(GenericISA)).validate_witness(&trace)
    }
}
