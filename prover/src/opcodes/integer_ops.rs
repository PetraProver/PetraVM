use std::{any::Any, ops::Deref};

use binius_field::{Field, PackedBinaryField32x1b};
use binius_m3::{
    builder::{
        upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1, B32,
    },
    gadgets::{
        mul::MulSS32,
        u32::{U32Add, U32AddFlags},
    },
};
use petravm_assembly::{opcodes::Opcode, AddEvent, AddiEvent, MulEvent, SubEvent};

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
        // Read src1
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

/// MUL table.
///
/// This table handles the MUL instruction, which performs integer
/// multiplication between two 32-bit elements. It returns a 64-bit result,
/// with the low 32 bits stored in the destination vrom address and the
/// high 32 bits stored in the destination vrom address + 1.
pub struct MulTable {
    id: TableId,
    state_cols: StateColumns<{ Opcode::Mul as u16 }>,
    dst_abs: Col<B32>,
    dst_abs_plus_1: Col<B32>,
    dst_val_low: Col<B32>,
    dst_val_high: Col<B32>,
    src1_abs: Col<B32>,
    src1_val: Col<B32>,
    src2_abs: Col<B32>,
    src2_val: Col<B32>,
    mul_op: MulSS32,
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

        // Carry out the multiplication.
        let mul_op = MulSS32::new(&mut table);
        let MulSS32 {
            xin: src1_val,
            yin: src2_val,
            out_low: dst_val_low,
            out_high: dst_val_high,
            ..
        } = mul_op;

        // Pull the destination and source values from the VROM channel.
        let dst_abs = table.add_computed("dst", state_cols.fp + upcast_col(state_cols.arg0));
        let dst_abs_plus_1 = table.add_computed("dst_plus_1", dst_abs + B32::ONE);
        let src1_abs = table.add_computed("src1", state_cols.fp + upcast_col(state_cols.arg1));
        let src2_abs = table.add_computed("src2", state_cols.fp + upcast_col(state_cols.arg2));

        table.pull(vrom_channel, [src1_abs, src1_val]);
        table.pull(vrom_channel, [src2_abs, src2_val]);
        table.pull(vrom_channel, [dst_abs, dst_val_low]);
        table.pull(vrom_channel, [dst_abs_plus_1, dst_val_high]);

        Self {
            id: table.id(),
            state_cols,
            dst_abs,
            dst_abs_plus_1,
            dst_val_low,
            dst_val_high,
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
        {
            let mut dst_abs = witness.get_mut_as(self.dst_abs)?;
            let mut dst_abs_plus_1 = witness.get_mut_as(self.dst_abs_plus_1)?;
            let mut dst_val_low = witness.get_mut_as(self.dst_val_low)?;
            let mut dst_val_high = witness.get_mut_as(self.dst_val_high)?;
            let mut src1_abs = witness.get_mut_as(self.src1_abs)?;
            let mut src1_val = witness.get_mut_as(self.src1_val)?;
            let mut src2_abs = witness.get_mut_as(self.src2_abs)?;
            let mut src2_val = witness.get_mut_as(self.src2_val)?;

            for (i, event) in rows.clone().enumerate() {
                dst_abs[i] = event.fp.addr(event.dst as u32);
                dst_abs_plus_1[i] = event.fp.addr(event.dst as u32 + 1);
                dst_val_low[i] = event.dst_val as u32;
                dst_val_high[i] = (event.dst_val >> 32) as u32;
                src1_abs[i] = event.fp.addr(event.src1 as u32);
                src1_val[i] = event.src1_val;
                src2_abs[i] = event.fp.addr(event.src2 as u32);
                src2_val[i] = event.src2_val;
            }
        }

        let state_rows = rows.clone().map(|event| StateGadget {
            pc: event.pc.into(),
            next_pc: None,
            fp: *event.fp.deref(),
            arg0: event.dst,
            arg1: event.src1,
            arg2: event.src2,
        });
        self.state_cols.populate(witness, state_rows)?;

        let x_vals = rows.clone().map(|event| event.src1_val.into());
        let y_vals = rows.clone().map(|event| event.src2_val.into());
        self.mul_op.populate_with_inputs(witness, x_vals, y_vals)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use petravm_assembly::isa::GenericISA;
    use proptest::prelude::*;

    use crate::model::Trace;
    use crate::prover::Prover;
    use crate::test_utils::generate_trace;

    /// Creates an execution trace for a simple program that uses the immediate
    /// integer operations.
    fn generate_imm_integer_ops_trace(src_value: u32, imm_value: u16) -> Result<Trace> {
        let asm_code = format!(
            "#[framesize(0x10)]\n\
             _start: 
                LDI.W @2, #{}\n\
                ADDI @3, @2, #{}\n\
                RET\n",
            src_value, imm_value
        );

        let addi_result = src_value.wrapping_add((imm_value as i16 as i32) as u32);

        // Add VROM writes from LDI and ADDI events
        let vrom_writes = vec![
            // LDI event
            (2, src_value, 2),
            // Initial values
            (0, 0, 1),
            (1, 0, 1),
            // ADDI event
            (3, addi_result, 1),
        ];

        generate_trace(asm_code, None, Some(vrom_writes))
    }

    /// Creates an execution trace for a simple program that uses the unsigned
    /// integer operations.
    fn generate_vrom_integer_ops_trace_unsigned(src1_value: u32, src2_value: u32) -> Result<Trace> {
        let asm_code = format!(
            "#[framesize(0x10)]\n\
             _start: 
                LDI.W @2, #{}\n\
                LDI.W @3, #{}\n\
                SUB @4, @2, @3\n\
                ADD @5, @2, @3\n\
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
            (4, src1_value.wrapping_sub(src2_value), 1),
            // ADD event
            (5, src1_value.wrapping_add(src2_value), 1),
        ];

        generate_trace(asm_code, None, Some(vrom_writes))
    }

    /// Creates an execution trace for a simple program that uses the signed
    /// integer operations.
    fn generate_vrom_integer_ops_trace_signed(src1_value: i32, src2_value: i32) -> Result<Trace> {
        let asm_code = format!(
            "#[framesize(0x10)]\n\
             _start: 
                LDI.W @2, #{}\n\
                LDI.W @3, #{}\n\
                MUL @4, @2, @3\n\
                RET\n",
            src1_value as u32, src2_value as u32
        );

        let mul_result = ((src1_value as i64) * (src2_value as i64)) as u64;

        // Add VROM writes from LDI and SUB events
        let vrom_writes: Vec<(u32, u32, u32)> = vec![
            // LDI events
            (2, src1_value as u32, 2),
            (3, src2_value as u32, 2),
            // Initial values
            (0, 0, 1),
            (1, 0, 1),
            // MUL event
            (4, (mul_result & 0xFFFFFFFF) as u32, 1),
            (5, (mul_result >> 32) as u32, 1),
        ];

        generate_trace(asm_code, None, Some(vrom_writes))
    }

    fn test_imm_integer_ops_with_values(src_value: u32, imm: u16) -> Result<()> {
        let trace = generate_imm_integer_ops_trace(src_value, imm)?;
        trace.validate()?;
        assert_eq!(trace.addi_events().len(), 1);
        Prover::new(Box::new(GenericISA)).validate_witness(&trace)
    }

    fn test_vrom_integer_ops_with_values_unsigned(src1_value: u32, src2_value: u32) -> Result<()> {
        let trace = generate_vrom_integer_ops_trace_unsigned(src1_value, src2_value)?;
        trace.validate()?;
        assert_eq!(trace.sub_events().len(), 1);
        assert_eq!(trace.add_events().len(), 1);
        Prover::new(Box::new(GenericISA)).validate_witness(&trace)
    }

    fn test_vrom_integer_ops_with_values_signed(src1_value: i32, src2_value: i32) -> Result<()> {
        let trace = generate_vrom_integer_ops_trace_signed(src1_value, src2_value)?;
        trace.validate()?;
        assert_eq!(trace.mul_events().len(), 1);
        Prover::new(Box::new(GenericISA)).validate_witness(&trace)
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(20))]

        #[test]
        fn test_vrom_integer_ops_unsigned(
            src1_value in  any::<u32>(),
            src2_value in  any::<u32>(),
        ) {
            prop_assert!(test_vrom_integer_ops_with_values_unsigned(src1_value, src2_value).is_ok());
        }

        #[test]
        fn test_vrom_integer_ops_signed(
            src1_value in  any::<i32>(),
            src2_value in  any::<i32>(),
        ) {
            prop_assert!(test_vrom_integer_ops_with_values_signed(src1_value, src2_value).is_ok());
        }

        #[test]
        fn test_imm_integer_ops(
            src_value in any::<u32>(),
            imm in any::<u16>(),
        ) {
            prop_assert!(test_imm_integer_ops_with_values(src_value, imm).is_ok());
        }
    }
}
