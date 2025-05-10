use binius_field::PackedField;
use binius_m3::{
    builder::{
        upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1, B32,
    },
    gadgets::sub::{U32Sub, U32SubFlags},
};
use petravm_asm::{opcodes::Opcode, SleiuEvent, SleuEvent, SltiuEvent, SltuEvent};

use crate::{
    channels::Channels,
    gadgets::state::{NextPc, StateColumns, StateColumnsOptions, StateGadget},
    table::Table,
    types::ProverPackedField,
};

const SLTU_OPCODE: u16 = Opcode::Sltu as u16;
const SLTIU_OPCODE: u16 = Opcode::Sltiu as u16;
const SLEU_OPCODE: u16 = Opcode::Sleu as u16;
const SLEIU_OPCODE: u16 = Opcode::Sleiu as u16;

/// SLTU table.
///
/// This table handles the SLTU instruction, which performs unsigned
/// integer comparison (set if less than) between two 32-bit elements.
pub struct SltuTable {
    id: TableId,
    state_cols: StateColumns<SLTU_OPCODE>,
    dst_abs: Col<B32>,
    src1_abs: Col<B32>,
    src1_val: Col<B1, 32>,
    src2_abs: Col<B32>,
    src2_val: Col<B1, 32>,
    subber: U32Sub,
}

impl Table for SltuTable {
    type Event = SltuEvent;

    fn name(&self) -> &'static str {
        "SltuTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("sltu");

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
        let src2_abs = table.add_computed("src2", state_cols.fp + upcast_col(state_cols.arg2));

        let src1_val = table.add_committed("src1_val");
        let src1_val_packed = table.add_packed("src1_val_packed", src1_val);

        let src2_val = table.add_committed("src2_val");
        let src2_val_packed = table.add_packed("src2_val_packed", src2_val);

        // Instantiate the subtractor with the appropriate flags
        let flags = U32SubFlags {
            borrow_in_bit: None,       // no extra borrow-in
            expose_final_borrow: true, // we want the "underflow" bit out
            commit_zout: false,        // we don't need the raw subtraction result
        };
        let subber = U32Sub::new(&mut table, src1_val, src2_val, flags);
        // `final_borrow` is 1 exactly when src1_val < src2_val
        let final_borrow: Col<B1> = subber
            .final_borrow
            .expect("Flag `expose_final_borrow` was set to `true`");
        let dst_val = upcast_col(final_borrow);

        // Read src1 and src2
        table.pull(vrom_channel, [src1_abs, src1_val_packed]);
        table.pull(vrom_channel, [src2_abs, src2_val_packed]);

        // Read dst
        table.pull(vrom_channel, [dst_abs, dst_val]);

        Self {
            id: table.id(),
            state_cols,
            dst_abs,
            src1_abs,
            src1_val,
            src2_abs,
            src2_val,
            subber,
        }
    }
}

impl TableFiller<ProverPackedField> for SltuTable {
    type Event = SltuEvent;

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
        self.subber.populate(witness)
    }
}

/// SLTIU table.
///
/// This table handles the SLTIU instruction, which performs unsigned
/// integer comparison (set if less than) between a 32-bit element and
/// a 16-bit immediate.
pub struct SltiuTable {
    id: TableId,
    state_cols: StateColumns<SLTIU_OPCODE>,
    dst_abs: Col<B32>,
    src_abs: Col<B32>,
    src_val: Col<B1, 32>,
    imm_32b: Col<B1, 32>,
    subber: U32Sub,
}

impl Table for SltiuTable {
    type Event = SltiuEvent;

    fn name(&self) -> &'static str {
        "SltiuTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("sltiu");

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
        let src_abs = table.add_computed("src", state_cols.fp + upcast_col(state_cols.arg1));

        let src_val = table.add_committed("src_val");
        let src_val_packed = table.add_packed("src_val_packed", src_val);

        let imm_unpacked = state_cols.arg2_unpacked;
        let imm_32b = table.add_zero_pad("imm_32b", imm_unpacked, 0);

        // Instantiate the subtractor with the appropriate flags
        let flags = U32SubFlags {
            borrow_in_bit: None,       // no extra borrow-in
            expose_final_borrow: true, // we want the "underflow" bit out
            commit_zout: false,        // we don't need the raw subtraction result
        };
        let subber = U32Sub::new(&mut table, src_val, imm_32b, flags);
        // `final_borrow` is 1 exactly when src_val < imm_val
        let final_borrow: Col<B1> = subber
            .final_borrow
            .expect("Flag `expose_final_borrow` was set to `true`");
        let dst_val = upcast_col(final_borrow);

        // Read src
        table.pull(vrom_channel, [src_abs, src_val_packed]);

        // Read dst
        table.pull(vrom_channel, [dst_abs, dst_val]);

        Self {
            id: table.id(),
            state_cols,
            dst_abs,
            src_abs,
            src_val,
            imm_32b,
            subber,
        }
    }
}

impl TableFiller<ProverPackedField> for SltiuTable {
    type Event = SltiuEvent;

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
            let mut src_abs = witness.get_scalars_mut(self.src_abs)?;
            let mut src_val = witness.get_mut_as(self.src_val)?;
            let mut imm = witness.get_mut_as(self.imm_32b)?;

            for (i, event) in rows.clone().enumerate() {
                dst_abs[i] = B32::new(event.fp.addr(event.dst));
                src_abs[i] = B32::new(event.fp.addr(event.src));
                src_val[i] = event.src_val;
                imm[i] = event.imm;
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
        self.subber.populate(witness)
    }
}

/// SLEU table.
///
/// This table handles the SLEU instruction, which performs unsigned
/// integer comparison (set if less or equal than) between two 32-bit elements.
pub struct SleuTable {
    id: TableId,
    state_cols: StateColumns<SLEU_OPCODE>,
    dst_abs: Col<B32>,
    dst_val: Col<B32>,
    src1_abs: Col<B32>,
    src1_val: Col<B1, 32>,
    src2_abs: Col<B32>,
    src2_val: Col<B1, 32>,
    subber: U32Sub,
}

impl Table for SleuTable {
    type Event = SleuEvent;

    fn name(&self) -> &'static str {
        "SleuTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("sleu");

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
        let src2_abs = table.add_computed("src2", state_cols.fp + upcast_col(state_cols.arg2));

        let src1_val = table.add_committed("src1_val");
        let src1_val_packed = table.add_packed("src1_val_packed", src1_val);

        let src2_val = table.add_committed("src2_val");
        let src2_val_packed = table.add_packed("src2_val_packed", src2_val);

        // Instantiate the subtractor with the appropriate flags
        let flags = U32SubFlags {
            borrow_in_bit: None,       // no extra borrow-in
            expose_final_borrow: true, // we want the "underflow" bit out
            commit_zout: false,        // we don't need the raw subtraction result
        };
        // src1_val <= src2_val <=> !(src2_val < src1_val)
        let subber = U32Sub::new(&mut table, src2_val, src1_val, flags);

        // `final_borrow` is 1 exactly when src2_val < src1_val
        let final_borrow: Col<B1> = subber
            .final_borrow
            .expect("Flag `expose_final_borrow` was set to `true`");

        // flip the borrow bit
        let dst_val = table.add_computed("dst_val", final_borrow + B1::one());
        let dst_val = upcast_col(dst_val);

        // Read src1 and src2
        table.pull(vrom_channel, [src1_abs, src1_val_packed]);
        table.pull(vrom_channel, [src2_abs, src2_val_packed]);

        // Read dst
        table.pull(vrom_channel, [dst_abs, dst_val]);

        Self {
            id: table.id(),
            state_cols,
            dst_abs,
            dst_val,
            src1_abs,
            src1_val,
            src2_abs,
            src2_val,
            subber,
        }
    }
}

impl TableFiller<ProverPackedField> for SleuTable {
    type Event = SleuEvent;

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
            let mut dst_val = witness.get_scalars_mut(self.dst_val)?;
            let mut src1_abs = witness.get_scalars_mut(self.src1_abs)?;
            let mut src1_val = witness.get_mut_as(self.src1_val)?;
            let mut src2_abs = witness.get_scalars_mut(self.src2_abs)?;
            let mut src2_val = witness.get_mut_as(self.src2_val)?;

            for (i, event) in rows.clone().enumerate() {
                dst_abs[i] = B32::new(event.fp.addr(event.dst));
                dst_val[i] = B32::new(event.dst_val);
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
        self.subber.populate(witness)
    }
}

/// SLEIU table.
///
/// This table handles the SLEIU instruction, which performs unsigned
/// integer comparison (set if less or equal than) between a 32-bit
/// element and a 16-bit immediate.
pub struct SleiuTable {
    id: TableId,
    state_cols: StateColumns<SLEIU_OPCODE>,
    dst_abs: Col<B32>,
    dst_val: Col<B32>,
    src_abs: Col<B32>,
    src_val: Col<B1, 32>,
    imm_32b: Col<B1, 32>,
    subber: U32Sub,
}

impl Table for SleiuTable {
    type Event = SleiuEvent;

    fn name(&self) -> &'static str {
        "SleiuTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("sleiu");

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
        let src_abs = table.add_computed("src", state_cols.fp + upcast_col(state_cols.arg1));

        let src_val = table.add_committed("src_val");
        let src_val_packed = table.add_packed("src_val_packed", src_val);

        let imm_unpacked = state_cols.arg2_unpacked;
        let imm_32b = table.add_zero_pad("imm_32b", imm_unpacked, 0);

        // Instantiate the subtractor with the appropriate flags
        let flags = U32SubFlags {
            borrow_in_bit: None,       // no extra borrow-in
            expose_final_borrow: true, // we want the "underflow" bit out
            commit_zout: false,        // we don't need the raw subtraction result
        };
        // src_val <= imm_val <=> !(imm_val < src_val)
        let subber = U32Sub::new(&mut table, imm_32b, src_val, flags);

        // `final_borrow` is 1 exactly when imm_val < src_val
        let final_borrow: Col<B1> = subber
            .final_borrow
            .expect("Flag `expose_final_borrow` was set to `true`");

        // flip the borrow bit
        let dst_val = table.add_computed("dst_val", final_borrow + B1::one());
        let dst_val = upcast_col(dst_val);

        // Read src
        table.pull(vrom_channel, [src_abs, src_val_packed]);

        // Read dst
        table.pull(vrom_channel, [dst_abs, dst_val]);

        Self {
            id: table.id(),
            state_cols,
            dst_abs,
            dst_val,
            src_abs,
            src_val,
            imm_32b,
            subber,
        }
    }
}

impl TableFiller<ProverPackedField> for SleiuTable {
    type Event = SleiuEvent;

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
            let mut dst_val = witness.get_scalars_mut(self.dst_val)?;
            let mut src_abs = witness.get_scalars_mut(self.src_abs)?;
            let mut src_val = witness.get_mut_as(self.src_val)?;
            let mut imm = witness.get_mut_as(self.imm_32b)?;

            for (i, event) in rows.clone().enumerate() {
                dst_abs[i] = B32::new(event.fp.addr(event.dst));
                dst_val[i] = B32::new(event.dst_val);
                src_abs[i] = B32::new(event.fp.addr(event.src));
                src_val[i] = event.src_val;
                imm[i] = event.imm;
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
        self.subber.populate(witness)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use petravm_asm::isa::GenericISA;
    use proptest::prelude::*;
    use proptest::prop_oneof;

    use crate::model::Trace;
    use crate::prover::Prover;
    use crate::test_utils::generate_trace;

    /// Creates an execution trace for a simple program that uses the SLEU or
    /// SLTU instructions.
    fn generate_unsigned_trace(src1_val: u32, src2_val: u32, op: &str) -> Result<Trace> {
        let asm_code = format!(
            "#[framesize(0x10)]\n\
             _start: 
                LDI.W @2, #{}\n\
                LDI.W @3, #{}\n\
                {} @4, @2, @3\n\
                RET\n",
            src1_val,
            src2_val,
            op.to_uppercase(),
        );

        // Calculate the expected result (1 if src1 < src2 (or src1 <= src2), 0
        // otherwise)
        let expected = match op {
            "sltu" => (src1_val < src2_val) as u32,
            "sleu" => (src1_val <= src2_val) as u32,
            _ => panic!("Unsupported operation"),
        };

        // Add VROM writes from LDI and comparison events
        let vrom_writes = vec![
            // LDI events
            (2, src1_val, 2),
            (3, src2_val, 2),
            // Initial values
            (0, 0, 1),
            (1, 0, 1),
            // Comparison event
            (4, expected, 1),
        ];

        generate_trace(asm_code, None, Some(vrom_writes))
    }

    fn test_unsigned_comparisons_with_values(src1_val: u32, src2_val: u32, op: &str) -> Result<()> {
        let trace = generate_unsigned_trace(src1_val, src2_val, op)?;
        trace.validate()?;

        match op {
            "sltu" => assert_eq!(trace.sltu_events().len(), 1),
            "sleu" => assert_eq!(trace.sleu_events().len(), 1),
            _ => panic!("Unsupported operation"),
        }

        assert_eq!(trace.ret_events().len(), 1);
        Prover::new(Box::new(GenericISA)).validate_witness(&trace)
    }

    /// Creates an execution trace for a simple program that uses the SLEIU or
    /// SLTIU instructions.
    fn generate_imm_unsigned_trace(src_val: u32, imm_val: u16, op: &str) -> Result<Trace> {
        let asm_code = format!(
            "#[framesize(0x10)]\n\
             _start: 
                LDI.W @2, #{}\n\
                {} @3, @2, #{}\n\
                RET\n",
            src_val,
            op.to_uppercase(),
            imm_val
        );

        // Calculate the expected result (1 if src < imm (or src <= imm), 0 otherwise)
        let expected = match op {
            "sltiu" => (src_val < imm_val as u32) as u32,
            "sleiu" => (src_val <= imm_val as u32) as u32,
            _ => panic!("Unsupported operation"),
        };

        // Add VROM writes from LDI and comparison events
        let vrom_writes = vec![
            // LDI event
            (2, src_val, 2),
            // Initial values
            (0, 0, 1),
            (1, 0, 1),
            // Comparison event
            (3, expected, 1),
        ];

        generate_trace(asm_code, None, Some(vrom_writes))
    }

    fn test_imm_unsigned_comparisons_with_values(
        src_val: u32,
        imm_val: u16,
        op: &str,
    ) -> Result<()> {
        let trace = generate_imm_unsigned_trace(src_val, imm_val, op)?;
        trace.validate()?;

        match op {
            "sltiu" => assert_eq!(trace.sltiu_events().len(), 1),
            "sleiu" => assert_eq!(trace.sleiu_events().len(), 1),
            _ => panic!("Unsupported operation"),
        }

        assert_eq!(trace.ret_events().len(), 1);
        Prover::new(Box::new(GenericISA)).validate_witness(&trace)
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(20))]

        #[test]
        fn test_sltu_operations(
            // Test both random values and specific edge cases
            (src1_val, src2_val) in prop_oneof![
                // Random value pairs
                (any::<u32>(), any::<u32>()),

                // Edge cases
                Just((0, 0)),                  // Equal at zero
                Just((1, 0)),                  // Greater than
                Just((0, 1)),                  // Less than
                Just((u32::MAX, u32::MAX)),    // Equal at max
                Just((0, u32::MAX)),           // Min < Max
                Just((u32::MAX, 0)),           // Max > Min

                // Additional interesting cases
                Just((u32::MAX/2, u32::MAX/2 + 1)),  // Middle values
                Just((1, u32::MAX)),                // 1 < MAX
                Just((u32::MAX - 1, u32::MAX))       // MAX-1 < MAX
            ],
        ) {
            prop_assert!(test_unsigned_comparisons_with_values(src1_val, src2_val, "sltu").is_ok());
        }

        #[test]
        fn test_sltiu_operations(
            // Test both random values and specific edge cases
            (src_val, imm_val) in prop_oneof![
                // Random value pairs
                (any::<u32>(), any::<u16>()),

                // Edge cases
                Just((0, 0)),                  // Equal at zero
                Just((1, 0)),                  // Greater than
                Just((0, 1)),                  // Less than
                Just((u32::MAX, u16::MAX)),    // Equal at max
                Just((1, u16::MAX)),           // 1 < MAX
                Just((0, u16::MAX)),           // Min < Max
                Just((u32::MAX, 0)),           // Max > Min
            ],
        ) {
            prop_assert!(test_imm_unsigned_comparisons_with_values(src_val, imm_val, "sltiu").is_ok());
        }

        #[test]
        fn test_sleu_operations(
            // Test both random values and specific edge cases
            (src1_val, src2_val) in prop_oneof![
                // Random value pairs
                (any::<u32>(), any::<u32>()),

                // Edge cases
                Just((0, 0)),                  // Equal at zero
                Just((1, 0)),                  // Greater than
                Just((0, 1)),                  // Less than
                Just((u32::MAX, u32::MAX)),    // Equal at max
                Just((0, u32::MAX)),           // Min < Max
                Just((u32::MAX, 0)),           // Max > Min

                // // Additional interesting cases
                Just((u32::MAX/2, u32::MAX/2 + 1)),  // Middle values
                Just((1, u32::MAX)),                // 1 < MAX
                Just((u32::MAX - 1, u32::MAX))       // MAX-1 < MAX
            ],
        ) {
            prop_assert!(test_unsigned_comparisons_with_values(src1_val, src2_val, "sleu").is_ok());
        }

        #[test]
        fn test_sleiu_operations(
            // Test both random values and specific edge cases
            (src_val, imm_val) in prop_oneof![
                // Random value pairs
                (any::<u32>(), any::<u16>()),

                // Edge cases
                Just((0, 0)),                  // Equal at zero
                Just((1, 0)),                  // Greater than
                Just((0, 1)),                  // Less than
                Just((u32::MAX, u16::MAX)),    // Equal at max
                Just((1, u16::MAX)),           // 1 < MAX
                Just((0, u16::MAX)),           // Min < Max
                Just((u32::MAX, 0)),           // Max > Min
            ],
        ) {
            prop_assert!(test_imm_unsigned_comparisons_with_values(src_val, imm_val, "sleiu").is_ok());
        }
    }
}
