//! Binary field operation tables for the PetraVM M3 circuit.
//!
//! This module contains tables for binary field arithmetic operations.

use std::any::Any;

use binius_field::underlier::Divisible;
use binius_m3::builder::{
    upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B128, B32,
};
use petravm_asm::{opcodes::Opcode, B128AddEvent, B128MulEvent};

use crate::{
    channels::Channels,
    gadgets::{
        multiple_lookup::{MultipleLookupColumns, MultipleLookupGadget},
        state::{NextPc, StateColumns, StateColumnsOptions, StateGadget},
    },
    table::Table,
    types::ProverPackedField,
};

// Constants for opcodes
const B128_ADD_OPCODE: u16 = Opcode::B128Add as u16;
const B128_MUL_OPCODE: u16 = Opcode::B128Mul as u16;

/// Expands to a `TableFiller<ProverPackedField>` impl for a given B128
/// instruction table.
macro_rules! impl_b128_table_filler {
    ($table_ty:ident, $event_ty:ident) => {
        impl TableFiller<ProverPackedField> for $table_ty {
            type Event = $event_ty;

            fn id(&self) -> TableId {
                self.id
            }

            fn fill<'a>(
                &'a self,
                rows: impl Iterator<Item = &'a Self::Event> + Clone,
                witness: &'a mut TableWitnessSegment<ProverPackedField>,
            ) -> anyhow::Result<()> {
                {
                    let mut src1_val_col_unpacked = witness.get_mut_as(self.src1_val_unpacked)?;
                    let mut src2_val_col_unpacked = witness.get_mut_as(self.src2_val_unpacked)?;
                    let mut result_val_col_unpacked =
                        witness.get_mut_as(self.result_val_unpacked)?;
                    let mut src1_abs_addr_col = witness.get_scalars_mut(self.src1_abs_addr)?;
                    let mut src2_abs_addr_col = witness.get_scalars_mut(self.src2_abs_addr)?;
                    let mut dst_abs_addr_col = witness.get_scalars_mut(self.dst_abs_addr)?;

                    for (i, event) in rows.clone().enumerate() {
                        src1_val_col_unpacked[i] = B128::new(event.src1_val);
                        src2_val_col_unpacked[i] = B128::new(event.src2_val);
                        result_val_col_unpacked[i] = B128::new(event.dst_val);
                        src1_abs_addr_col[i] = B32::new(event.fp.addr(event.src1));
                        src2_abs_addr_col[i] = B32::new(event.fp.addr(event.src2));
                        dst_abs_addr_col[i] = B32::new(event.fp.addr(event.dst));
                    }
                }

                let state_iter = rows.clone().map(|ev| StateGadget {
                    pc: ev.pc.val(),
                    next_pc: None,
                    fp: *ev.fp,
                    arg0: ev.dst,
                    arg1: ev.src1,
                    arg2: ev.src2,
                });
                self.state_cols.populate(witness, state_iter)?;

                let src1_iter = rows.clone().map(|ev| {
                    let vals: [u32; 4] = <u128 as Divisible<u32>>::split_val(ev.src1_val);
                    MultipleLookupGadget {
                        addr: ev.fp.addr(ev.src1),
                        vals,
                    }
                });
                self.src1_lookup.populate(witness, src1_iter)?;

                let src2_iter = rows.clone().map(|ev| {
                    let vals: [u32; 4] = <u128 as Divisible<u32>>::split_val(ev.src2_val);
                    MultipleLookupGadget {
                        addr: ev.fp.addr(ev.src2),
                        vals,
                    }
                });
                self.src2_lookup.populate(witness, src2_iter)?;

                let result_iter = rows.map(|ev| {
                    let vals: [u32; 4] = <u128 as Divisible<u32>>::split_val(ev.dst_val);
                    MultipleLookupGadget {
                        addr: ev.fp.addr(ev.dst),
                        vals,
                    }
                });
                self.result_lookup.populate(witness, result_iter)
            }
        }
    };
}

/// B128_ADD (Binary Field Addition) table.
///
/// This table handles the B128_ADD instruction, which performs addition
/// in the binary field GF(2^128).
pub struct B128AddTable {
    /// Table ID
    pub id: TableId,
    /// State columns
    state_cols: StateColumns<{ B128_ADD_OPCODE }>,
    /// First source value
    pub src1_val_unpacked: Col<B32, 4>,
    /// Lookup for first source
    src1_lookup: MultipleLookupColumns<4>,
    /// Second source value
    pub src2_val_unpacked: Col<B32, 4>,
    /// Lookup for second source
    src2_lookup: MultipleLookupColumns<4>,
    /// Result value
    pub result_val_unpacked: Col<B32, 4>, // Virtual
    /// Lookup for result
    result_lookup: MultipleLookupColumns<4>,
    /// First source absolute address
    pub src1_abs_addr: Col<B32>,
    /// Second source absolute address
    pub src2_abs_addr: Col<B32>,
    /// Destination absolute address
    pub dst_abs_addr: Col<B32>,
}

impl Table for B128AddTable {
    type Event = B128AddEvent;

    fn name(&self) -> &'static str {
        "B128AddTable"
    }

    /// Create a new B128_ADD table with the given constraint system and
    /// channels.
    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("b128_add");

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
            arg1: src1,
            arg2: src2,
            ..
        } = state_cols;

        let src1_val_unpacked = table.add_committed("b128_add_src1_val_unpacked");
        let src2_val_unpacked = table.add_committed("b128_add_src2_val_unpacked");
        let result_val_unpacked = table.add_computed(
            "b128_add_result_val_unpacked",
            src1_val_unpacked + src2_val_unpacked,
        );

        // Pull source values from VROM channel
        let src1_abs_addr = table.add_computed("src1_addr", fp + upcast_expr(src1.into()));
        let src1_lookup = MultipleLookupColumns::new(
            &mut table,
            channels.vrom_channel,
            src1_abs_addr,
            src1_val_unpacked,
            "b128_add_src1",
        );
        let src2_abs_addr = table.add_computed("src2_addr", fp + upcast_expr(src2.into()));
        let src2_lookup = MultipleLookupColumns::new(
            &mut table,
            channels.vrom_channel,
            src2_abs_addr,
            src2_val_unpacked,
            "b128_add_src2",
        );

        // Pull result from VROM channel
        let dst_abs_addr = table.add_computed("dst_addr", fp + upcast_expr(dst.into()));
        let result_lookup = MultipleLookupColumns::new(
            &mut table,
            channels.vrom_channel,
            dst_abs_addr,
            result_val_unpacked,
            "b128_add_dst",
        );

        Self {
            id: table.id(),
            state_cols,
            src1_val_unpacked,
            src1_lookup,
            src2_val_unpacked,
            src2_lookup,
            result_val_unpacked,
            result_lookup,
            src1_abs_addr,
            src2_abs_addr,
            dst_abs_addr,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl_b128_table_filler!(B128AddTable, B128AddEvent);

/// B128_MUL (Binary Field Multiplication) table.
///
/// This table handles the B128_MUL instruction, which performs multiplication
/// in the binary field GF(2^128).
pub struct B128MulTable {
    /// Table ID
    pub id: TableId,
    /// State columns
    state_cols: StateColumns<{ B128_MUL_OPCODE }>,
    /// First source value
    pub src1_val: Col<B128>,
    pub src1_val_unpacked: Col<B32, 4>,
    /// Lookup for first source
    src1_lookup: MultipleLookupColumns<4>,
    /// Second source value
    pub src2_val: Col<B128>,
    pub src2_val_unpacked: Col<B32, 4>,
    /// Lookup for second source
    src2_lookup: MultipleLookupColumns<4>,
    /// Result value
    pub result_val: Col<B128>,
    pub result_val_unpacked: Col<B32, 4>,
    /// Lookup for result
    result_lookup: MultipleLookupColumns<4>,
    /// First source absolute address
    pub src1_abs_addr: Col<B32>,
    /// Second source absolute address
    pub src2_abs_addr: Col<B32>,
    /// Destination absolute address
    pub dst_abs_addr: Col<B32>,
}

impl Table for B128MulTable {
    type Event = B128MulEvent;

    fn name(&self) -> &'static str {
        "B128MulTable"
    }

    /// Create a new B128_MUL table with the given constraint system and
    /// channels.
    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("b128_mul");

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
            arg1: src1,
            arg2: src2,
            ..
        } = state_cols;

        let src1_val_unpacked = table.add_committed("b128_mul_src1_val_unpacked");
        let src1_val = table.add_packed("b128_mul_src1_val", src1_val_unpacked);
        let src2_val_unpacked = table.add_committed("b128_mul_src2_val_unpacked");
        let src2_val = table.add_packed("b128_mul_src2_val", src2_val_unpacked);
        let result_val_unpacked = table.add_committed("b128_mul_result_val_unpacked");
        let result_val = table.add_packed("b128_mul_result_val", result_val_unpacked);

        // Pull source values from VROM channel
        let src1_abs_addr = table.add_computed("src1_addr", fp + upcast_expr(src1.into()));
        let src1_lookup = MultipleLookupColumns::new(
            &mut table,
            channels.vrom_channel,
            src1_abs_addr,
            src1_val_unpacked,
            "b128_mul_src1",
        );

        let src2_abs_addr = table.add_computed("src2_addr", fp + upcast_expr(src2.into()));
        let src2_lookup = MultipleLookupColumns::new(
            &mut table,
            channels.vrom_channel,
            src2_abs_addr,
            src2_val_unpacked,
            "b128_mul_src2",
        );

        table.assert_zero("check_b128_mul_result", src1_val * src2_val - result_val);

        // Pull result from VROM channel
        let dst_abs_addr = table.add_computed("dst_addr", fp + upcast_expr(dst.into()));
        let result_lookup = MultipleLookupColumns::new(
            &mut table,
            channels.vrom_channel,
            dst_abs_addr,
            result_val_unpacked,
            "b128_mul_dst",
        );

        Self {
            id: table.id(),
            state_cols,
            src1_val,
            src1_val_unpacked,
            src1_lookup,
            src2_val,
            src2_val_unpacked,
            src2_lookup,
            result_val,
            result_val_unpacked,
            result_lookup,
            src1_abs_addr,
            src2_abs_addr,
            dst_abs_addr,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl_b128_table_filler!(B128MulTable, B128MulEvent);
