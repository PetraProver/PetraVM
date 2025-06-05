use binius_core::oracle::ShiftVariant;
use binius_field::Field;
use binius_m3::{
    builder::{
        upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1, B32,
    },
    gadgets::barrel_shifter::BarrelShifter,
};
use petravm_asm::{Opcode, SllEvent, SlliEvent, SraEvent, SraiEvent, SrlEvent, SrliEvent};

use crate::{
    channels::Channels,
    gadgets::state::{StateColumns, StateColumnsOptions, StateGadget},
    table::Table,
    types::ProverPackedField,
    utils::{pull_vrom_channel, setup_mux_constraint},
};

// Implementation of SrliTable for immediate shift right logical operations
pub struct SrliTable {
    id: TableId,
    state_cols: StateColumns<{ Opcode::Srli as u16 }>,
    dst_abs: Col<B32>, // Destination absolute address
    dst_val: Col<B32>, // Destination value (shift result)
    src_abs: Col<B32>, // Source absolute address
    src_val: Col<B32>, // Source value (value to be shifted)
}

impl Table for SrliTable {
    type Event = SrliEvent;
    fn name(&self) -> &'static str {
        "SrliTable"
    }
    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("srli");
        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions::default(),
        );

        // Common unpack→packed columns for source value
        let src_val: Col<B32> = table.add_committed("src_val");
        let dst_val: Col<B32> = table.add_committed("dst_val");
        let shift_amount: Col<B32> = upcast_col(state_cols.arg2);

        // Absolute addresses for destination and source
        let dst_abs = table.add_computed("dst_abs", state_cols.fp + upcast_col(state_cols.arg0));
        let src_abs = table.add_computed("src_abs", state_cols.fp + upcast_col(state_cols.arg1));

        // Pull columns from VROM channel
        pull_vrom_channel(&mut table, channels.vrom_channel, [dst_abs, dst_val]);
        pull_vrom_channel(&mut table, channels.vrom_channel, [src_abs, src_val]);
        table.pull(
            channels.right_shifter_channel,
            [src_val, shift_amount, dst_val],
        );

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

impl TableFiller<ProverPackedField> for SrliTable {
    type Event = SrliEvent;
    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a SrliEvent> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        // Fill source value, destination address, and source address
        {
            let mut src_abs = witness.get_scalars_mut(self.src_abs)?;
            let mut src_val = witness.get_scalars_mut(self.src_val)?;
            let mut dst_abs = witness.get_scalars_mut(self.dst_abs)?;
            let mut dst_val = witness.get_scalars_mut(self.dst_val)?;

            for (i, ev) in rows.clone().enumerate() {
                src_val[i] = B32::new(ev.src_val);
                dst_abs[i] = B32::new(ev.fp.addr(ev.dst));
                src_abs[i] = B32::new(ev.fp.addr(ev.src));
                dst_val[i] = B32::new(ev.dst_val);
            }
        }

        // Populate StateGadget and shifter
        let state_rows = rows.map(|ev| StateGadget {
            pc: ev.pc.val(),
            next_pc: None,
            fp: *ev.fp,
            arg0: ev.dst,
            arg1: ev.src,
            arg2: ev.shift_amount as u16,
        });
        self.state_cols.populate(witness, state_rows)?;
        Ok(())
    }
}

// Implementation of SlliTable for immediate shift left logical operations
pub struct SlliTable {
    id: TableId,
    state_cols: StateColumns<{ Opcode::Slli as u16 }>,
    shifter: BarrelShifter,
    dst_abs: Col<B32>, // Destination absolute address
    src_abs: Col<B32>, // Source absolute address
    src_val: Col<B32>, // Source value (value to be shifted)
}

impl Table for SlliTable {
    type Event = SlliEvent;
    fn name(&self) -> &'static str {
        "SlliTable"
    }
    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("slli");
        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions::default(),
        );

        // Common unpack→packed columns for source value
        let src_val_unpacked: Col<B1, 32> = table.add_committed("src_val_unpacked");
        let src_val: Col<B32> = table.add_packed("src_val", src_val_unpacked);

        // Absolute addresses for destination and source
        let dst_abs = table.add_computed("dst_abs", state_cols.fp + upcast_col(state_cols.arg0));
        let src_abs = table.add_computed("src_abs", state_cols.fp + upcast_col(state_cols.arg1));

        // Barrel shifter wired to state_cols.arg2_unpacked (immediate shift amount)
        let shifter = BarrelShifter::new(
            &mut table,
            src_val_unpacked,
            state_cols.arg2_unpacked,
            ShiftVariant::LogicalLeft,
        );
        let dst_val = table.add_packed("dst_val", shifter.output);

        // Pull columns from VROM channel
        pull_vrom_channel(&mut table, channels.vrom_channel, [dst_abs, dst_val]);
        pull_vrom_channel(&mut table, channels.vrom_channel, [src_abs, src_val]);

        Self {
            id: table.id(),
            state_cols,
            shifter,
            dst_abs,
            src_abs,
            src_val,
        }
    }
}

impl TableFiller<ProverPackedField> for SlliTable {
    type Event = SlliEvent;
    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a SlliEvent> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        // Fill source value, destination address, and source address
        {
            let mut src_val = witness.get_scalars_mut(self.src_val)?;
            let mut dst_abs = witness.get_scalars_mut(self.dst_abs)?;
            let mut src_abs = witness.get_scalars_mut(self.src_abs)?;

            for (i, ev) in rows.clone().enumerate() {
                src_val[i] = B32::new(ev.src_val);
                dst_abs[i] = B32::new(ev.fp.addr(ev.dst));
                src_abs[i] = B32::new(ev.fp.addr(ev.src));
            }
        }

        // Populate StateGadget and shifter
        let state_rows = rows.map(|ev| StateGadget {
            pc: ev.pc.val(),
            next_pc: None,
            fp: *ev.fp,
            arg0: ev.dst,
            arg1: ev.src,
            arg2: ev.shift_amount as u16,
        });
        self.state_cols.populate(witness, state_rows)?;
        self.shifter.populate(witness)?;
        Ok(())
    }
}

// Implementation of SrlTable for vrom-based shift right logical operations
pub struct SrlTable {
    id: TableId,
    state_cols: StateColumns<{ Opcode::Srl as u16 }>,
    dst_abs: Col<B32>,   // Destination absolute address
    dst_val: Col<B32>,   // Destination value (shift result)
    src_abs: Col<B32>,   // Source absolute address
    src_val: Col<B32>,   // Source value (shift result)
    shift_abs: Col<B32>, // Shift vrom absolute address
    shift_val: Col<B32>, // Shift value (shift amount)
}

impl Table for SrlTable {
    type Event = SrlEvent;
    fn name(&self) -> &'static str {
        "SrlTable"
    }
    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("srl");
        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions::default(),
        );

        // Address calculations
        let dst_abs = table.add_computed("dst_abs", state_cols.fp + upcast_col(state_cols.arg0));
        let src_abs = table.add_computed("src_abs", state_cols.fp + upcast_col(state_cols.arg1));
        let shift_abs =
            table.add_computed("shift_abs", state_cols.fp + upcast_col(state_cols.arg2));

        // Source value columns
        let src_val = table.add_committed("src_val");
        let dst_val = table.add_committed("dst_val");
        let shift_val = table.add_committed("shift_val");

        // Pull memory access data from VROM channel
        pull_vrom_channel(&mut table, channels.vrom_channel, [dst_abs, dst_val]);
        pull_vrom_channel(&mut table, channels.vrom_channel, [src_abs, src_val]);
        pull_vrom_channel(&mut table, channels.vrom_channel, [shift_abs, shift_val]);

        // Pull shift value from ShifterChannel
        table.pull(
            channels.right_shifter_channel,
            [src_val, shift_val, dst_val],
        );

        Self {
            id: table.id(),
            state_cols,
            dst_abs,
            dst_val,
            src_abs,
            src_val,
            shift_abs,
            shift_val,
        }
    }
}

impl TableFiller<ProverPackedField> for SrlTable {
    type Event = SrlEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a SrlEvent> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        // Fill basic columns and shift amount data
        {
            let mut dst_abs = witness.get_scalars_mut(self.dst_abs)?;
            let mut dst_val = witness.get_scalars_mut(self.dst_val)?;
            let mut src_abs = witness.get_scalars_mut(self.src_abs)?;
            let mut src_val = witness.get_scalars_mut(self.src_val)?;
            let mut shift_abs = witness.get_scalars_mut(self.shift_abs)?;
            let mut shift_val = witness.get_scalars_mut(self.shift_val)?;

            for (i, ev) in rows.clone().enumerate() {
                src_val[i] = B32::new(ev.src_val);
                dst_abs[i] = B32::new(ev.fp.addr(ev.dst));
                src_abs[i] = B32::new(ev.fp.addr(ev.src));
                dst_val[i] = B32::new(ev.dst_val);
                shift_abs[i] = B32::new(ev.fp.addr(ev.shift));
                shift_val[i] = B32::new(ev.shift_amount);
            }
        }

        // Populate StateGadget columns
        let state_rows = rows.clone().map(|ev| StateGadget {
            pc: ev.pc.val(),
            next_pc: None,
            fp: *ev.fp,
            arg0: ev.dst,
            arg1: ev.src,
            arg2: ev.shift,
        });
        self.state_cols.populate(witness, state_rows)
    }
}

// Implementation of SllTable for vrom-based shift left logical operations
pub struct SllTable {
    id: TableId,
    state_cols: StateColumns<{ Opcode::Sll as u16 }>,
    shifter: BarrelShifter,
    dst_abs: Col<B32>,                  // Destination absolute address
    src_abs: Col<B32>,                  // Source absolute address
    src_val_unpacked: Col<B1, 32>,      // Source value in bit-unpacked form
    shift_abs: Col<B32>,                // Shift vrom absolute address
    shift_amount_unpacked: Col<B1, 32>, // Shift amount in bit-unpacked form
    shift_amount_low: Col<B1, 16>,      // Shift amount in bit-unpacked form
}

impl Table for SllTable {
    type Event = SllEvent;
    fn name(&self) -> &'static str {
        "SllTable"
    }
    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("sll");
        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions::default(),
        );

        // Source value columns
        let src_val_unpacked: Col<B1, 32> = table.add_committed("src_val_unpacked");
        let src_val: Col<B32> = table.add_packed("src_val", src_val_unpacked);

        // Address calculations
        let dst_abs = table.add_computed("dst_abs", state_cols.fp + upcast_col(state_cols.arg0));
        let src_abs = table.add_computed("src_abs", state_cols.fp + upcast_col(state_cols.arg1));
        let shift_abs =
            table.add_computed("shift_abs", state_cols.fp + upcast_col(state_cols.arg2));

        // Shift amount columns
        let shift_amount_unpacked: Col<B1, 32> = table.add_committed("shift_amount_unpacked");
        let shift_amount_packed: Col<B32> = table.add_packed("shift_amount", shift_amount_unpacked);
        let shift_amount_low: Col<B1, 16> =
            table.add_selected_block("shift_amount_low", shift_amount_unpacked, 0);

        // Barrel shifter for the actual shift operation
        let shifter = BarrelShifter::new(
            &mut table,
            src_val_unpacked,
            shift_amount_low,
            ShiftVariant::LogicalLeft,
        );
        let dst_val = table.add_packed("dst_val", shifter.output);

        // Pull memory access data from VROM channel
        pull_vrom_channel(&mut table, channels.vrom_channel, [dst_abs, dst_val]);
        pull_vrom_channel(&mut table, channels.vrom_channel, [src_abs, src_val]);
        pull_vrom_channel(
            &mut table,
            channels.vrom_channel,
            [shift_abs, shift_amount_packed],
        );

        Self {
            id: table.id(),
            state_cols,
            shifter,
            dst_abs,
            src_abs,
            src_val_unpacked,
            shift_abs,
            shift_amount_unpacked,
            shift_amount_low,
        }
    }
}

impl TableFiller<ProverPackedField> for SllTable {
    type Event = SllEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a SllEvent> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        // Fill basic columns and shift amount data
        {
            let mut dst_abs = witness.get_scalars_mut(self.dst_abs)?;
            let mut src_abs = witness.get_scalars_mut(self.src_abs)?;
            let mut src_unpacked = witness.get_mut_as(self.src_val_unpacked)?;
            let mut shift_abs = witness.get_scalars_mut(self.shift_abs)?;
            let mut shift_unpacked = witness.get_mut_as(self.shift_amount_unpacked)?;
            let mut shift_amount_low = witness.get_mut_as(self.shift_amount_low)?;

            for (i, ev) in rows.clone().enumerate() {
                src_unpacked[i] = ev.src_val;
                dst_abs[i] = B32::new(ev.fp.addr(ev.dst));
                src_abs[i] = B32::new(ev.fp.addr(ev.src));
                shift_abs[i] = B32::new(ev.fp.addr(ev.shift));
                shift_unpacked[i] = ev.shift_amount;
                shift_amount_low[i] = ev.shift_amount as u16;
            }
        }

        // Populate StateGadget columns
        let state_rows = rows.clone().map(|ev| StateGadget {
            pc: ev.pc.val(),
            next_pc: None,
            fp: *ev.fp,
            arg0: ev.dst,
            arg1: ev.src,
            arg2: ev.shift,
        });
        self.state_cols.populate(witness, state_rows)?;

        // Populate barrel shifter columns
        self.shifter.populate(witness)?;
        Ok(())
    }
}

// SRA: Shift Right Arithmetic (vrom-based shift amount)
pub struct SraTable {
    id: TableId,
    state_cols: StateColumns<{ Opcode::Sra as u16 }>,
    dst_abs: Col<B32>,
    src_abs: Col<B32>,
    src_val_unpacked: Col<B1, 32>,
    sign_bit: Col<B1>,
    inverted_input: Col<B1, 32>, // ~input for negative number path
    shifter_input: Col<B1, 32>,  /* Selected input for shifter (original or inverted based on
                                  * sign bit) */
    shift_abs: Col<B32>,
    shift_val: Col<B32>,
    right_shifter_output: Col<B1, 32>,
    inverted_output: Col<B1, 32>, // ~shifter.output for negative number path
    result: Col<B1, 32>,          // Final result after selection
}

impl Table for SraTable {
    type Event = SraEvent;
    fn name(&self) -> &'static str {
        "SraTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("sra");
        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions::default(),
        );

        // Source value columns
        let src_abs = table.add_computed("src_abs", state_cols.fp + upcast_col(state_cols.arg1));
        let src_val_unpacked: Col<B1, 32> = table.add_committed("src_val_unpacked");
        let src_val: Col<B32> = table.add_packed("src_val", src_val_unpacked);
        pull_vrom_channel(&mut table, channels.vrom_channel, [src_abs, src_val]);

        // Shift amount columns
        let shift_abs =
            table.add_computed("shift_abs", state_cols.fp + upcast_col(state_cols.arg2));
        let shift_val = table.add_committed("shift_val");
        pull_vrom_channel(&mut table, channels.vrom_channel, [shift_abs, shift_val]);

        // Get sign bit (MSB of src value)
        let sign_bit = table.add_selected("sign_bit", src_val_unpacked, 31);

        // Create inverted input for negative numbers: ~input
        let inverted_input = table.add_computed("inverted_input", src_val_unpacked + B1::ONE);

        // Add a committed column for the shifter input (selected based on sign bit)
        // For positive numbers: original input
        // For negative numbers: inverted input (~input)
        let shifter_input = table.add_committed::<B1, 32>("shifter_input");
        let shifter_input_packed = table.add_packed("shifter_input", shifter_input);

        // Mux to select shifter input: sign_bit ? inverted_input : src_val_unpacked
        setup_mux_constraint(
            &mut table,
            &shifter_input,
            &inverted_input,
            &src_val_unpacked,
            &sign_bit,
        );

        let right_shifter_output: Col<B1, 32> = table.add_committed("right_shifter_output");
        let right_shifter_output_packed =
            table.add_packed("right_shifter_output", right_shifter_output);
        table.pull(
            channels.right_shifter_channel,
            [shifter_input_packed, shift_val, right_shifter_output_packed],
        );

        // Invert the shifter output for negative numbers: ~(shifted value)
        // This completes the invert-shift-invert pattern (~(~input >> shift))
        let inverted_output = table.add_computed("inverted_output", right_shifter_output + B1::ONE);

        // Result selector based on sign bit
        let result = table.add_committed("result");

        // Set up multiplexer constraint: result = sign_bit ? inverted_output :
        // shifter.output
        setup_mux_constraint(
            &mut table,
            &result,
            &inverted_output,
            &right_shifter_output,
            &sign_bit,
        );

        // Address calculations
        let dst_abs = table.add_computed("dst_abs", state_cols.fp + upcast_col(state_cols.arg0));
        let dst_val = table.add_packed("dst_val", result);

        // Pull memory access data from VROM channel
        pull_vrom_channel(&mut table, channels.vrom_channel, [dst_abs, dst_val]);

        Self {
            id: table.id(),
            state_cols,
            dst_abs,
            src_abs,
            src_val_unpacked,
            sign_bit,
            inverted_input,
            shifter_input,
            shift_abs,
            shift_val,
            right_shifter_output,
            inverted_output,
            result,
        }
    }
}

impl TableFiller<ProverPackedField> for SraTable {
    type Event = SraEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a SraEvent> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        // Fill basic columns and shift amount data
        {
            let mut dst_abs = witness.get_scalars_mut(self.dst_abs)?;
            let mut src_abs = witness.get_scalars_mut(self.src_abs)?;
            let mut src_unpacked = witness.get_mut_as(self.src_val_unpacked)?;
            let mut shift_abs = witness.get_scalars_mut(self.shift_abs)?;
            let mut shift_val = witness.get_scalars_mut(self.shift_val)?;
            let mut right_shifter_output = witness.get_mut_as(self.right_shifter_output)?;
            let mut inverted_input = witness.get_mut_as(self.inverted_input)?;
            let mut shifter_input = witness.get_mut_as(self.shifter_input)?;
            let mut inverted_output = witness.get_mut_as(self.inverted_output)?;
            let mut result = witness.get_mut_as(self.result)?;
            let mut sign_bit = witness.get_mut(self.sign_bit)?;

            for (i, ev) in rows.clone().enumerate() {
                src_unpacked[i] = ev.src_val;
                dst_abs[i] = B32::new(ev.fp.addr(ev.dst));
                src_abs[i] = B32::new(ev.fp.addr(ev.src));
                shift_abs[i] = B32::new(ev.fp.addr(ev.shift));
                shift_val[i] = B32::new(ev.shift_amount);

                // Calculate sign bit
                let is_negative = (ev.src_val >> 31) & 1 == 1;
                binius_field::packed::set_packed_slice(&mut sign_bit, i, B1::from(is_negative));

                // Calculate inverted input for negative numbers (~input)
                inverted_input[i] = !ev.src_val;

                // Select the input for the shifter based on sign bit
                // For positive numbers: original input
                // For negative numbers: inverted input (~input)
                shifter_input[i] = if is_negative {
                    inverted_input[i]
                } else {
                    ev.src_val
                };

                // For positive numbers: input >> shift
                // For negative numbers: We implement arithmetic right shift using the
                // invert-shift-invert pattern:
                //   1. Invert the input (~input)
                //   2. Perform logical right shift on inverted input
                //   3. Invert the result (~(~input >> shift))
                // This correctly fills 1s from the left for negative numbers
                let shift_result = shifter_input[i] >> (ev.shift_amount & 0x1F) as usize;
                right_shifter_output[i] = shift_result;

                // Calculate inverted output (must be calculated with bit negation)
                inverted_output[i] = !shift_result;

                // Select final output based on sign bit
                result[i] = if is_negative {
                    // For negative numbers: ~(~input >> shift)
                    !shift_result
                } else {
                    // For positive numbers: input >> shift
                    shift_result
                };
            }
        }

        // Populate StateGadget columns
        let state_rows = rows.clone().map(|ev| StateGadget {
            pc: ev.pc.val(),
            next_pc: None,
            fp: *ev.fp,
            arg0: ev.dst,
            arg1: ev.src,
            arg2: ev.shift,
        });
        self.state_cols.populate(witness, state_rows)
    }
}

// SRAI: Shift Right Arithmetic Immediate
pub struct SraiTable {
    id: TableId,
    state_cols: StateColumns<{ Opcode::Srai as u16 }>,
    dst_abs: Col<B32>,
    src_abs: Col<B32>,
    src_val_unpacked: Col<B1, 32>,
    sign_bit: Col<B1>,
    inverted_input: Col<B1, 32>, // ~input for negative number path
    shifter_input: Col<B1, 32>,  /* Selected input for shifter (original or inverted based on
                                  * sign bit) */
    right_shifter_output: Col<B1, 32>, // shifter.output
    inverted_output: Col<B1, 32>,      // ~shifter.output for negative number path
    result: Col<B1, 32>,               // Final result after selection
}

impl Table for SraiTable {
    type Event = SraiEvent;
    fn name(&self) -> &'static str {
        "SraiTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("srai");
        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions::default(),
        );

        // Source value columns
        let src_val_unpacked: Col<B1, 32> = table.add_committed("src_val_unpacked");
        let src_val: Col<B32> = table.add_packed("src_val", src_val_unpacked);

        // Get sign bit (MSB of src value)
        let sign_bit = table.add_selected("sign_bit", src_val_unpacked, 31);

        // Create inverted input for negative numbers: ~input
        let inverted_input = table.add_computed("inverted_input", src_val_unpacked + B1::ONE);

        // Add a committed column for the shifter input (selected based on sign bit)
        // For positive numbers: original input
        // For negative numbers: inverted input (~input)
        let shifter_input = table.add_committed::<B1, 32>("shifter_input");
        let shifter_input_packed: Col<B32> = table.add_packed("shifter_input", shifter_input);

        // Mux to select shifter input: sign_bit ? inverted_input : src_val_unpacked
        setup_mux_constraint(
            &mut table,
            &shifter_input,
            &inverted_input,
            &src_val_unpacked,
            &sign_bit,
        );

        let shift_val = upcast_col(state_cols.arg2);
        let right_shifter_output = table.add_committed("right_shifter_output");
        let right_shifter_output_packed =
            table.add_packed("right_shifter_output", right_shifter_output);
        table.pull(
            channels.right_shifter_channel,
            [shifter_input_packed, shift_val, right_shifter_output_packed],
        );

        // Absolute addresses for destination and source
        let dst_abs = table.add_computed("dst_abs", state_cols.fp + upcast_col(state_cols.arg0));
        let src_abs = table.add_computed("src_abs", state_cols.fp + upcast_col(state_cols.arg1));

        // Invert the shifter output for negative numbers: ~(shifted value)
        // This completes the invert-shift-invert pattern (~(~input >> shift))
        let inverted_output = table.add_computed("inverted_output", right_shifter_output + B1::ONE);

        // Result selector based on sign bit
        let result = table.add_committed("result");

        // Set up multiplexer constraint: result = sign_bit ? inverted_output :
        // shifter.output
        setup_mux_constraint(
            &mut table,
            &result,
            &inverted_output,
            &right_shifter_output,
            &sign_bit,
        );

        let dst_val = table.add_packed("dst_val", result);

        // Pull columns from VROM channel
        pull_vrom_channel(&mut table, channels.vrom_channel, [dst_abs, dst_val]);
        pull_vrom_channel(&mut table, channels.vrom_channel, [src_abs, src_val]);

        Self {
            id: table.id(),
            state_cols,
            dst_abs,
            src_abs,
            src_val_unpacked,
            sign_bit,
            inverted_input,
            shifter_input,
            right_shifter_output,
            inverted_output,
            result,
        }
    }
}

impl TableFiller<ProverPackedField> for SraiTable {
    type Event = SraiEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a SraiEvent> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        // Fill source value, destination address, and source address
        {
            let mut src_unpacked = witness.get_mut_as(self.src_val_unpacked)?;
            let mut dst_abs = witness.get_scalars_mut(self.dst_abs)?;
            let mut src_abs = witness.get_scalars_mut(self.src_abs)?;
            let mut inverted_input = witness.get_mut_as(self.inverted_input)?;
            let mut shifter_input = witness.get_mut_as(self.shifter_input)?;
            let mut right_shifter_output = witness.get_mut_as(self.right_shifter_output)?;
            let mut inverted_output = witness.get_mut_as(self.inverted_output)?;
            let mut result = witness.get_mut_as(self.result)?;
            let mut sign_bit = witness.get_mut(self.sign_bit)?;

            for (i, ev) in rows.clone().enumerate() {
                src_unpacked[i] = ev.src_val;
                dst_abs[i] = B32::new(ev.fp.addr(ev.dst));
                src_abs[i] = B32::new(ev.fp.addr(ev.src));

                // Calculate sign bit
                let is_negative = (ev.src_val >> 31) & 1 == 1;
                binius_field::packed::set_packed_slice(&mut sign_bit, i, B1::from(is_negative));

                // Calculate inverted input for negative numbers (~input)
                inverted_input[i] = !ev.src_val;

                // Select the input for the shifter based on sign bit
                // For positive numbers: original input
                // For negative numbers: inverted input (~input)
                shifter_input[i] = if is_negative {
                    inverted_input[i]
                } else {
                    ev.src_val
                };

                // For positive numbers: input >> shift
                // For negative numbers: We implement arithmetic right shift using the
                // invert-shift-invert pattern:
                //   1. Invert the input (~input)
                //   2. Perform logical right shift on inverted input
                //   3. Invert the result (~(~input >> shift))
                // This correctly fills 1s from the left for negative numbers
                let shift_result = shifter_input[i] >> (ev.shift_amount & 0x1F) as usize;
                right_shifter_output[i] = shift_result;

                // Calculate inverted output (must be calculated with bit negation)
                inverted_output[i] = !shift_result;

                // Select final output based on sign bit
                result[i] = if is_negative {
                    // For negative numbers: ~(~input >> shift)
                    !shift_result
                } else {
                    // For positive numbers: input >> shift
                    shift_result
                };
            }
        }

        // Populate StateGadget
        let state_rows = rows.map(|ev| StateGadget {
            pc: ev.pc.val(),
            next_pc: None,
            fp: *ev.fp,
            arg0: ev.dst,
            arg1: ev.src,
            arg2: ev.shift_amount as u16,
        });
        self.state_cols.populate(witness, state_rows)
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

    /// Creates an execution trace for a simple program that uses various shift
    /// instructions to test shift operations.
    fn generate_shift_trace(val: u32, shift_amount: u32) -> Result<Trace> {
        let imm = shift_amount as u16;
        let asm_code = format!(
            "#[framesize(0x10)]\n\
            _start:\n\
            LDI.W @3, #{shift_amount}\n\
            SRLI @4, @2, #{imm}\n\
            SRL  @5, @2, @3 \n\
            SLLI @6, @2, #{imm}\n\
            SLL  @7, @2, @3 \n\
            SRAI @8, @2, #{imm}\n\
            SRA  @9, @2, @3 \n\
            RET\n"
        );

        let init_values = vec![0, 0, val];
        generate_trace(asm_code, Some(init_values), None)
    }

    fn test_shift_with_values(val: u32, shift_amount: u32) -> Result<()> {
        let trace = generate_shift_trace(val, shift_amount)?;
        trace.validate()?;

        // Verify we have the correct number of events
        assert_eq!(trace.srli_events().len(), 1);
        assert_eq!(trace.slli_events().len(), 1);
        assert_eq!(trace.srl_events().len(), 1);
        assert_eq!(trace.sll_events().len(), 1);
        assert_eq!(trace.srai_events().len(), 1);
        assert_eq!(trace.sra_events().len(), 1);

        // Validate the witness
        Prover::new(Box::new(GenericISA)).validate_witness(&trace)
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(20))]

        #[test]
        fn test_shift_operations(
            val in prop_oneof![
                any::<u32>()                    // Random values
            ],
            shift_amount in prop_oneof![
                Just(0u32),                     // Zero shift
                Just(1),                        // Minimal shift
                Just(31),                       // Maximum shift for u32
                any::<u32>()                    // Random values
            ]
        ) {
            prop_assert!(test_shift_with_values(val, shift_amount).is_ok());
        }
    }
}
