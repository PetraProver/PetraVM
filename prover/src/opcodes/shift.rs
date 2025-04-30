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

/// This macro generates table structures for shift operations.
/// Two variants are supported:
///   - `imm`: For immediate shift operations
///   - `reg`: For vrom-based shift operations (shift amount from a vrom value)
///
/// The macro generates:
/// 1. A table structure with columns for the operation
/// 2. Table implementation for accessing the table
/// 3. TableFiller implementation for populating the table with shift events
macro_rules! define_logic_shift_table {
    // Immediate variant: For shift operations with immediate shift amounts
    // Parameters:
    //   - $Name: The name of the generated table structure
    //   - $table_str: String identifier for the table
    //   - Event: The event type that this table handles
    //   - OPCODE: The opcode enum value for this operation
    //   - VARIANT: The shift variant (logical left/right)
    (imm: $Name:ident, $table_str:expr,
         Event=$Event:ty,
         OPCODE=$OpCode:expr,
         VARIANT=$ShiftVar:expr) => {
        pub struct $Name {
            id: TableId,
            cpu_cols: CpuColumns<{ $OpCode as u16 }>,
            shifter: BarrelShifter,
            dst_abs: Col<B32>, // Destination absolute address
            dst_val: Col<B32>, // Destination value (shift result)
            src_abs: Col<B32>, // Source absolute address
            src_val: Col<B32>, // Source value (value to be shifted)
        }

        impl Table for $Name {
            type Event = $Event;
            fn name(&self) -> &'static str {
                stringify!($Name)
            }
            fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
                let mut table = cs.add_table($table_str);
                let cpu_cols = CpuColumns::new(
                    &mut table,
                    channels.state_channel,
                    channels.prom_channel,
                    CpuColumnsOptions::default(),
                );

                // Common unpack→packed columns for source value
                let src_val_unpacked: Col<B1, 32> = table.add_committed("src_val_unpacked");
                let src_val: Col<B32> = table.add_packed("src_val", src_val_unpacked);

                // Absolute addresses for destination and source
                let dst_abs =
                    table.add_computed("dst_abs", cpu_cols.fp + upcast_col(cpu_cols.arg0));
                let src_abs =
                    table.add_computed("src_abs", cpu_cols.fp + upcast_col(cpu_cols.arg1));

                // Barrel shifter wired to cpu_cols.arg2_unpacked (immediate shift amount)
                let shifter = BarrelShifter::new(
                    &mut table,
                    src_val_unpacked,
                    cpu_cols.arg2_unpacked,
                    $ShiftVar,
                );
                let dst_val = table.add_packed("dst_val", shifter.output);

                // Pull columns from VROM channel
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

        impl TableFiller<ProverPackedField> for $Name {
            type Event = $Event;
            fn id(&self) -> TableId {
                self.id
            }

            fn fill<'a>(
                &'a self,
                rows: impl Iterator<Item = &'a $Event> + Clone,
                witness: &'a mut TableWitnessSegment<ProverPackedField>,
            ) -> anyhow::Result<()> {
                // Fill source value, destination address, and source address
                {
                    let mut src_val = witness.get_mut_as(self.src_val)?;
                    let mut dst_abs = witness.get_mut_as(self.dst_abs)?;
                    let mut src_abs = witness.get_mut_as(self.src_abs)?;

                    for (i, ev) in rows.clone().enumerate() {
                        src_val[i] = ev.src_val;
                        dst_abs[i] = ev.fp.addr(ev.dst);
                        src_abs[i] = ev.fp.addr(ev.src);
                    }
                }

                // Populate CPU gadget and shifter
                let cpu_rows = rows.map(|ev| CpuGadget {
                    pc: ev.pc.val(),
                    next_pc: None,
                    fp: *ev.fp,
                    arg0: ev.dst,
                    arg1: ev.src,
                    arg2: ev.shift_amount as u16,
                });
                self.cpu_cols.populate(witness, cpu_rows)?;
                self.shifter.populate(witness)?;
                Ok(())
            }
        }
    };

    // Register variant: For shift operations where shift amount comes from a vrom value
    // Parameters:
    //   - $Name: The name of the generated table structure
    //   - $table_str: String identifier for the table
    //   - Event: The event type that this table handles
    //   - OPCODE: The opcode enum value for this operation
    //   - VARIANT: The shift variant (logical left/right)
    (reg: $Name:ident, $table_str:expr,
         Event=$Event:ty,
         OPCODE=$OpCode:expr,
         VARIANT=$ShiftVar:expr) => {
        pub struct $Name {
            id: TableId,
            cpu_cols: CpuColumns<{ $OpCode as u16 }>,
            shifter: BarrelShifter,
            dst_abs: Col<B32>,                  // Destination absolute address
            dst_val: Col<B32>,                  // Destination value (shift result)
            src_abs: Col<B32>,                  // Source absolute address
            src_val_unpacked: Col<B1, 32>,      // Source value in bit-unpacked form
            shift_abs: Col<B32>,                // Shift vrom absolute address
            shift_amount_unpacked: Col<B1, 16>, // Shift amount in bit-unpacked form
            shift_val: Col<B32>,                // Shift value (full vrom value)
        }

        impl Table for $Name {
            type Event = $Event;
            fn name(&self) -> &'static str {
                stringify!($Name)
            }
            fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
                let mut table = cs.add_table($table_str);
                let cpu_cols = CpuColumns::new(
                    &mut table,
                    channels.state_channel,
                    channels.prom_channel,
                    CpuColumnsOptions::default(),
                );

                // Source value columns
                let src_val_unpacked: Col<B1, 32> = table.add_committed("src_val_unpacked");
                let src_val: Col<B32> = table.add_packed("src_val", src_val_unpacked);

                // Address calculations
                let dst_abs =
                    table.add_computed("dst_abs", cpu_cols.fp + upcast_col(cpu_cols.arg0));
                let src_abs =
                    table.add_computed("src_abs", cpu_cols.fp + upcast_col(cpu_cols.arg1));
                let shift_abs =
                    table.add_computed("shift_abs", cpu_cols.fp + upcast_col(cpu_cols.arg2));

                // Shift amount columns
                let shift_amount_unpacked: Col<B1, 16> =
                    table.add_committed("shift_amount_unpacked");
                let shift_amount_packed: Col<B16, 1> =
                    table.add_packed("shift_amount", shift_amount_unpacked);
                let shift_val = upcast_col(shift_amount_packed);

                // Barrel shifter for the actual shift operation
                let shifter = BarrelShifter::new(
                    &mut table,
                    src_val_unpacked,
                    shift_amount_unpacked,
                    $ShiftVar,
                );
                let dst_val = table.add_packed("dst_val", shifter.output);

                // Pull memory access data from VROM channel
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
                    src_val_unpacked,
                    shift_abs,
                    shift_amount_unpacked,
                    shift_val,
                }
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        impl TableFiller<ProverPackedField> for $Name {
            type Event = $Event;

            fn id(&self) -> TableId {
                self.id
            }

            fn fill<'a>(
                &'a self,
                rows: impl Iterator<Item = &'a $Event> + Clone,
                witness: &'a mut TableWitnessSegment<ProverPackedField>,
            ) -> anyhow::Result<()> {
                // Fill basic columns and shift amount data
                {
                    let mut dst_abs = witness.get_mut_as(self.dst_abs)?;
                    let mut src_abs = witness.get_mut_as(self.src_abs)?;
                    let mut src_unpacked = witness.get_mut_as(self.src_val_unpacked)?;
                    let mut shift_abs = witness.get_mut_as(self.shift_abs)?;
                    let mut shift_unpacked = witness.get_mut_as(self.shift_amount_unpacked)?;

                    for (i, ev) in rows.clone().enumerate() {
                        src_unpacked[i] = ev.src_val;
                        dst_abs[i] = ev.fp.addr(ev.dst);
                        src_abs[i] = ev.fp.addr(ev.src);
                        shift_abs[i] = ev.fp.addr(ev.shift);
                        shift_unpacked[i] = ev.shift_amount as u16;
                    }
                }

                // Populate CPU gadget columns
                let cpu_rows = rows.clone().map(|ev| CpuGadget {
                    pc: ev.pc.val(),
                    next_pc: None,
                    fp: *ev.fp,
                    arg0: ev.dst,
                    arg1: ev.src,
                    arg2: ev.shift,
                });
                self.cpu_cols.populate(witness, cpu_rows)?;

                // Populate barrel shifter columns
                self.shifter.populate(witness)?;
                Ok(())
            }
        }
    };
}

// Define immediate shift amount tables
define_logic_shift_table!(imm: SrliTable, "srli",
     Event=SrliEvent, OPCODE=Opcode::Srli, VARIANT=ShiftVariant::LogicalRight);

define_logic_shift_table!(imm: SlliTable, "slli",
     Event=SlliEvent, OPCODE=Opcode::Slli, VARIANT=ShiftVariant::LogicalLeft);

// Define vrom-based shift amount tables
define_logic_shift_table!(reg:  SrlTable,  "srl",
     Event=SrlEvent,  OPCODE=Opcode::Srl,  VARIANT=ShiftVariant::LogicalRight);

define_logic_shift_table!(reg:  SllTable,  "sll",
     Event=SllEvent,  OPCODE=Opcode::Sll,  VARIANT=ShiftVariant::LogicalLeft);

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use zcrayvm_assembly::isa::GenericISA;

    use crate::model::Trace;
    use crate::prover::Prover;
    use crate::test_utils::generate_trace;

    /// Creates an execution trace for a simple program that uses various shift
    /// instructions to test shift operations.
    ///
    /// The test program performs:
    /// - SRLI: Logical right shift with immediate value
    /// - SRL: Logical right shift with vrom value
    /// - SLLI: Logical left shift with immediate value
    /// - SLL: Logical left shift with vrom value
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
            // Used for all shift operations
            (2, 127, 4),
            // LDI + SRL + SLL
            (3, 2, 3),
            // Initial values
            (0, 0, 1),
            (1, 0, 1),
            // Shift operations
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
