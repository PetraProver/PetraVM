use std::{any::Any, ops::Deref};

use binius_m3::{
    builder::{
        upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1, B32,
    },
    gadgets::u32::{U32Sub, U32SubFlags},
};
use petravm_assembly::{opcodes::Opcode, SltuEvent};

use crate::{
    channels::Channels,
    gadgets::state::{NextPc, StateColumns, StateColumnsOptions, StateGadget},
    table::Table,
    types::ProverPackedField,
};

const SLTU_OPCODE: u16 = Opcode::Sltu as u16;

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

    fn as_any(&self) -> &dyn Any {
        self
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
            fp: *event.fp.deref(),
            arg0: event.dst,
            arg1: event.src1,
            arg2: event.src2,
        });
        self.state_cols.populate(witness, state_rows)?;
        self.subber.populate(witness)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use petravm_assembly::isa::GenericISA;
    use proptest::prelude::*;
    use proptest::prop_oneof;

    use crate::model::Trace;
    use crate::prover::Prover;
    use crate::test_utils::generate_trace;

    /// Creates an execution trace for a simple program that uses the SLTU
    /// instruction.
    fn generate_sltu_trace(src1_val: u32, src2_val: u32) -> Result<Trace> {
        let asm_code = format!(
            "#[framesize(0x10)]\n\
             _start: 
                LDI.W @2, #{}\n\
                LDI.W @3, #{}\n\
                SLTU @4, @2, @3\n\
                RET\n",
            src1_val, src2_val
        );

        // Calculate the expected result (1 if src1 < src2, 0 otherwise)
        let expected = (src1_val < src2_val) as u32;

        // Add VROM writes from LDI and SLTU events
        let vrom_writes = vec![
            // LDI events
            (2, src1_val, 2),
            (3, src2_val, 2),
            // Initial values
            (0, 0, 1),
            (1, 0, 1),
            // SLTU event
            (4, expected, 1),
        ];

        generate_trace(asm_code, None, Some(vrom_writes))
    }

    fn test_sltu_with_values(src1_val: u32, src2_val: u32) -> Result<()> {
        let trace = generate_sltu_trace(src1_val, src2_val)?;
        trace.validate()?;
        assert_eq!(trace.sltu_events().len(), 1);
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
            prop_assert!(test_sltu_with_values(src1_val, src2_val).is_ok());
        }
    }
}
