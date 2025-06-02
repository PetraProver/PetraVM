//! Binary field operation tables for the PetraVM M3 circuit.
//!
//! This module contains tables for binary field arithmetic operations.

use binius_field::Field;
use binius_m3::builder::{
    upcast_col, upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1,
    B128, B16, B32,
};
use petravm_asm::{
    opcodes::Opcode, AndEvent, AndiEvent, B32MulEvent, B32MuliEvent, OrEvent, OriEvent, XorEvent,
    XoriEvent,
};

use crate::{
    channels::Channels,
    gadgets::state::{NextPc, StateColumns, StateColumnsOptions, StateGadget},
    table::Table,
    types::ProverPackedField,
    utils::{pack_b16_into_b32, pack_instruction_one_arg},
};
use crate::{opcodes::G, utils::pack_instruction_with_32bits_imm_b128};

// Constants for opcodes
const B32_MUL_OPCODE: u16 = Opcode::B32Mul as u16;
const B32_MULI_OPCODE: u16 = Opcode::B32Muli as u16;
const XOR_OPCODE: u16 = Opcode::Xor as u16;
const XORI_OPCODE: u16 = Opcode::Xori as u16;
const AND_OPCODE: u16 = Opcode::And as u16;
const ANDI_OPCODE: u16 = Opcode::Andi as u16;
const OR_OPCODE: u16 = Opcode::Or as u16;
const ORI_OPCODE: u16 = Opcode::Ori as u16;

/// Expands to a `TableFiller<ProverPackedField>` impl for a given B32
/// instruction table.
macro_rules! impl_b32_table_filler {
    ($table_ty:ident, $event_ty:ident) => {
        impl TableFiller<ProverPackedField> for $table_ty {
            type Event = $event_ty;

            fn id(&self) -> TableId {
                self.id
            }

            fn fill<'a>(
                &self,
                rows: impl Iterator<Item = &'a Self::Event> + Clone,
                witness: &'a mut TableWitnessSegment<ProverPackedField>,
            ) -> Result<(), anyhow::Error> {
                {
                    let mut dst_abs_addr = witness.get_scalars_mut(self.dst_abs_addr)?;
                    let mut dst_val = witness.get_scalars_mut(self.dst_val)?;
                    let mut src1_abs_addr = witness.get_scalars_mut(self.src1_abs_addr)?;
                    let mut src1_val = witness.get_scalars_mut(self.src1_val)?;
                    let mut src2_abs_addr = witness.get_scalars_mut(self.src2_abs_addr)?;
                    let mut src2_val = witness.get_scalars_mut(self.src2_val)?;

                    for (i, event) in rows.clone().enumerate() {
                        dst_abs_addr[i] = B32::new(event.fp.addr(event.dst));
                        dst_val[i] = B32::new(event.dst_val);
                        src1_abs_addr[i] = B32::new(event.fp.addr(event.src1));
                        src1_val[i] = B32::new(event.src1_val);
                        src2_abs_addr[i] = B32::new(event.fp.addr(event.src2));
                        src2_val[i] = B32::new(event.src2_val);
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
                self.state_cols.populate(witness, state_rows)
            }
        }
    };
}

/// B32_MUL (Binary Field Multiplication) table.
///
/// This table handles the B32_MUL instruction, which performs multiplication
/// in the binary field GF(2^32).
pub struct B32MulTable {
    /// Table ID
    pub id: TableId,
    /// State columns
    state_cols: StateColumns<{ B32_MUL_OPCODE }>,
    /// First source value
    pub src1_val: Col<B32>,
    /// Second source value
    pub src2_val: Col<B32>,
    /// Result value
    pub dst_val: Col<B32>, // Virtual
    /// PROM channel pull value
    pub src1_abs_addr: Col<B32>,
    /// Second source absolute address
    pub src2_abs_addr: Col<B32>,
    /// Destination absolute address
    pub dst_abs_addr: Col<B32>,
}

impl Table for B32MulTable {
    type Event = B32MulEvent;

    fn name(&self) -> &'static str {
        "B32MulTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("b32_mul");

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

        let src1_val = table.add_committed("b32_mul_src1_val");
        let src2_val = table.add_committed("b32_mul_src2_val");

        // Pull source values from VROM channel
        let src1_abs_addr = table.add_computed("src1_addr", fp + upcast_expr(src1.into()));
        let src2_abs_addr = table.add_computed("src2_addr", fp + upcast_expr(src2.into()));
        table.pull(channels.vrom_channel, [src1_abs_addr, src1_val]);
        table.pull(channels.vrom_channel, [src2_abs_addr, src2_val]);

        // Compute the result
        let dst_val = table.add_committed("b32_mul_dst_val");
        table.assert_zero("b32_mul_dst_val", dst_val - src1_val * src2_val);

        // Pull result from VROM channel
        let dst_abs_addr = table.add_computed("dst_addr", fp + upcast_expr(dst.into()));
        table.pull(channels.vrom_channel, [dst_abs_addr, dst_val]);

        Self {
            id: table.id(),
            state_cols,
            src1_val,
            src2_val,
            dst_val,
            src1_abs_addr,
            src2_abs_addr,
            dst_abs_addr,
        }
    }
}

impl_b32_table_filler!(B32MulTable, B32MulEvent);

pub struct XorTable {
    /// Table ID
    id: TableId,
    /// State columns
    state_cols: StateColumns<XOR_OPCODE>,
    /// First source value
    pub src1_val: Col<B32>,
    /// Second source value
    pub src2_val: Col<B32>,
    /// Result value
    pub dst_val: Col<B32>,
    /// PROM channel pull value
    pub src1_abs_addr: Col<B32>,
    /// Second source absolute address
    pub src2_abs_addr: Col<B32>,
    /// Destination absolute address
    pub dst_abs_addr: Col<B32>,
}

impl Table for XorTable {
    type Event = XorEvent;

    fn name(&self) -> &'static str {
        "XorTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("xor");
        let src1_val = table.add_committed("src1_val");
        let src2_val = table.add_committed("src2_val");

        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions::default(),
        );
        let dst_abs_addr =
            table.add_computed("dst_abs_addr", state_cols.fp + upcast_col(state_cols.arg0));
        let src1_abs_addr =
            table.add_computed("src1_abs_addr", state_cols.fp + upcast_col(state_cols.arg1));
        let src2_abs_addr =
            table.add_computed("src2_abs_addr", state_cols.fp + upcast_col(state_cols.arg2));

        let dst_val = table.add_computed("dst_val", src1_val + src2_val);

        // Read src1_val and src2_val
        table.pull(channels.vrom_channel, [src1_abs_addr, src1_val]);
        table.pull(channels.vrom_channel, [src2_abs_addr, src2_val]);

        // Read dst_val
        table.pull(channels.vrom_channel, [dst_abs_addr, dst_val]);

        Self {
            id: table.id(),
            state_cols,
            src1_abs_addr,
            src1_val,
            src2_abs_addr,
            src2_val,
            dst_abs_addr,
            dst_val,
        }
    }
}

impl_b32_table_filler!(XorTable, XorEvent);

pub struct AndTable {
    /// Table ID
    id: TableId,
    /// State columns
    state_cols: StateColumns<AND_OPCODE>,
    /// First source value
    pub src1_val: Col<B32>,
    /// Second source value
    pub src2_val: Col<B32>,
    /// Result value
    pub dst_val: Col<B32>,
    /// PROM channel pull value
    pub src1_abs_addr: Col<B32>,
    /// Second source absolute address
    pub src2_abs_addr: Col<B32>,
    /// Destination absolute address
    pub dst_abs_addr: Col<B32>,
}

impl Table for AndTable {
    type Event = AndEvent;

    fn name(&self) -> &'static str {
        "AndTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("and");
        let src1_val_unpacked: Col<B1, 32> = table.add_committed("src1_val");
        let src1_val = table.add_packed("src1_val", src1_val_unpacked);
        let src2_val_unpacked: Col<B1, 32> = table.add_committed("src2_val");
        let src2_val = table.add_packed("src2_val", src2_val_unpacked);

        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions::default(),
        );

        let dst_abs_addr =
            table.add_computed("dst_abs_addr", state_cols.fp + upcast_col(state_cols.arg0));
        let src1_abs_addr =
            table.add_computed("src1_abs_addr", state_cols.fp + upcast_col(state_cols.arg1));
        let src2_abs_addr =
            table.add_computed("src2_abs_addr", state_cols.fp + upcast_col(state_cols.arg2));

        let dst_val_unpacked = table.add_committed("dst_val_unpacked");
        table.assert_zero(
            "and_dst_val_unpacked",
            dst_val_unpacked - src1_val_unpacked * src2_val_unpacked,
        );
        let dst_val = table.add_packed("dst_val", dst_val_unpacked);

        // Read src1_val and src2_val
        table.pull(channels.vrom_channel, [src1_abs_addr, src1_val]);
        table.pull(channels.vrom_channel, [src2_abs_addr, src2_val]);

        // Read dst_val
        table.pull(channels.vrom_channel, [dst_abs_addr, dst_val]);

        Self {
            id: table.id(),
            state_cols,
            src1_abs_addr,
            src1_val,
            src2_abs_addr,
            src2_val,
            dst_abs_addr,
            dst_val,
        }
    }
}

impl_b32_table_filler!(AndTable, AndEvent);

pub struct OrTable {
    /// Table ID
    id: TableId,
    /// State columns
    state_cols: StateColumns<OR_OPCODE>,
    /// First source value
    pub src1_val: Col<B32>,
    /// Second source value
    pub src2_val: Col<B32>,
    /// Result value
    pub dst_val: Col<B32>,
    /// PROM channel pull value
    pub src1_abs_addr: Col<B32>,
    /// Second source absolute address
    pub src2_abs_addr: Col<B32>,
    /// Destination absolute address
    pub dst_abs_addr: Col<B32>,
}

impl Table for OrTable {
    type Event = OrEvent;

    fn name(&self) -> &'static str {
        "OrTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("or");
        let src1_val_unpacked: Col<B1, 32> = table.add_committed("src1_val");
        let src1_val = table.add_packed("src1_val", src1_val_unpacked);
        let src2_val_unpacked: Col<B1, 32> = table.add_committed("src2_val");
        let src2_val = table.add_packed("src2_val", src2_val_unpacked);

        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions::default(),
        );

        let dst_abs_addr =
            table.add_computed("dst_abs_addr", state_cols.fp + upcast_col(state_cols.arg0));
        let src1_abs_addr =
            table.add_computed("src1_abs_addr", state_cols.fp + upcast_col(state_cols.arg1));
        let src2_abs_addr =
            table.add_computed("src2_abs_addr", state_cols.fp + upcast_col(state_cols.arg2));

        let dst_val_unpacked = table.add_committed("dst_val_unpacked");
        table.assert_zero(
            "or_dst_val_unpacked",
            // DeMorgan Law: a | b == a + b + (a * b)
            dst_val_unpacked
                - src1_val_unpacked
                - src2_val_unpacked
                - (src1_val_unpacked * src2_val_unpacked),
        );
        let dst_val = table.add_packed("dst_val", dst_val_unpacked);

        // Read src1_val and src2_val
        table.pull(channels.vrom_channel, [src1_abs_addr, src1_val]);
        table.pull(channels.vrom_channel, [src2_abs_addr, src2_val]);

        // Read dst_val
        table.pull(channels.vrom_channel, [dst_abs_addr, dst_val]);

        Self {
            id: table.id(),
            state_cols,
            src1_abs_addr,
            src1_val,
            src2_abs_addr,
            src2_val,
            dst_abs_addr,
            dst_val,
        }
    }
}

impl_b32_table_filler!(OrTable, OrEvent);

pub struct OriTable {
    /// Table ID
    id: TableId,
    /// State columns
    state_cols: StateColumns<ORI_OPCODE>,
    /// Source value
    pub src_val: Col<B32>,
    /// Source value, unpacked
    src_val_unpacked: Col<B1, 32>,
    /// Immediate value, unpacked
    imm_32b_unpacked: Col<B1, 32>,
    /// Result value
    pub dst_val: Col<B32>,
    /// Result value, unpacked
    dst_val_unpacked: Col<B1, 32>,
    /// PROM channel pull value
    pub src_abs_addr: Col<B32>,
    /// Destination absolute address
    pub dst_abs_addr: Col<B32>,
}

impl Table for OriTable {
    type Event = OriEvent;

    fn name(&self) -> &'static str {
        "OriTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("ori");
        let src_val_unpacked: Col<B1, 32> = table.add_committed("src_val");
        let src_val = table.add_packed("src_val", src_val_unpacked);

        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions::default(),
        );
        let imm_32b_unpacked = table.add_zero_pad("imm_32b", state_cols.arg2_unpacked, 0);

        let dst_abs_addr =
            table.add_computed("dst_abs_addr", state_cols.fp + upcast_col(state_cols.arg0));
        let src_abs_addr =
            table.add_computed("src_abs_addr", state_cols.fp + upcast_col(state_cols.arg1));

        let dst_val_unpacked = table.add_committed("dst_val_unpacked");
        table.assert_zero(
            "ori_dst_val_unpacked",
            dst_val_unpacked
                - src_val_unpacked
                - imm_32b_unpacked
                - (src_val_unpacked * imm_32b_unpacked),
        );
        let dst_val = table.add_packed("dst_val", dst_val_unpacked);

        // Read src_val
        table.pull(channels.vrom_channel, [src_abs_addr, src_val]);

        // Read dst_val
        table.pull(channels.vrom_channel, [dst_abs_addr, dst_val]);

        Self {
            id: table.id(),
            state_cols,
            src_abs_addr,
            src_val,
            src_val_unpacked,
            imm_32b_unpacked,
            dst_abs_addr,
            dst_val,
            dst_val_unpacked,
        }
    }
}

impl TableFiller<ProverPackedField> for OriTable {
    type Event = OriEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut dst_abs_addr = witness.get_scalars_mut(self.dst_abs_addr)?;
            let mut dst_val_unpacked = witness.get_mut_as(self.dst_val_unpacked)?;
            let mut src_abs_addr = witness.get_scalars_mut(self.src_abs_addr)?;
            let mut src_val_unpacked = witness.get_mut_as(self.src_val_unpacked)?;
            let mut imm_32b_unpacked = witness.get_mut_as(self.imm_32b_unpacked)?;

            for (i, event) in rows.clone().enumerate() {
                dst_abs_addr[i] = B32::new(event.fp.addr(event.dst));
                dst_val_unpacked[i] = event.dst_val;
                src_abs_addr[i] = B32::new(event.fp.addr(event.src));
                src_val_unpacked[i] = event.src_val;
                imm_32b_unpacked[i] = event.imm as u32;
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
        self.state_cols.populate(witness, state_rows)
    }
}

pub struct XoriTable {
    id: TableId,
    state_cols: StateColumns<XORI_OPCODE>,
    dst_abs: Col<B32>, // Virtual
    dst_val: Col<B32>, // Virtual
    src_abs: Col<B32>, // Virtual
    src_val: Col<B32>,
}

impl Table for XoriTable {
    type Event = XoriEvent;

    fn name(&self) -> &'static str {
        "XoriTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("xori");
        let src_val = table.add_committed("src_val");

        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions::default(),
        );
        let dst_abs = table.add_computed("dst_abs", state_cols.fp + upcast_col(state_cols.arg0));
        let src_abs = table.add_computed("src_abs", state_cols.fp + upcast_col(state_cols.arg1));
        let imm = state_cols.arg2;

        let dst_val = table.add_computed("dst_val", src_val + upcast_expr(imm.into()));

        // Read dst_val
        table.pull(channels.vrom_channel, [dst_abs, dst_val]);

        // Read src_val
        table.pull(channels.vrom_channel, [src_abs, src_val]);

        Self {
            id: table.id(),
            state_cols,
            dst_abs,
            dst_val,
            src_abs,
            src_val,
        }
    }
}

impl TableFiller<ProverPackedField> for XoriTable {
    type Event = XoriEvent;

    fn id(&self) -> TableId {
        self.id
    }

    // TODO: This implementation might be very similar for all immediate binary
    // operations
    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut dst_abs = witness.get_scalars_mut(self.dst_abs)?;
            let mut dst_val = witness.get_scalars_mut(self.dst_val)?;
            let mut src_abs = witness.get_scalars_mut(self.src_abs)?;
            let mut src_val = witness.get_scalars_mut(self.src_val)?;
            for (i, event) in rows.clone().enumerate() {
                dst_abs[i] = B32::new(event.fp.addr(event.dst));
                dst_val[i] = B32::new(event.dst_val);
                src_abs[i] = B32::new(event.fp.addr(event.src));
                src_val[i] = B32::new(event.src_val);
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
        self.state_cols.populate(witness, state_rows)
    }
}

pub struct AndiTable {
    id: TableId,
    state_cols: StateColumns<ANDI_OPCODE>,
    dst_abs: Col<B32>,             // Virtual
    src_abs: Col<B32>,             // Virtual
    dst_val_unpacked: Col<B1, 16>, // Virtual
    src_val_unpacked: Col<B1, 32>,
    /// The lower 16 bits of src_val.
    src_val_low: Col<B1, 16>,
}

impl Table for AndiTable {
    type Event = AndiEvent;

    fn name(&self) -> &'static str {
        "AndiTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("andi");
        let src_val_unpacked: Col<B1, 32> = table.add_committed("src_val");
        let src_val = table.add_packed("src_val", src_val_unpacked);

        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions::default(),
        );

        let dst_abs = table.add_computed("dst_abs", state_cols.fp + upcast_col(state_cols.arg0));
        let src_abs = table.add_computed("src_abs", state_cols.fp + upcast_col(state_cols.arg1));
        let imm = state_cols.arg2_unpacked;

        let src_val_low: Col<B1, 16> = table.add_selected_block("src_val_low", src_val_unpacked, 0);

        let dst_val_unpacked: Col<B1, 16> = table.add_committed("dst_val_unpacked");
        table.assert_zero(
            "andi_dst_val_unpacked",
            dst_val_unpacked - src_val_low * imm,
        );
        let dst_val: Col<B16> = table.add_packed("dst_val", dst_val_unpacked);

        // Read dst_val
        table.pull(channels.vrom_channel, [dst_abs, upcast_col(dst_val)]);

        // Read src_val
        table.pull(channels.vrom_channel, [src_abs, src_val]);

        Self {
            id: table.id(),
            state_cols,
            dst_abs,
            src_abs,
            dst_val_unpacked,
            src_val_unpacked,
            src_val_low,
        }
    }
}

impl TableFiller<ProverPackedField> for AndiTable {
    type Event = AndiEvent;

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
            let mut dst_val_unpacked = witness.get_mut_as(self.dst_val_unpacked)?;
            let mut src_abs = witness.get_scalars_mut(self.src_abs)?;
            let mut src_val_unpacked = witness.get_mut_as(self.src_val_unpacked)?;
            let mut src_val_low = witness.get_mut_as(self.src_val_low)?;
            for (i, event) in rows.clone().enumerate() {
                dst_abs[i] = B32::new(event.fp.addr(event.dst));
                dst_val_unpacked[i] = event.dst_val as u16;
                src_abs[i] = B32::new(event.fp.addr(event.src));
                src_val_unpacked[i] = event.src_val;
                src_val_low[i] = B16::new(event.src_val as u16);
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
        self.state_cols.populate(witness, state_rows)
    }
}

/// B32_MULI (Binary Field Multiplication with Immediate) table.
///
/// This table handles the B32_MULI instruction, which performs multiplication
/// in the binary field GF(2^32) with a 32-bit immediate value.
/// This operation is special as it spans two instructions, with the immediate
/// split across them.
pub struct B32MuliTable {
    /// Table ID
    pub id: TableId,
    /// State columns for first instruction
    state_cols: StateColumns<B32_MULI_OPCODE>,
    /// Source value
    pub src_val: Col<B32>,
    /// Immediate value (32-bit constructed from two 16-bit values)
    pub imm_val: Col<B32>,
    /// Result value
    pub dst_val: Col<B32>,
    /// Source absolute address
    pub src_abs_addr: Col<B32>,
    /// Destination absolute address
    pub dst_abs_addr: Col<B32>,
    /// Second instruction packed
    pub second_instruction_packed: Col<B128>,
    /// Second instruction PC
    pub second_instruction_pc: Col<B32>,
    /// Second instruction arg0
    pub imm_high: Col<B16>,
}

impl Table for B32MuliTable {
    type Event = B32MuliEvent;

    fn name(&self) -> &'static str {
        "B32MuliTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("b32_muli");
        let next_pc = table.add_committed("next_pc");

        // First instruction - captures the initial opcode, dst, src, and imm_low
        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Target(next_pc),
                next_fp: None,
            },
        );

        let StateColumns {
            pc,
            fp,
            arg0: dst,
            arg1: src,
            arg2: imm_low,
            ..
        } = state_cols;

        // Checks that the next PC is PC * G * G
        let second_instruction_pc = table.add_computed("second_instruction_pc", pc * G);
        table.assert_zero("next_pc_check", next_pc - second_instruction_pc * G);

        // Create columns for values
        let src_val = table.add_committed("b32_muli_src_val");

        // Construct the 32-bit immediate from the two 16-bit parts
        let imm_high = table.add_committed("imm_high_col");
        let imm_val = table.add_computed("b32_muli_imm_val", pack_b16_into_b32(imm_low, imm_high));

        // Pull source value from VROM channel
        let src_abs_addr = table.add_computed("src_addr", fp + upcast_expr(src.into()));
        table.pull(channels.vrom_channel, [src_abs_addr, src_val]);

        // Compute the result
        let dst_val = table.add_committed("b32_muli_dst_val");
        table.assert_zero("b32_muli_dst_val", dst_val - src_val * imm_val);

        // Pull result from VROM channel
        let dst_abs_addr = table.add_computed("dst_addr", fp + upcast_expr(dst.into()));
        table.pull(channels.vrom_channel, [dst_abs_addr, dst_val]);

        // Pack the second instruction
        let second_instruction_packed = pack_instruction_one_arg(
            &mut table,
            "second_instruction_packed",
            second_instruction_pc,
            Opcode::B32Muli as u16,
            imm_high,
        );
        table.pull(channels.prom_channel, [second_instruction_packed]);

        Self {
            id: table.id(),
            state_cols,
            src_val,
            imm_val,
            dst_val,
            src_abs_addr,
            dst_abs_addr,
            second_instruction_packed,
            second_instruction_pc,
            imm_high,
        }
    }
}

impl TableFiller<ProverPackedField> for B32MuliTable {
    type Event = B32MuliEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            let mut src_val_col = witness.get_scalars_mut(self.src_val)?;
            let mut imm_val_col = witness.get_scalars_mut(self.imm_val)?;
            let mut dst_val_col = witness.get_scalars_mut(self.dst_val)?;
            let mut src_abs_addr_col = witness.get_scalars_mut(self.src_abs_addr)?;
            let mut dst_abs_addr_col = witness.get_scalars_mut(self.dst_abs_addr)?;
            let mut second_instruction_pc_col =
                witness.get_scalars_mut(self.second_instruction_pc)?;
            let mut imm_high_col = witness.get_scalars_mut(self.imm_high)?;
            let mut second_instruction_packed_col =
                witness.get_scalars_mut(self.second_instruction_packed)?;

            for (i, event) in rows.clone().enumerate() {
                src_val_col[i] = B32::new(event.src_val);
                imm_val_col[i] = B32::new(event.imm);
                dst_val_col[i] = B32::new(event.dst_val);
                src_abs_addr_col[i] = B32::new(event.fp.addr(event.src));
                dst_abs_addr_col[i] = B32::new(event.fp.addr(event.dst));
                second_instruction_pc_col[i] = event.pc * G;
                imm_high_col[i] = B16::new((event.imm >> 16) as u16);
                second_instruction_packed_col[i] = pack_instruction_with_32bits_imm_b128(
                    second_instruction_pc_col[i],
                    B16::new(Opcode::B32Muli as u16),
                    imm_high_col[i],
                    B32::ZERO,
                );
            }
        }

        // Populate the first instruction State rows
        let state_rows = rows.clone().map(|event| StateGadget {
            pc: event.pc.val(),
            next_pc: Some((event.pc * G * G).val()),
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.src,
            arg2: event.imm as u16, // imm_low
        });

        self.state_cols.populate(witness, state_rows)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use petravm_asm::isa::GenericISA;
    use proptest::prelude::*;

    use crate::model::Trace;
    use crate::prover::Prover;
    use crate::test_utils::generate_trace;

    /// Creates an execution trace for a simple program that uses various binary
    /// field operations to test binary operations.
    fn generate_binary_ops_trace(val1: u32, val2: u32) -> Result<Trace> {
        let imm16 = val2 as u16;
        let asm_code = format!(
            "#[framesize(0x20)]\n\
            _start:\n\
            LDI.W @2, #{val1}\n\
            LDI.W @3, #{val2}\n\
            B32_MUL @4, @2, @3\n\
            XOR @5, @2, @3\n\
            XORI @6, @2, #{imm16}\n\
            AND @7, @2, @3\n\
            ANDI @8, @2, #{imm16}\n\
            OR @9, @2, @3\n\
            ORI @10, @2, #{imm16}\n\
            B32_MULI @11, @2, #{val2}\n\
            ;; repeat to test witness filling
            B32_MUL @4, @2, @3\n\
            XOR @5, @2, @3\n\
            XORI @6, @2, #{imm16}\n\
            AND @7, @2, @3\n\
            ANDI @8, @2, #{imm16}\n\
            OR @9, @2, @3\n\
            ORI @10, @2, #{imm16}\n\
            B32_MULI @11, @2, #{val2}\n\
            RET\n"
        );

        generate_trace(asm_code, None, None)
    }

    fn test_binary_ops_with_values(val1: u32, val2: u32) -> Result<()> {
        let trace = generate_binary_ops_trace(val1, val2)?;
        trace.validate()?;

        // Verify we have the correct number of events
        assert_eq!(trace.b32_mul_events().len(), 2);
        assert_eq!(trace.xor_events().len(), 2);
        assert_eq!(trace.xori_events().len(), 2);
        assert_eq!(trace.and_events().len(), 2);
        assert_eq!(trace.andi_events().len(), 2);
        assert_eq!(trace.or_events().len(), 2);
        assert_eq!(trace.ori_events().len(), 2);
        assert_eq!(trace.b32_muli_events().len(), 2);

        // Validate the witness
        Prover::new(Box::new(GenericISA)).validate_witness(&trace)
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(20))]

        #[test]
        fn test_binary_operations(
            val1 in any::<u32>(),
            val2 in any::<u32>(),
        ) {
            prop_assert!(test_binary_ops_with_values(val1, val2).is_ok());
        }
    }
}
