use binius_core::oracle::ShiftVariant;
use binius_m3::{
    builder::{
        upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1, B16, B32,
    },
    gadgets::barrel_shifter::BarrelShifter,
};
use zcrayvm_assembly::{Opcode, SllEvent, SlliEvent, SrlEvent, SrliEvent};

use crate::{
    channels::Channels,
    gadgets::cpu::{CpuColumns, CpuColumnsOptions, CpuGadget},
    table::Table,
    types::ProverPackedField,
};

/// Table for the SRLI (Shift Right Logical Immediate) instruction. It
/// constraints the values src_val  to be equal to dst_val << shift_amount. The
/// shift amount is given as an immediate. In addition to the standard CPU
/// columns and src, dst columns, it also includes columns for performing a
/// Barrel shifter circuit.
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
            ShiftVariant::LogicalRight,
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
                dst_abs[i] = event.fp.addr(event.dst);
                src_abs[i] = event.fp.addr(event.src);
            }
        }
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.val(),
            next_pc: None,
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.src,
            arg2: event.shift_amount as u16,
        });
        self.cpu_cols.populate(witness, cpu_rows)?;
        self.shifter.populate(witness)?;
        Ok(())
    }
}

/// Table for the SLLI (Shift Left Logical Immediate) instruction. It
/// constraints the values src_val  to be equal to dst_val << shift_amount. The
/// shift amount is given as an immediate. In addition to the standard CPU
/// columns and src, dst columns, it also includes columns for performing a
/// Barrel shifter circuit.
pub struct SlliTable {
    id: TableId,
    cpu_cols: CpuColumns<{ Opcode::Slli as u16 }>,
    shifter: BarrelShifter,
    dst_abs: Col<B32>, // Virtual
    dst_val: Col<B32>, // Virtual
    src_abs: Col<B32>, // Virtual
    src_val: Col<B32>, // Virtual
}

impl Table for SlliTable {
    type Event = SlliEvent;

    fn name(&self) -> &'static str {
        "SlliTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("slli");
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
            ShiftVariant::LogicalLeft,
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

impl TableFiller<ProverPackedField> for SlliTable {
    type Event = SlliEvent;

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
                dst_abs[i] = event.fp.addr(event.dst);
                src_abs[i] = event.fp.addr(event.src);
            }
        }
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.val(),
            next_pc: None,
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.src,
            arg2: event.shift_amount as u16,
        });
        self.cpu_cols.populate(witness, cpu_rows)?;
        self.shifter.populate(witness)?;
        Ok(())
    }
}

/// Table for the SRL (Shift Right Logical) instruction. It
/// constraints the values src_val  to be equal to dst_val >> shift_amount. The
/// shift amount is given as an immediate. In addition to the standard CPU
/// columns and src, dst columns, it also includes columns for performing a
/// Barrel shifter circuit.
pub struct SrlTable {
    id: TableId,
    cpu_cols: CpuColumns<{ Opcode::Srl as u16 }>,
    shifter: BarrelShifter,
    dst_abs: Col<B32>,                  // Virtual
    dst_val: Col<B32>,                  // Virtual
    src_abs: Col<B32>,                  // Virtual
    src_val: Col<B32>,                  // Virtual
    src_val_unpacked: Col<B1, 32>,      // Committed
    shift_abs: Col<B32>,                // Virtual
    shift_val: Col<B32>,                // Virtual
    shift_amount_unpacked: Col<B1, 16>, // Committed
}

impl Table for SrlTable {
    type Event = SrlEvent;

    fn name(&self) -> &'static str {
        "SrlTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("srl");

        // Set up CPU columns
        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions::default(),
        );

        // Create columns for source, destination, and shift
        let src_val_unpacked: Col<B1, 32> = table.add_committed("src_val_unpacked");
        let src_val: Col<B32> = table.add_packed("src_val", src_val_unpacked);
        let dst_abs = table.add_computed("dst_abs", cpu_cols.fp + upcast_col(cpu_cols.arg0));
        let src_abs = table.add_computed("src_abs", cpu_cols.fp + upcast_col(cpu_cols.arg1));
        let shift_abs = table.add_computed("shift_abs", cpu_cols.fp + upcast_col(cpu_cols.arg2));

        // Create shift amount columns
        let shift_amount_unpacked: Col<B1, 16> = table.add_committed("shift_amount_unpacked");
        let shift_amount_packed: Col<B16, 1> =
            table.add_packed("shift_amount", shift_amount_unpacked);
        let shift_val = upcast_col(shift_amount_packed);

        // Create the barrel shifter
        let shifter = BarrelShifter::new(
            &mut table,
            src_val_unpacked,
            shift_amount_unpacked,
            ShiftVariant::LogicalRight,
        );

        // Create result column
        let dst_val = table.add_packed("dst_val", shifter.output);

        // Set up channel pulls
        table.pull(channels.vrom_channel, [dst_abs, dst_val]);
        table.pull(channels.vrom_channel, [src_abs, src_val]);
        table.pull(channels.vrom_channel, [shift_abs, shift_val]);

        Self {
            id: table.id(),
            cpu_cols,
            shifter,
            dst_abs,
            dst_val,
            src_abs,
            src_val,
            src_val_unpacked,
            shift_abs,
            shift_val,
            shift_amount_unpacked,
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl TableFiller<ProverPackedField> for SrlTable {
    type Event = SrlEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        // Fill in basic data and shifter data separately to avoid borrowing conflicts
        {
            let mut dst_abs = witness.get_mut_as(self.dst_abs)?;
            let mut src_abs = witness.get_mut_as(self.src_abs)?;
            let mut src_val_unpacked = witness.get_mut_as(self.src_val_unpacked)?;
            let mut shift_abs = witness.get_mut_as(self.shift_abs)?;
            let mut shift_amount_unpacked = witness.get_mut_as(self.shift_amount_unpacked)?;

            for (i, event) in rows.clone().enumerate() {
                src_val_unpacked[i] = event.src_val;
                dst_abs[i] = event.fp.addr(event.dst);
                src_abs[i] = event.fp.addr(event.src);
                shift_abs[i] = event.fp.addr(event.shift);
                shift_amount_unpacked[i] = event.shift_amount as u16;
            }
        }

        // Fill CPU data
        {
            let cpu_rows = rows.clone().map(|event| CpuGadget {
                pc: event.pc.val(),
                next_pc: None,
                fp: *event.fp,
                arg0: event.dst,
                arg1: event.src,
                arg2: event.shift,
            });
            self.cpu_cols.populate(witness, cpu_rows)?;
        }

        // Let the barrel shifter populate its own data
        self.shifter.populate(witness)?;

        Ok(())
    }
}

/// Table for the SLL (Shift Left Logical) instruction. It
/// constraints the values src_val  to be equal to dst_val << shift_amount. The
/// shift amount is given as an immediate. In addition to the standard CPU
/// columns and src, dst columns, it also includes columns for performing a
/// Barrel shifter circuit.
pub struct SllTable {
    id: TableId,
    cpu_cols: CpuColumns<{ Opcode::Sll as u16 }>,
    shifter: BarrelShifter,
    dst_abs: Col<B32>,                  // Virtual
    dst_val: Col<B32>,                  // Virtual
    src_abs: Col<B32>,                  // Virtual
    src_val: Col<B32>,                  // Virtual
    src_val_unpacked: Col<B1, 32>,      // Committed
    shift_abs: Col<B32>,                // Virtual
    shift_val: Col<B32>,                // Virtual
    shift_amount_unpacked: Col<B1, 16>, // Committed
}

impl Table for SllTable {
    type Event = SllEvent;

    fn name(&self) -> &'static str {
        "SllTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("sll");

        // Set up CPU columns
        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions::default(),
        );

        // Create columns for source, destination, and shift
        let src_val_unpacked: Col<B1, 32> = table.add_committed("src_val_unpacked");
        let src_val: Col<B32> = table.add_packed("src_val", src_val_unpacked);
        let dst_abs = table.add_computed("dst_abs", cpu_cols.fp + upcast_col(cpu_cols.arg0));
        let src_abs = table.add_computed("src_abs", cpu_cols.fp + upcast_col(cpu_cols.arg1));
        let shift_abs = table.add_computed("shift_abs", cpu_cols.fp + upcast_col(cpu_cols.arg2));

        // Create shift amount columns
        let shift_amount_unpacked: Col<B1, 16> = table.add_committed("shift_amount_unpacked");
        let shift_amount_packed: Col<B16, 1> =
            table.add_packed("shift_amount", shift_amount_unpacked);
        let shift_val = upcast_col(shift_amount_packed);

        // Create the barrel shifter
        let shifter = BarrelShifter::new(
            &mut table,
            src_val_unpacked,
            shift_amount_unpacked,
            ShiftVariant::LogicalLeft,
        );

        // Create result column
        let dst_val = table.add_packed("dst_val", shifter.output);

        // Set up channel pulls
        table.pull(channels.vrom_channel, [dst_abs, dst_val]);
        table.pull(channels.vrom_channel, [src_abs, src_val]);
        table.pull(channels.vrom_channel, [shift_abs, shift_val]);

        Self {
            id: table.id(),
            cpu_cols,
            shifter,
            dst_abs,
            dst_val,
            src_abs,
            src_val,
            src_val_unpacked,
            shift_abs,
            shift_val,
            shift_amount_unpacked,
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl TableFiller<ProverPackedField> for SllTable {
    type Event = SllEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        // Fill in basic data and shifter data separately to avoid borrowing conflicts
        {
            let mut dst_abs = witness.get_mut_as(self.dst_abs)?;
            let mut src_abs = witness.get_mut_as(self.src_abs)?;
            let mut src_val_unpacked = witness.get_mut_as(self.src_val_unpacked)?;
            let mut shift_abs = witness.get_mut_as(self.shift_abs)?;
            let mut shift_amount_unpacked = witness.get_mut_as(self.shift_amount_unpacked)?;

            for (i, event) in rows.clone().enumerate() {
                src_val_unpacked[i] = event.src_val;
                dst_abs[i] = event.fp.addr(event.dst);
                src_abs[i] = event.fp.addr(event.src);
                shift_abs[i] = event.fp.addr(event.shift);
                shift_amount_unpacked[i] = event.shift_amount as u16;
            }
        }

        // Fill CPU data
        {
            let cpu_rows = rows.clone().map(|event| CpuGadget {
                pc: event.pc.val(),
                next_pc: None,
                fp: *event.fp,
                arg0: event.dst,
                arg1: event.src,
                arg2: event.shift,
            });
            self.cpu_cols.populate(witness, cpu_rows)?;
        }

        // Let the barrel shifter populate its own data
        self.shifter.populate(witness)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use zcrayvm_assembly::isa::GenericISA;

    use crate::model::Trace;
    use crate::prover::Prover;
    use crate::test_utils::generate_trace;

    /// Creates an execution trace for a simple program that uses the SRLI
    /// instruction to test shift operations.
    fn generate_logic_shift_immediate_trace() -> Result<Trace> {
        let asm_code = "#[framesize(0x10)]\n\
            _start:\n\
            LDI.W @3, #2\n\
            SRLI @4, @2, #2 \n\
            SRL  @5, @2, @3 \n\
            SLLI @6, @2, #2 \n\
            SLL  @7, @2, @3 \n\
            RET\n"
            .to_string();

        let init_values = vec![0, 0, 127];

        let vrom_writes = vec![
            (2, 127, 4),
            (3, 2, 3),
            // Initial values
            (0, 0, 1),
            (1, 0, 1),
            (4, 127 >> 2, 1),
            (5, 127 >> 2, 1),
            (6, 127 << 2, 1),
            (7, 127 << 2, 1),
        ];

        generate_trace(asm_code, Some(init_values), Some(vrom_writes))
    }

    #[test]
    fn test_logic_shift_immediate() -> Result<()> {
        let trace = generate_logic_shift_immediate_trace()?;
        trace.validate()?;
        assert_eq!(trace.srli_events().len(), 1);
        assert_eq!(trace.slli_events().len(), 1);
        assert_eq!(trace.srl_events().len(), 1);
        assert_eq!(trace.sll_events().len(), 1);
        Prover::new(Box::new(GenericISA)).validate_witness(&trace)
    }
}
