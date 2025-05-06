use std::{any::Any, ops::Deref};

use binius_m3::{
    builder::{
        upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1, B32,
    },
    gadgets::{
        u32::{U32Add, U32AddFlags},
        mul::MulUU32,
    },
};
use petravm_assembly::{opcodes::Opcode, AddEvent, SubEvent, MulEvent};

use crate::{
    channels::Channels,
    gadgets::state::{NextPc, StateColumns, StateColumnsOptions, StateGadget},
    table::Table,
    types::ProverPackedField,
};

const ADD_OPCODE: u16 = Opcode::Add as u16;
const MUL_OPCODE: u16 = Opcode::Mul as u16;

/// ADD table.
///
/// This table handles the ADD instruction, which performs integer
/// addition between two 32-bit elements.
pub struct AddTable {
    id: TableId,
    state_cols: StateColumns<ADD_OPCODE>,
    dst_abs: Col<B32>, // Virtual
    dst_val_packed: Col<B32>,
    src1_abs: Col<B32>, // Virtual
    src1_val: Col<B1, 32>,
    src2_abs: Col<B32>, // Virtual
    src2_val: Col<B1, 32>,
    add_op: U32Add,
}

impl Table for AddTable {
    type Event = AddEvent;

    fn name(&self) -> &'static str {
        "AddTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("add");

        let Channels {
            state_channel,
            prom_channel,
            vrom_channel,
            ..
        } = *channels;

        let state_cols = StateColumns::new(
            &mut table,
            state_channel,
            prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        // Pull the destination and source values from the VROM channel.
        let dst_abs = table.add_computed("dst", state_cols.fp + upcast_col(state_cols.arg0));
        let src1_abs = table.add_computed("src1", state_cols.fp + upcast_col(state_cols.arg1));
        let src1_val = table.add_committed("src1_val");
        let src1_val_packed = table.add_packed("src1_val_packed", src1_val);

        let src2_abs = table.add_computed("src2", state_cols.fp + upcast_col(state_cols.arg2));
        let src2_val = table.add_committed("src2_val");
        let src2_val_packed = table.add_packed("src2_val_packed", src2_val);

        // Carry out the addition.
        let add_op = U32Add::new(&mut table, src1_val, src2_val, U32AddFlags::default());
        let dst_val_packed = table.add_packed("dst_val_packed", add_op.zout);

        // Read src1
        table.pull(vrom_channel, [src1_abs, src1_val_packed]);

        // Read src2
        table.pull(vrom_channel, [src2_abs, src2_val_packed]);

        // Write dst
        table.pull(vrom_channel, [dst_abs, dst_val_packed]);

        Self {
            id: table.id(),
            state_cols,
            dst_abs,
            src1_abs,
            src1_val,
            src2_abs,
            src2_val,
            add_op,
            dst_val_packed,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TableFiller<ProverPackedField> for AddTable {
    type Event = AddEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut dst_abs = witness.get_scalars_mut(self.dst_abs)?;
            let mut src1_abs = witness.get_scalars_mut(self.src1_abs)?;
            let mut src1_val = witness.get_mut_as(self.src1_val)?;
            let mut src2_abs = witness.get_scalars_mut(self.src2_abs)?;
            let mut src2_val = witness.get_mut_as(self.src2_val)?;

            for (i, event) in rows.clone().enumerate() {
                dst_abs[i] = B32::new(event.fp.addr(event.dst));
                src1_abs[i] = B32::new(event.fp.addr(event.src1));
                src1_val[i] = event.src1_val;
                src2_abs[i] = B32::new(event.fp.addr(event.src2));
                src2_val[i] = event.src2_val;
            }
        }
        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.into(),
            next_pc: None,
            fp: *event.fp.deref(),
            arg0: event.dst,
            arg1: event.src1,
            arg2: event.src2,
        });
        self.state_cols.populate(witness, state_rows)?;
        self.add_op.populate(witness)
    }
}

/// SUB table.
///
/// This table handles the SUB instruction, which performs integer
/// subtraction between two 32-bit elements.
pub struct SubTable {
    id: TableId,
    state_cols: StateColumns<{ Opcode::Sub as u16 }>,
    dst_abs: Col<B32>, // Virtual
    dst_val: Col<B1, 32>,
    src1_abs: Col<B32>, // Virtual
    src2_abs: Col<B32>, // Virtual
    src2_val: Col<B1, 32>,
    add_op: U32Add,
}

impl Table for SubTable {
    type Event = SubEvent;

    fn name(&self) -> &'static str {
        "SubTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("sub");

        let Channels {
            state_channel,
            prom_channel,
            vrom_channel,
            ..
        } = *channels;

        let state_cols = StateColumns::new(
            &mut table,
            state_channel,
            prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        // Pull the destination and source values from the VROM channel.
        let dst_abs = table.add_computed("dst", state_cols.fp + upcast_col(state_cols.arg0));
        let dst_val = table.add_committed("dst_val");
        let dst_val_packed = table.add_packed("dst_val_packed", dst_val);

        let src1_abs = table.add_computed("src1", state_cols.fp + upcast_col(state_cols.arg1));

        let src2_abs = table.add_computed("src2", state_cols.fp + upcast_col(state_cols.arg2));
        let src2_val = table.add_committed("src2_val");
        let src2_val_packed = table.add_packed("src2_val_packed", src2_val);

        // Carry out the subtraction.
        let add_op = U32Add::new(&mut table, dst_val, src2_val, U32AddFlags::default());
        let src1_val_packed = table.add_packed("src1_val_packed", add_op.zout);

        // Read src1
        table.pull(vrom_channel, [src1_abs, src1_val_packed]);

        // Read src2
        table.pull(vrom_channel, [src2_abs, src2_val_packed]);

        // Write dst
        table.pull(vrom_channel, [dst_abs, dst_val_packed]);

        Self {
            id: table.id(),
            state_cols,
            dst_abs,
            src1_abs,
            src2_abs,
            src2_val,
            add_op,
            dst_val,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TableFiller<ProverPackedField> for SubTable {
    type Event = SubEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut dst_abs = witness.get_scalars_mut(self.dst_abs)?;
            let mut dst_val = witness.get_mut_as(self.dst_val)?;
            let mut src1_abs = witness.get_scalars_mut(self.src1_abs)?;
            let mut src2_abs = witness.get_scalars_mut(self.src2_abs)?;
            let mut src2_val = witness.get_mut_as(self.src2_val)?;

            for (i, event) in rows.clone().enumerate() {
                dst_abs[i] = B32::new(event.fp.addr(event.dst));
                dst_val[i] = event.dst_val;
                src1_abs[i] = B32::new(event.fp.addr(event.src1));
                src2_abs[i] = B32::new(event.fp.addr(event.src2));
                src2_val[i] = event.src2_val;
            }
        }
        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.into(),
            next_pc: None,
            fp: *event.fp.deref(),
            arg0: event.dst,
            arg1: event.src1,
            arg2: event.src2,
        });
        self.state_cols.populate(witness, state_rows)?;
        self.add_op.populate(witness)
    }
}

/// MUL table.
///
/// This table handles the MUL instruction, which performs integer
/// multiplication between two 32-bit elements. It returns a 64-bit result,
/// with the low 32 bits stored in the destination register.
pub struct MulTable {
    id: TableId,
    state_cols: StateColumns<MUL_OPCODE>,
    dst_abs: Col<B32>, // Virtual
    dst_val_packed: Col<B32>,
    src1_abs: Col<B32>, // Virtual
    src1_val: Col<B1, 32>,
    src2_abs: Col<B32>, // Virtual
    src2_val: Col<B1, 32>,
    mul_op: MulUU32,
}

impl Table for MulTable {
    type Event = MulEvent;

    fn name(&self) -> &'static str {
        "MulTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("mul");

        let Channels {
            state_channel,
            prom_channel,
            vrom_channel,
            ..
        } = *channels;

        let state_cols = StateColumns::new(
            &mut table,
            state_channel,
            prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        // Pull the destination and source values from the VROM channel.
        let dst_abs = table.add_computed("dst", state_cols.fp + upcast_col(state_cols.arg0));
        let src1_abs = table.add_computed("src1", state_cols.fp + upcast_col(state_cols.arg1));
        let src1_val = table.add_committed("src1_val");
        let src1_val_packed = table.add_packed("src1_val_packed", src1_val);

        let src2_abs = table.add_computed("src2", state_cols.fp + upcast_col(state_cols.arg2));
        let src2_val = table.add_committed("src2_val");
        let src2_val_packed = table.add_packed("src2_val_packed", src2_val);

        // Carry out the multiplication.
        let mul_op = MulUU32::new(&mut table);
        let dst_val_packed = table.add_packed("dst_val_packed", mul_op.out_low);

        // Read src1
        table.pull(vrom_channel, [src1_abs, src1_val_packed]);

        // Read src2
        table.pull(vrom_channel, [src2_abs, src2_val_packed]);

        // Write dst
        table.pull(vrom_channel, [dst_abs, dst_val_packed]);

        Self {
            id: table.id(),
            state_cols,
            dst_abs,
            dst_val_packed,
            src1_abs,
            src1_val,
            src2_abs,
            src2_val,
            mul_op,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TableFiller<ProverPackedField> for MulTable {
    type Event = MulEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> Result<(), anyhow::Error> {
        // Create clones of rows for each part of the method
        let rows_for_values = rows.clone();
        let rows_for_state = rows.clone();
        let rows_for_mul_op_src1 = rows.clone();
        let rows_for_mul_op_src2 = rows.clone();

        {
            let mut dst_abs = witness.get_scalars_mut(self.dst_abs)?;
            let mut src1_abs = witness.get_scalars_mut(self.src1_abs)?;
            let mut src1_val = witness.get_mut_as(self.src1_val)?;
            let mut src2_abs = witness.get_scalars_mut(self.src2_abs)?;
            let mut src2_val = witness.get_mut_as(self.src2_val)?;

            for (i, event) in rows_for_values.enumerate() {
                dst_abs[i] = B32::new(event.fp.addr(event.dst));
                src1_abs[i] = B32::new(event.fp.addr(event.src1));
                src1_val[i] = B32::new(event.src1_val);
                src2_abs[i] = B32::new(event.fp.addr(event.src2));
                src2_val[i] = B32::new(event.src2_val);
            }
        }
        let state_rows = rows_for_state.map(|event| StateGadget {
            pc: event.pc.into(),
            next_pc: None,
            fp: *event.fp.deref(),
            arg0: event.dst,
            arg1: event.src1,
            arg2: event.src2,
        });
        self.state_cols.populate(witness, state_rows)?;
        
        // Populate the mul_op with the appropriate values
        self.mul_op.populate(
            witness,
            rows_for_mul_op_src1.map(|event| B32::new(event.src1_val)),
            rows_for_mul_op_src2.map(|event| B32::new(event.src2_val)),
        )
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use binius_field::BinaryField;
    use petravm_assembly::isa::GenericISA;
    use proptest::prelude::*;
    use proptest::prop_oneof;

    use super::*;
    use crate::model::Trace;
    use crate::prover::Prover;
    use crate::test_utils::generate_trace;

    pub(crate) const G: B32 = B32::MULTIPLICATIVE_GENERATOR;

    /// Creates an execution trace for a simple program that uses the ADD
    /// instruction.
    fn generate_add_trace(src1_value: u32, src2_value: u32) -> Result<Trace> {
        let asm_code = format!(
            "#[framesize(0x10)]\n\
             _start: 
                LDI.W @2, #{}\n\
                LDI.W @3, #{}\n\
                ;; Skip @4 to test a gap in vrom writes
                ADD @5, @2, @3\n\
                RET\n",
            src1_value, src2_value
        );

        // Add VROM writes from LDI and ADD events
        let vrom_writes = vec![
            // LDI events
            (2, src1_value, 2),
            (3, src2_value, 2),
            // Initial values
            (0, 0, 1),
            (1, 0, 1),
            // ADD event
            (5, src1_value.wrapping_add(src2_value), 1),
        ];

        generate_trace(asm_code, None, Some(vrom_writes))
    }

    /// Creates an execution trace for a simple program that uses the SUB
    /// instruction.
    fn generate_sub_trace(src1_value: u32, src2_value: u32) -> Result<Trace> {
        let asm_code = format!(
            "#[framesize(0x10)]\n\
             _start: 
                LDI.W @2, #{}\n\
                LDI.W @3, #{}\n\
                ;; Skip @4 to test a gap in vrom writes
                SUB @5, @2, @3\n\
                RET\n",
            src1_value, src2_value
        );

        // Add VROM writes from LDI and SUB events
        let vrom_writes = vec![
            // LDI events
            (2, src1_value, 2),
            (3, src2_value, 2),
            // Initial values
            (0, 0, 1),
            (1, 0, 1),
            // SUB event
            (5, src1_value.wrapping_sub(src2_value), 1),
        ];

        generate_trace(asm_code, None, Some(vrom_writes))
    }

    /// Creates an execution trace for a simple program that uses the MUL
    /// instruction.
    fn generate_mul_trace(src1_value: u32, src2_value: u32) -> Result<Trace> {
        let asm_code = format!(
            "#[framesize(0x10)]\n\
             _start: 
                LDI.W @2, #{}\n\
                LDI.W @3, #{}\n\
                ;; Skip @4 to test a gap in vrom writes
                MUL @5, @2, @3\n\
                RET\n",
            src1_value, src2_value
        );

        // Calculate expected result (signed multiplication)
        let result = ((src1_value as i32) as i64 * (src2_value as i32) as i64) as u32;

        // Add VROM writes from LDI and MUL events
        let vrom_writes = vec![
            // LDI events
            (2, src1_value, 2),
            (3, src2_value, 2),
            // Initial values
            (0, 0, 1),
            (1, 0, 1),
            // MUL event (writing the low 32 bits of the result)
            (5, result, 1),
        ];

        generate_trace(asm_code, None, Some(vrom_writes))
    }

    fn test_add_with_values(src1_value: u32, src2_value: u32) -> Result<()> {
        let trace = generate_add_trace(src1_value, src2_value)?;
        trace.validate()?;
        assert_eq!(trace.add_events().len(), 1);
        assert_eq!(trace.ldi_events().len(), 2);
        assert_eq!(trace.ret_events().len(), 1);
        Prover::new(Box::new(GenericISA)).validate_witness(&trace)
    }

    fn test_sub_with_values(src1_value: u32, src2_value: u32) -> Result<()> {
        let trace = generate_sub_trace(src1_value, src2_value)?;
        trace.validate()?;
        assert_eq!(trace.sub_events().len(), 1);
        assert_eq!(trace.ldi_events().len(), 2);
        assert_eq!(trace.ret_events().len(), 1);
        Prover::new(Box::new(GenericISA)).validate_witness(&trace)
    }

    fn test_mul_with_values(src1_value: u32, src2_value: u32) -> Result<()> {
        let trace = generate_mul_trace(src1_value, src2_value)?;
        trace.validate()?;
        assert_eq!(trace.mul_events().len(), 1);
        assert_eq!(trace.ldi_events().len(), 2);
        assert_eq!(trace.ret_events().len(), 1);
        Prover::new(Box::new(GenericISA)).validate_witness(&trace)
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(20))]

        #[test]
        fn test_integer_operations(
            src1_value in prop_oneof![
                any::<u32>()                    // Random values
            ],
            src2_value in prop_oneof![
                any::<u32>()                    // Random values
            ],
        ) {
            prop_assert!(test_add_with_values(src1_value, src2_value).is_ok());
            prop_assert!(test_sub_with_values(src1_value, src2_value).is_ok());
            prop_assert!(test_mul_with_values(src1_value, src2_value).is_ok());
        }
    }
}
