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

macro_rules! define_logic_shift_table {
    // Immediate form
    (imm: $Name:ident, $table_str:expr,
         Event=$Event:ty,
         OPCODE=$OpCode:expr,
         VARIANT=$ShiftVar:expr) => {
        pub struct $Name {
            id: TableId,
            cpu_cols: CpuColumns<{ $OpCode as u16 }>,
            shifter: BarrelShifter,
            dst_abs: Col<B32>,
            dst_val: Col<B32>,
            src_abs: Col<B32>,
            src_val: Col<B32>,
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

                // common unpack→packed for src
                let src_val_unpacked: Col<B1, 32> = table.add_committed("src_val_unpacked");
                let src_val: Col<B32> = table.add_packed("src_val", src_val_unpacked);

                // same for dst/src addrs
                let dst_abs =
                    table.add_computed("dst_abs", cpu_cols.fp + upcast_col(cpu_cols.arg0));
                let src_abs =
                    table.add_computed("src_abs", cpu_cols.fp + upcast_col(cpu_cols.arg1));

                // barrel shifter wired to cpu_cols.arg2_unpacked
                let shifter = BarrelShifter::new(
                    &mut table,
                    src_val_unpacked,
                    cpu_cols.arg2_unpacked,
                    $ShiftVar,
                );
                let dst_val = table.add_packed("dst_val", shifter.output);

                // pulls for plain imm form
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
                // fill src_val, dst_abs, src_abs
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

                // CPU gadget + shifter
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

    // vrom‐based form
    (reg: $Name:ident, $table_str:expr,
         Event=$Event:ty,
         OPCODE=$OpCode:expr,
         VARIANT=$ShiftVar:expr) => {
        pub struct $Name {
            id: TableId,
            cpu_cols: CpuColumns<{ $OpCode as u16 }>,
            shifter: BarrelShifter,
            dst_abs: Col<B32>,
            dst_val: Col<B32>,
            src_abs: Col<B32>,
            // unpacked val + shift
            src_val_unpacked: Col<B1, 32>,
            shift_abs: Col<B32>,
            shift_amount_unpacked: Col<B1, 16>,
            shift_val: Col<B32>,
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

                let src_val_unpacked: Col<B1, 32> = table.add_committed("src_val_unpacked");
                let src_val: Col<B32> = table.add_packed("src_val", src_val_unpacked);
                let dst_abs =
                    table.add_computed("dst_abs", cpu_cols.fp + upcast_col(cpu_cols.arg0));
                let src_abs =
                    table.add_computed("src_abs", cpu_cols.fp + upcast_col(cpu_cols.arg1));
                let shift_abs =
                    table.add_computed("shift_abs", cpu_cols.fp + upcast_col(cpu_cols.arg2));

                let shift_amount_unpacked: Col<B1, 16> =
                    table.add_committed("shift_amount_unpacked");
                let shift_amount_packed: Col<B16, 1> =
                    table.add_packed("shift_amount", shift_amount_unpacked);
                let shift_val = upcast_col(shift_amount_packed);

                let shifter = BarrelShifter::new(
                    &mut table,
                    src_val_unpacked,
                    shift_amount_unpacked,
                    $ShiftVar,
                );
                let dst_val = table.add_packed("dst_val", shifter.output);

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
                // basic & shift columns
                {
                    let mut dst_abs = witness.get_mut_as(self.dst_abs)?;
                    let mut src_abs = witness.get_mut_as(self.src_abs)?;
                    let mut src_un = witness.get_mut_as(self.src_val_unpacked)?;
                    let mut shift_abs = witness.get_mut_as(self.shift_abs)?;
                    let mut shift_un = witness.get_mut_as(self.shift_amount_unpacked)?;

                    for (i, ev) in rows.clone().enumerate() {
                        src_un[i] = ev.src_val;
                        dst_abs[i] = ev.fp.addr(ev.dst);
                        src_abs[i] = ev.fp.addr(ev.src);
                        shift_abs[i] = ev.fp.addr(ev.shift);
                        shift_un[i] = ev.shift_amount as u16;
                    }
                }

                // CPU gadget
                let cpu_rows = rows.clone().map(|ev| CpuGadget {
                    pc: ev.pc.val(),
                    next_pc: None,
                    fp: *ev.fp,
                    arg0: ev.dst,
                    arg1: ev.src,
                    arg2: ev.shift,
                });
                self.cpu_cols.populate(witness, cpu_rows)?;

                // last, shifter
                self.shifter.populate(witness)?;
                Ok(())
            }
        }
    };
}

define_logic_shift_table!(imm: SrliTable, "srli",
     Event=SrliEvent, OPCODE=Opcode::Srli, VARIANT=ShiftVariant::LogicalRight);

define_logic_shift_table!(imm: SlliTable, "slli",
     Event=SlliEvent, OPCODE=Opcode::Slli, VARIANT=ShiftVariant::LogicalLeft);

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
