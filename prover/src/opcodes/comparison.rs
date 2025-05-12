use std::ops::Deref;

use binius_field::{Field, PackedField};
use binius_m3::{
    builder::{
        upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1, B32,
    },
    gadgets::sub::{U32Sub, U32SubFlags},
};
use petravm_asm::{
    opcodes::Opcode, SleEvent, SleiEvent, SleiuEvent, SleuEvent, SltEvent, SltiEvent, SltiuEvent,
    SltuEvent,
};

use super::integer_ops::{setup_sign_extended_immediate, SignExtendedImmediateOutput};
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
const SLT_OPCODE: u16 = Opcode::Slt as u16;
const SLTI_OPCODE: u16 = Opcode::Slti as u16;
const SLE_OPCODE: u16 = Opcode::Sle as u16;
const SLEI_OPCODE: u16 = Opcode::Slei as u16;

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

/// SLT table.
///
/// This table handles the SLT instruction, which performs signed
/// integer comparison (set if less than) between two 32-bit elements.
pub struct SltTable {
    id: TableId,
    state_cols: StateColumns<SLT_OPCODE>,
    dst_abs: Col<B32>,
    src1_abs: Col<B32>,
    src1_val: Col<B1, 32>,
    src1_sign: Col<B1>,
    src2_abs: Col<B32>,
    src2_val: Col<B1, 32>,
    src2_sign: Col<B1>,
    dst_bit: Col<B1>,
    subber: U32Sub,
}

impl Table for SltTable {
    type Event = SltEvent;

    fn name(&self) -> &'static str {
        "SltTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("slt");

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

        // Get the sign bits of src1 and src2
        let src1_sign = table.add_selected("src1_sign", src1_val, 31);
        let src2_sign = table.add_selected("src2_sign", src2_val, 31);

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

        // Direct comparison works whenever both signs are equal. If not, it's
        // determined by the src1_val sign. Therefore, the  bit is computed as
        // (src1_sign XOR src2_sign) * src1_sign XOR !(src1_sign XOR src2_sign)
        // * final_borrow
        let dst_bit = table.add_computed(
            "dst_val",
            (src1_sign + src2_sign) * src1_sign + (src1_sign + src2_sign + B1::ONE) * final_borrow,
        );
        let dst_val = upcast_col(dst_bit);

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
            src1_sign,
            src2_abs,
            src2_val,
            src2_sign,
            dst_bit,
            subber,
        }
    }
}

impl TableFiller<ProverPackedField> for SltTable {
    type Event = SltEvent;

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
            let mut src1_sign = witness.get_mut(self.src1_sign)?;
            let mut src2_abs = witness.get_scalars_mut(self.src2_abs)?;
            let mut src2_val = witness.get_mut_as(self.src2_val)?;
            let mut src2_sign = witness.get_mut(self.src2_sign)?;
            let mut dst_bit = witness.get_mut(self.dst_bit)?;

            for (i, event) in rows.clone().enumerate() {
                // Set the values of the first operand
                src1_abs[i] = B32::new(event.fp.addr(event.src1));
                src1_val[i] = event.src1_val;
                let is_src1_negative = (event.src1_val >> 31) & 1 == 1;
                binius_field::packed::set_packed_slice(
                    &mut src1_sign,
                    i,
                    B1::from(is_src1_negative),
                );

                // Set the values of the second operand
                src2_abs[i] = B32::new(event.fp.addr(event.src2));
                src2_val[i] = event.src2_val;
                let is_src2_negative = (event.src2_val >> 31) & 1 == 1;
                binius_field::packed::set_packed_slice(
                    &mut src2_sign,
                    i,
                    B1::from(is_src2_negative),
                );

                // Set the destination to
                dst_abs[i] = B32::new(event.fp.addr(event.dst));
                binius_field::packed::set_packed_slice(
                    &mut dst_bit,
                    i,
                    B1::from(if is_src1_negative ^ is_src2_negative {
                        is_src1_negative
                    } else {
                        event.src1_val < event.src2_val
                    }),
                );
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
/// SLTI table.
///
/// This table handles the SLTI instruction, which performs signed
/// integer comparison (set if less than) between one 32-bit signed elements
/// read from memory, and another 16-bit element given as an immediate.
pub struct SltiTable {
    id: TableId,
    state_cols: StateColumns<SLTI_OPCODE>,
    dst_abs: Col<B32>,
    src_abs: Col<B32>,
    src_val: Col<B1, 32>,
    src_sign: Col<B1>,
    imm_sign: Col<B1>,
    imm_32b: Col<B1, 32>,
    imm_32b_negative: Col<B1, 32>,
    signed_imm_32b: Col<B1, 32>,
    ones: Col<B1, 32>,
    dst_bit: Col<B1>,
    subber: U32Sub,
}

impl Table for SltiTable {
    type Event = SltiEvent;

    fn name(&self) -> &'static str {
        "SltiTable"
    }

    // TODO: Consider swapping the order of src1 and src2 depending on the sign,
    // or using a U32Add gadget.
    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("slti");

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
        let src_abs = table.add_computed("src1", state_cols.fp + upcast_col(state_cols.arg1));

        let src_val = table.add_committed("src_val");
        let src_val_packed = table.add_packed("src_val_packed", src_val);

        // Get the sign bits of src
        let src_sign = table.add_selected("src_sign", src_val, 31);

        // Get the sign bit of imm and compute the sign extension
        let imm_unpacked = state_cols.arg2_unpacked;
        let SignExtendedImmediateOutput {
            imm_unpacked,
            msb,
            negative_unpacked,
            signed_imm_unpacked,
            ones,
        } = setup_sign_extended_immediate(&mut table, imm_unpacked);

        // Instantiate the subtractor with the appropriate flags
        let flags = U32SubFlags {
            borrow_in_bit: None,       // no extra borrow-in
            expose_final_borrow: true, // we want the "underflow" bit out
            commit_zout: false,        // we don't need the raw subtraction result
        };
        let subber = U32Sub::new(&mut table, src_val, signed_imm_unpacked, flags);
        // `final_borrow` is 1 exactly when src_val < imm_val
        let final_borrow: Col<B1> = subber
            .final_borrow
            .expect("Flag `expose_final_borrow` was set to `true`");

        // Direct comparison works whenever both signs are equal. If not, it's
        // determined by the src_val sign. Therefore, the  bit is computed as
        // (src_sign XOR imm_sign) * src_sign XOR !(src_sign XOR imm_sign) *
        // final_borrow
        let dst_bit = table.add_computed(
            "dst_val",
            (src_sign + msb) * src_sign + (src_sign + msb + B1::ONE) * final_borrow,
        );
        let dst_val = upcast_col(dst_bit);

        // Read src1 and src2
        table.pull(vrom_channel, [src_abs, src_val_packed]);

        // Read dst
        table.pull(vrom_channel, [dst_abs, dst_val]);

        Self {
            id: table.id(),
            state_cols,
            dst_abs,
            src_abs,
            src_val,
            src_sign,
            imm_sign: msb,
            imm_32b: imm_unpacked,
            imm_32b_negative: negative_unpacked,
            signed_imm_32b: signed_imm_unpacked,
            ones,
            dst_bit,
            subber,
        }
    }
}

impl TableFiller<ProverPackedField> for SltiTable {
    type Event = SltiEvent;

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
            let mut src_sign = witness.get_mut(self.src_sign)?;
            let mut imm_32b = witness.get_mut_as(self.imm_32b)?;
            let mut imm_sign = witness.get_mut(self.imm_sign)?;
            let mut imm_32b_negative = witness.get_mut_as(self.imm_32b_negative)?;
            let mut signed_imm_32b = witness.get_mut_as(self.signed_imm_32b)?;
            let mut dst_bit = witness.get_mut(self.dst_bit)?;
            let mut ones_col = witness.get_mut_as(self.ones)?;

            for (i, event) in rows.clone().enumerate() {
                // Set the values of the first operand
                src_abs[i] = B32::new(event.fp.addr(event.src));
                src_val[i] = event.src_val;
                let is_src1_negative = (event.src_val >> 31) & 1 == 1;
                binius_field::packed::set_packed_slice(
                    &mut src_sign,
                    i,
                    B1::from(is_src1_negative),
                );

                // Set the values of the second operand
                imm_32b[i] = event.imm as u32;
                let is_imm_negative = (event.imm >> 15) & 1 == 1;
                binius_field::packed::set_packed_slice(&mut imm_sign, i, B1::from(is_imm_negative));

                // Compute the sign extension of `imm`.
                let ones = 0b1111_1111_1111_1111u32;
                ones_col[i] = ones << 16;
                imm_32b_negative[i] = (ones << 16) + event.imm as u32;
                signed_imm_32b[i] = event.imm as i16 as i32;

                // Set the destination to
                dst_abs[i] = B32::new(event.fp.addr(event.dst));
                binius_field::packed::set_packed_slice(
                    &mut dst_bit,
                    i,
                    B1::from(if is_src1_negative ^ is_imm_negative {
                        is_src1_negative
                    } else {
                        event.src_val < (signed_imm_32b[i] as u32)
                    }),
                );
            }
        }
        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.into(),
            next_pc: None,
            fp: *event.fp.deref(),
            arg0: event.dst,
            arg1: event.src,
            arg2: event.imm,
        });
        self.state_cols.populate(witness, state_rows)?;
        self.subber.populate(witness)
    }
}

/// SLE table.
///
/// This table handles the SLE instruction, which performs signed
/// integer comparison (set if less or equal than) between two 32-bit elements.
pub struct SleTable {
    id: TableId,
    state_cols: StateColumns<SLE_OPCODE>,
    dst_abs: Col<B32>,
    src1_abs: Col<B32>,
    src1_val: Col<B1, 32>,
    src1_sign: Col<B1>,
    src2_abs: Col<B32>,
    src2_val: Col<B1, 32>,
    src2_sign: Col<B1>,
    dst_bit: Col<B1>,
    subber: U32Sub,
}

impl Table for SleTable {
    type Event = SleEvent;

    fn name(&self) -> &'static str {
        "SleTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("sle");

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

        // Get the sign bits of src1 and src2
        let src1_sign = table.add_selected("src1_sign", src1_val, 31);
        let src2_sign = table.add_selected("src2_sign", src2_val, 31);

        // Instantiate the subtractor with the appropriate flags
        let flags = U32SubFlags {
            borrow_in_bit: None,       // no extra borrow-in
            expose_final_borrow: true, // we want the "underflow" bit out
            commit_zout: false,        // we don't need the raw subtraction result
        };
        let subber = U32Sub::new(&mut table, src2_val, src1_val, flags);
        // `final_borrow` is 1 exactly when src1_val < src2_val
        let final_borrow: Col<B1> = subber
            .final_borrow
            .expect("Flag `expose_final_borrow` was set to `true`");

        // Direct comparison works whenever both signs are equal. If not, it's
        // determined by the src1_val sign. Therefore, the  bit is computed as
        // (src1_sign XOR src2_sign) * src1_sign XOR !(src1_sign XOR src2_sign)
        // * final_borrow
        let dst_bit = table.add_computed(
            "dst_val",
            (src1_sign + src2_sign) * src1_sign
                + (src1_sign + src2_sign + B1::ONE) * (final_borrow + B1::ONE),
        );
        let dst_val = upcast_col(dst_bit);

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
            src1_sign,
            src2_abs,
            src2_val,
            src2_sign,
            dst_bit,
            subber,
        }
    }
}

impl TableFiller<ProverPackedField> for SleTable {
    type Event = SleEvent;

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
            let mut src1_sign = witness.get_mut(self.src1_sign)?;
            let mut src2_abs = witness.get_scalars_mut(self.src2_abs)?;
            let mut src2_val = witness.get_mut_as(self.src2_val)?;
            let mut src2_sign = witness.get_mut(self.src2_sign)?;
            let mut dst_bit = witness.get_mut(self.dst_bit)?;

            for (i, event) in rows.clone().enumerate() {
                dst_abs[i] = B32::new(event.fp.addr(event.dst));
                src1_abs[i] = B32::new(event.fp.addr(event.src1));
                src1_val[i] = event.src1_val;
                let is_src1_negative = (event.src1_val >> 31) & 1 == 1;
                binius_field::packed::set_packed_slice(
                    &mut src1_sign,
                    i,
                    B1::from(is_src1_negative),
                );
                src2_abs[i] = B32::new(event.fp.addr(event.src2));
                src2_val[i] = event.src2_val;
                let is_src2_negative = (event.src2_val >> 31) & 1 == 1;
                binius_field::packed::set_packed_slice(
                    &mut src2_sign,
                    i,
                    B1::from(is_src2_negative),
                );
                binius_field::packed::set_packed_slice(
                    &mut dst_bit,
                    i,
                    B1::from(if is_src1_negative ^ is_src2_negative {
                        is_src1_negative
                    } else {
                        event.src1_val <= event.src2_val
                    }),
                );
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

/// SLEI table.
///
/// This table handles the SLEI instruction, which performs signed
/// integer comparison (set if less than) between on 32-bit elementm
/// and another 16-bit element given as an immediate.
pub struct SleiTable {
    id: TableId,
    state_cols: StateColumns<SLEI_OPCODE>,
    dst_abs: Col<B32>,
    src_abs: Col<B32>,
    src_val: Col<B1, 32>,
    src_sign: Col<B1>,
    imm_sign: Col<B1>,
    imm_32b: Col<B1, 32>,
    imm_32b_negative: Col<B1, 32>,
    signed_imm_32b: Col<B1, 32>,
    ones: Col<B1, 32>,
    dst_bit: Col<B1>,
    subber: U32Sub,
}

impl Table for SleiTable {
    type Event = SleiEvent;

    fn name(&self) -> &'static str {
        "SleiTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("slei");

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
        let src_abs = table.add_computed("src1", state_cols.fp + upcast_col(state_cols.arg1));

        let src_val = table.add_committed("src1_val");
        let src_val_packed = table.add_packed("src1_val_packed", src_val);

        // Get the sign bits of src
        let src_sign = table.add_selected("src_sign", src_val, 31);

        // Get the sign bit of imm and compute the sign extension
        let imm_unpacked = state_cols.arg2_unpacked;
        let SignExtendedImmediateOutput {
            imm_unpacked,
            msb,
            negative_unpacked,
            signed_imm_unpacked,
            ones,
        } = setup_sign_extended_immediate(&mut table, imm_unpacked);

        // Instantiate the subtractor with the appropriate flags
        let flags = U32SubFlags {
            borrow_in_bit: None,       // no extra borrow-in
            expose_final_borrow: true, // we want the "underflow" bit out
            commit_zout: false,        // we don't need the raw subtraction result
        };
        let subber = U32Sub::new(&mut table, signed_imm_unpacked, src_val, flags);
        // `final_borrow` is 1 exactly when src_val < imm_val
        let final_borrow: Col<B1> = subber
            .final_borrow
            .expect("Flag `expose_final_borrow` was set to `true`");

        // Direct comparison works whenever both sigs are equal. If not, it's determined
        // by the src_val sign. Therefore, the  bit is computed as (src_sign
        // XOR imm_sign) * src_sign XOR !(src_sign XOR imm_sign) *
        // !final_borrow
        let dst_bit = table.add_computed(
            "dst_val",
            (src_sign + msb) * src_sign + (src_sign + msb + B1::ONE) * (final_borrow + B1::ONE),
        );
        let dst_val = upcast_col(dst_bit);

        // Read src1 and src2
        table.pull(vrom_channel, [src_abs, src_val_packed]);

        // Read dst
        table.pull(vrom_channel, [dst_abs, dst_val]);

        Self {
            id: table.id(),
            state_cols,
            dst_abs,
            src_abs,
            src_val,
            src_sign,
            imm_sign: msb,
            imm_32b: imm_unpacked,
            imm_32b_negative: negative_unpacked,
            signed_imm_32b: signed_imm_unpacked,
            ones,
            dst_bit,
            subber,
        }
    }
}

impl TableFiller<ProverPackedField> for SleiTable {
    type Event = SleiEvent;

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
            let mut src_sign = witness.get_mut(self.src_sign)?;
            let mut imm_32b = witness.get_mut_as(self.imm_32b)?;
            let mut imm_sign = witness.get_mut(self.imm_sign)?;
            let mut imm_32b_negative = witness.get_mut_as(self.imm_32b_negative)?;
            let mut signed_imm_32b = witness.get_mut_as(self.signed_imm_32b)?;
            let mut dst_bit = witness.get_mut(self.dst_bit)?;
            let mut ones_col = witness.get_mut_as(self.ones)?;

            for (i, event) in rows.clone().enumerate() {
                // Set the values of the first operand
                src_abs[i] = B32::new(event.fp.addr(event.src));
                src_val[i] = event.src_val;
                let is_src1_negative = (event.src_val >> 31) & 1 == 1;
                binius_field::packed::set_packed_slice(
                    &mut src_sign,
                    i,
                    B1::from(is_src1_negative),
                );

                // Set the values of the second operand
                imm_32b[i] = event.imm as u32;
                let is_imm_negative = (event.imm >> 15) & 1 == 1;
                binius_field::packed::set_packed_slice(&mut imm_sign, i, B1::from(is_imm_negative));

                // Compute the sign extension of `imm`.
                let ones = 0b1111_1111_1111_1111u32;
                ones_col[i] = ones << 16;
                imm_32b_negative[i] = (ones << 16) + event.imm as u32;
                signed_imm_32b[i] = event.imm as i16 as i32;

                // Set the destination to
                dst_abs[i] = B32::new(event.fp.addr(event.dst));
                binius_field::packed::set_packed_slice(
                    &mut dst_bit,
                    i,
                    B1::from(if is_src1_negative ^ is_imm_negative {
                        is_src1_negative
                    } else {
                        event.src_val <= (signed_imm_32b[i] as u32)
                    }),
                );
            }
        }
        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.into(),
            next_pc: None,
            fp: *event.fp.deref(),
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
    use petravm_asm::{isa::GenericISA, Opcode};
    use proptest::prelude::*;
    use proptest::prop_oneof;

    use crate::prover::Prover;
    use crate::test_utils::generate_trace;

    /// Creates an execution trace for a simple program that uses a single
    /// comparison operation.
    fn test_comparison_with_values(opcode: Opcode, src1_val: u32, src2_val: u32) -> Result<()> {
        let asm_code = match opcode {
            Opcode::Sltu => format!(
                "#[framesize(0x10)]\n\
                    _start: 
                        LDI.W @2, #{src1_val}\n\
                        LDI.W @3, #{src2_val}\n\
                        SLTU @4, @2, @3\n\
                        RET\n",
            ),
            Opcode::Sltiu => {
                let imm = src2_val as u16;
                format!(
                    "#[framesize(0x10)]\n\
                 _start: 
                    LDI.W @2, #{src1_val}\n\
                    SLTIU @3, @2, #{imm}\n\
                    RET\n",
                )
            }
            Opcode::Sleu => {
                format!(
                    "#[framesize(0x10)]\n\
                 _start: 
                    LDI.W @2, #{src1_val}\n\
                    LDI.W @3, #{src2_val}\n\
                    SLEU @4, @2, @3\n\
                    RET\n"
                )
            }
            Opcode::Sleiu => {
                let imm = src2_val as u16;
                format!(
                    "#[framesize(0x10)]\n\
                 _start: 
                    LDI.W @2, #{src1_val}\n\
                    SLEIU @3, @2, #{imm}\n\
                    RET\n"
                )
            }
            Opcode::Slt => format!(
                "#[framesize(0x10)]\n\
                 _start: 
                    LDI.W @2, #{src1_val}\n\
                    LDI.W @3, #{src2_val}\n\
                    SLT @4, @2, @3\n\
                    RET\n"
            ),
            Opcode::Slti => {
                let imm = src2_val as u16;
                format!(
                    "#[framesize(0x10)]\n\
                 _start: 
                    LDI.W @2, #{src1_val}\n\
                    SLTI @3, @2, #{imm}\n\
                    RET\n"
                )
            }
            Opcode::Sle => {
                format!(
                    "#[framesize(0x10)]\n\
                 _start: 
                    LDI.W @2, #{src1_val}\n\
                    LDI.W @3, #{src2_val}\n\
                    SLE @4, @2, @3\n\
                    RET\n"
                )
            }
            Opcode::Slei => {
                let imm = src2_val as u16;
                format!(
                    "#[framesize(0x10)]\n\
                 _start: 
                    LDI.W @2, #{src1_val}\n\
                    SLEI @3, @2, #{imm}\n\
                    RET\n"
                )
            }
            _ => panic!("Not a comparison opcode"),
        };

        let trace = generate_trace(asm_code, None, None)?;
        trace.validate()?;
        assert_eq!(
            match opcode {
                Opcode::Sltu => trace.sltu_events().len(),
                Opcode::Sltiu => trace.sltiu_events().len(),
                Opcode::Sleu => trace.sleu_events().len(),
                Opcode::Sleiu => trace.sleiu_events().len(),
                Opcode::Slt => trace.slt_events().len(),
                Opcode::Slti => trace.slti_events().len(),
                Opcode::Sle => trace.sle_events().len(),
                Opcode::Slei => trace.slei_events().len(),
                _ => panic!("Not a comparison opcode"),
            },
            1
        );
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
            prop_assert!(test_comparison_with_values(Opcode::Sltu, src1_val, src2_val).is_ok());
        }
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(20))]

        #[test]
        fn test_sltiu_operations(
            // Test both random values and specific edge cases
            (src_val, imm) in prop_oneof![
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
            prop_assert!(test_comparison_with_values(Opcode::Sltiu, src_val, imm as u32).is_ok());
        }
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(20))]

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
            prop_assert!(test_comparison_with_values(Opcode::Sleu, src1_val, src2_val).is_ok());
        }
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(20))]

        #[test]
        fn test_sleiu_operations(
            // Test both random values and specific edge cases
            (src_val, imm) in prop_oneof![
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
            prop_assert!(test_comparison_with_values(Opcode::Sleiu, src_val, imm as u32).is_ok());
        }
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(20))]

        #[test]
        fn test_slt_operations(
            // Test both random values and specific edge cases
            (src1_val, src2_val) in prop_oneof![
                // Random value pairs
                (any::<u32>(), any::<u32>()),

                // Edge cases
                Just((0, 0)),                  // Equal at zero
                Just((1, 0)),                  // Greater than
                Just((0, 1)),                  // Less than
                Just((-1i32 as u32, 0)),          // -1 < 0
                Just((i32::MAX as u32, i32::MAX as u32)),   // Equal at max
                Just((i32::MIN as u32, i32::MIN as u32)),   // Equal at min
                Just((i32::MIN as u32, i32::MAX as u32)),   // Min < Max
                Just((i32::MAX as u32, i32::MIN as u32)),   // Max > Min

                // Additional interesting cases
                Just(((i32::MAX/2 - 1) as u32, (i32::MAX/2) as u32)),  // Middle values
                Just(((i32::MIN/2) as u32, (i32::MIN/2 + 1) as u32)),  // Middle values
                Just((1, i32::MAX as u32)),                // 1 < MAX
                Just(((i32::MAX - 1) as u32, i32::MAX as u32)),       // MAX-1 < MAX
                Just((i32::MIN as u32, (i32::MIN + 1) as u32)),      // MIN < MIN + 1
            ],
        ) {
            prop_assert!(test_comparison_with_values(Opcode::Slt, src1_val, src2_val).is_ok());
        }
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(20))]

        #[test]
        fn test_slti_operations(
            // Test both random values and specific edge cases
            (src_val, imm) in prop_oneof![
                // Random value pairs
                (any::<u32>(), any::<u16>()),

                // Edge cases
                Just((0, 0)),                  // Equal at zero
                Just((1, 0)),                  // Greater than
                Just((0, 1)),                  // Less than
                Just((-1i32 as u32, 0)),          // -1 < 0

                // Additional interesting cases
                Just((1, i16::MAX as u16)),                // 1 < MAX
                Just(((i16::MAX - 1) as u32, i16::MAX as u16)),       // MAX-1 < MAX
                Just((i16::MIN as u32, (i16::MIN + 1) as u16)),      // MIN < MIN + 1
            ],
        ) {
            prop_assert!(test_comparison_with_values(Opcode::Slti, src_val, imm as u32).is_ok());
        }
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(20))]

        #[test]
        fn test_sle_operations(
            // Test both random values and specific edge cases
            (src1_val, src2_val) in prop_oneof![
                // Random value pairs
                (any::<u32>(), any::<u32>()),

                // Edge cases
                Just((0, 0)),                  // Equal at zero
                Just((1, 0)),                  // Greater than
                Just((0, 1)),                  // Less than
                Just((-1i32 as u32, 0)),          // -1 < 0
                Just((i32::MAX as u32, i32::MAX as u32)),   // Equal at max
                Just((i32::MIN as u32, i32::MIN as u32)),   // Equal at min
                Just((i32::MIN as u32, i32::MAX as u32)),   // Min < Max
                Just((i32::MAX as u32, i32::MIN as u32)),   // Max > Min

                // Additional interesting cases
                Just(((i32::MAX/2 - 1) as u32, (i32::MAX/2) as u32)),  // Middle values
                Just(((i32::MIN/2) as u32, (i32::MIN/2 + 1) as u32)),  // Middle values
                Just((1, i32::MAX as u32)),                // 1 < MAX
                Just(((i32::MAX - 1) as u32, i32::MAX as u32)),       // MAX-1 < MAX
                Just((i32::MIN as u32, (i32::MIN + 1) as u32)),      // MIN < MIN + 1
            ],
        ) {
            prop_assert!(test_comparison_with_values(Opcode::Sle, src1_val, src2_val).is_ok());
        }
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(20))]

        #[test]
        fn test_slei_operations(
            // Test both random values and specific edge cases
            (src_val, imm) in prop_oneof![
                // Random value pairs
                (any::<u32>(), any::<u16>()),

                // Edge cases
                Just((0, 0)),                  // Equal at zero
                Just((1, 0)),                  // Greater than
                Just((0, 1)),                  // Less than
                Just((-1i32 as u32, 0)),          // -1 < 0

                // Additional interesting cases
                Just((1, i16::MAX as u16)),                // 1 < MAX
                Just(((i16::MAX - 1) as u32, i16::MAX as u16)),       // MAX-1 < MAX
                Just((i16::MIN as u32, (i16::MIN + 1) as u16)),      // MIN < MIN + 1
            ],
        ) {
            prop_assert!(test_comparison_with_values(Opcode::Slei, src_val, imm as u32).is_ok());
        }
    }
}
