use std::any::Any;

use binius_field::{Field, PackedBinaryField32x1b};
use binius_m3::{
    builder::{
        upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1, B32,
    },
    gadgets::add::{U32Add, U32AddFlags},
};
use petravm_asm::{opcodes::Opcode, AddEvent, AddiEvent, SubEvent};

use crate::{
    channels::Channels,
    gadgets::state::{NextPc, StateColumns, StateColumnsOptions, StateGadget},
    table::Table,
    types::ProverPackedField,
    utils::setup_mux_constraint,
};

/// ADD table.
///
/// This table handles the ADD instruction, which performs integer
/// addition between two 32-bit elements.
pub struct AddTable {
    id: TableId,
    state_cols: StateColumns<{ Opcode::Add as u16 }>,
    dst_abs: Col<B32>,  // Virtual
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
            fp: *event.fp,
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
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.src1,
            arg2: event.src2,
        });
        self.state_cols.populate(witness, state_rows)?;
        self.add_op.populate(witness)
    }
}

/// ADDI table.
///
/// This table handles the ADDI instruction, which performs signed integer
/// addition between a 32-bit element and a 16-bit immediate.
pub struct AddiTable {
    id: TableId,
    state_cols: StateColumns<{ Opcode::Addi as u16 }>,
    dst_abs: Col<B32>, // Virtual
    src_abs: Col<B32>, // Virtual
    src_val: Col<B1, 32>,
    imm_32b: Col<B1, 32>, // Virtual
    msb: Col<B1>,         // Virtual
    negative: Col<B1, 32>,
    signed_imm_32b: Col<B1, 32>,
    ones: Col<B1, 32>,
    add_op: U32Add,
}

impl Table for AddiTable {
    type Event = AddiEvent;

    fn name(&self) -> &'static str {
        "AddiTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("addi");

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

        let dst_abs = table.add_computed("dst", state_cols.fp + upcast_col(state_cols.arg0));
        let src_abs = table.add_computed("src", state_cols.fp + upcast_col(state_cols.arg1));
        let src_val = table.add_committed("src_val");
        let src_val_packed = table.add_packed("src_val_packed", src_val);

        let imm_unpacked = state_cols.arg2_unpacked;
        let imm_32b = table.add_zero_pad("imm_32b", imm_unpacked, 0);

        // We need to sign extend `imm`. First, get the sign bit and the necessary
        // constants.
        let msb = table.add_selected("msb", imm_unpacked, 15);
        let mut constants = [B1::ONE; 32];
        for c in constants.iter_mut().take(16) {
            *c = B1::ZERO;
        }
        let ones = table.add_constant("ones", constants);

        // Compute the negative case.
        let negative = table.add_computed("negative", ones + imm_32b);

        // We commit to the sign-extended value.
        let signed_imm_32b = table.add_committed("signed_imm_32b");

        // Check that the sign extension is correct.
        setup_mux_constraint(&mut table, &signed_imm_32b, &negative, &imm_32b, &msb);

        // Carry out the addition.
        let add_op = U32Add::new(&mut table, src_val, signed_imm_32b, U32AddFlags::default());
        let dst_val_packed = table.add_packed("dst_val_packed", add_op.zout);

        // Pull the destination and source values from the VROM channel.
        // Read src
        table.pull(vrom_channel, [src_abs, src_val_packed]);

        // Write dst
        table.pull(vrom_channel, [dst_abs, dst_val_packed]);

        Self {
            id: table.id(),
            state_cols,
            dst_abs,
            src_abs,
            src_val,
            imm_32b,
            msb,
            negative,
            signed_imm_32b,
            ones,
            add_op,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TableFiller<ProverPackedField> for AddiTable {
    type Event = AddiEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut dst_abs = witness.get_mut_as(self.dst_abs)?;
            let mut src_abs = witness.get_mut_as(self.src_abs)?;
            let mut src_val = witness.get_mut_as(self.src_val)?;
            let mut imm = witness.get_mut_as(self.imm_32b)?;
            let mut msb: std::cell::RefMut<'_, [PackedBinaryField32x1b]> =
                witness.get_mut_as(self.msb)?;
            let mut negative = witness.get_mut_as(self.negative)?;
            let mut signed_imm_32b = witness.get_mut_as(self.signed_imm_32b)?;
            let mut ones_col = witness.get_mut_as(self.ones)?;

            for (i, event) in rows.clone().enumerate() {
                dst_abs[i] = event.fp.addr(event.dst as u32);
                src_abs[i] = event.fp.addr(event.src as u32);
                src_val[i] = event.src_val;
                imm[i] = event.imm as u32;

                // Calculate imm's MSB.
                let is_negative = (event.imm >> 15) & 1 == 1;
                binius_field::packed::set_packed_slice(&mut msb, i, B1::from(is_negative));

                // Compute the sign extension of `imm`.
                let ones = 0b1111_1111_1111_1111u32;
                ones_col[i] = ones << 16;
                negative[i] = (ones << 16) + event.imm as u32;
                signed_imm_32b[i] = event.imm as i16 as i32;
            }
        }
        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.into(),
            next_pc: None,
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.src,
            arg2: event.imm,
        });
        self.state_cols.populate(witness, state_rows)?;
        self.add_op.populate(witness)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use binius_field::BinaryField;
    use petravm_asm::isa::GenericISA;
    use proptest::prelude::*;
    use proptest::prop_oneof;

    use super::*;
    use crate::model::Trace;
    use crate::prover::Prover;
    use crate::test_utils::generate_trace;

    pub(crate) const G: B32 = B32::MULTIPLICATIVE_GENERATOR;

    /// Creates an execution trace for a simple program that uses the ADDI
    /// instruction.
    fn generate_imm_integer_ops_trace(src_value: u32, imm_value: u16) -> Result<Trace> {
        let asm_code = format!(
            "#[framesize(0x10)]\n\
             _start: 
                LDI.W @2, #{}\n\
                ADDI @3, @2, #{}\n\
                RET\n",
            src_value, imm_value
        );

        // Add VROM writes from LDI and ADDI events
        let vrom_writes = vec![
            // LDI event
            (2, src_value, 2),
            // Initial values
            (0, 0, 1),
            (1, 0, 1),
            // ADDI event
            (
                3,
                src_value.wrapping_add((imm_value as i16 as i32) as u32),
                1,
            ),
        ];

        generate_trace(asm_code, None, Some(vrom_writes))
    }

    /// Creates an execution trace for a simple program that uses the SUB and
    /// ADD instructions.
    fn generate_vrom_integer_ops_trace(src1_value: u32, src2_value: u32) -> Result<Trace> {
        let asm_code = format!(
            "#[framesize(0x10)]\n\
             _start: 
                LDI.W @2, #{}\n\
                LDI.W @3, #{}\n\
                ;; Skip @4 to test a gap in vrom writes
                SUB @5, @2, @3\n\
                ADD @6, @2, @3\n\
                RET\n",
            src1_value, src2_value
        );

        // Add VROM writes from LDI and SUB events
        let vrom_writes = vec![
            // LDI events
            (2, src1_value, 3),
            (3, src2_value, 3),
            // Initial values
            (0, 0, 1),
            (1, 0, 1),
            // SUB event
            (5, src1_value.wrapping_sub(src2_value), 1),
            // ADD event
            (6, src1_value.wrapping_add(src2_value), 1),
        ];

        generate_trace(asm_code, None, Some(vrom_writes))
    }

    fn test_imm_integer_ops_with_values(src_value: u32, imm: u16) -> Result<()> {
        let trace = generate_imm_integer_ops_trace(src_value, imm)?;
        trace.validate()?;
        assert_eq!(trace.addi_events().len(), 1);
        Prover::new(Box::new(GenericISA)).validate_witness(&trace)
    }

    fn test_vrom_integer_ops_with_values(src1_value: u32, src2_value: u32) -> Result<()> {
        let trace = generate_vrom_integer_ops_trace(src1_value, src2_value)?;
        trace.validate()?;
        assert_eq!(trace.sub_events().len(), 1);
        assert_eq!(trace.add_events().len(), 1);
        Prover::new(Box::new(GenericISA)).validate_witness(&trace)
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(20))]

        #[test]
        fn test_vrom_integer_ops(
            src1_value in prop_oneof![
                any::<u32>()                    // Random values
            ],
            src2_value in prop_oneof![
                any::<u32>()                    // Random values
            ],
        ) {
            prop_assert!(test_vrom_integer_ops_with_values(src1_value, src2_value).is_ok());

        }
        #[test]
    fn test_imm_integer_ops(
        src in any::<u32>(),
        imm in any::<u16>(),
    ) {
        prop_assert!(test_imm_integer_ops_with_values(src, imm).is_ok());
    }
    }
}
