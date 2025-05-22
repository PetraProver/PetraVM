use binius_core::oracle::ShiftVariant;
use binius_m3::builder::{
    Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1, B32,
};
use binius_m3::gadgets::barrel_shifter::BarrelShifter;
use petravm_asm::event::RightLogicShiftGadgetEvent;

use crate::channels::Channels;
use crate::table::Table;
use crate::types::ProverPackedField;

/// Table that implements a right logical shifter channel
pub struct RightShifterTable {
    id: TableId,
    shifter: BarrelShifter,
    input: Col<B1, 32>,            // Input value in unpacked form
    shift_amount: Col<B1, 32>,     // Shift amount in unpacked form
    shift_amount_low: Col<B1, 16>, // Shift amount in unpacked form
}

impl Table for RightShifterTable {
    type Event = RightLogicShiftGadgetEvent;

    fn name(&self) -> &'static str {
        "RightShifterTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("right_shifter");

        // Define columns
        let input: Col<B1, 32> = table.add_committed("input");
        let input_packed: Col<B32> = table.add_packed("input_packed", input);

        // For shift amount, we'll store both the truncated 16-bit version for the
        // barrel shifter and the full 32-bit version for the channel
        let shift_amount: Col<B1, 32> = table.add_committed("shift_amount");
        let shift_amount_low =
            table.add_selected_block::<_, 32, 16>("shift_amount_low", shift_amount, 0);
        let shift_amount_packed: Col<B32> = table.add_packed("shift_amount_packed", shift_amount);

        // Create barrel shifter for right logical shift
        let shifter = BarrelShifter::new(
            &mut table,
            input,
            shift_amount_low,
            ShiftVariant::LogicalRight,
        );

        let output = table.add_packed("output", shifter.output);

        // TODO: Check performance of pushing a packed column with 16 bits shift amount
        // Push values to the right shifter channel
        table.push(
            channels.right_shifter_channel,
            [input_packed, shift_amount_packed, output],
        );

        Self {
            id: table.id(),
            shifter,
            input,
            shift_amount,
            shift_amount_low,
        }
    }
}

impl TableFiller<ProverPackedField> for RightShifterTable {
    type Event = RightLogicShiftGadgetEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a RightLogicShiftGadgetEvent> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        // Fill input and shift amount columns
        {
            let mut input_unpacked = witness.get_mut_as(self.input)?;
            let mut shift_unpacked = witness.get_mut_as(self.shift_amount)?;
            let mut shift_amount_low = witness.get_mut_as(self.shift_amount_low)?;

            for (i, ev) in rows.clone().enumerate() {
                input_unpacked[i] = ev.input;
                shift_unpacked[i] = ev.shift_amount;
                shift_amount_low[i] = ev.shift_amount as u16;
            }
        }

        // Populate the barrel shifter
        self.shifter.populate(witness)?;

        Ok(())
    }
}
