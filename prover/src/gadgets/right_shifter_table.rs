use binius_core::oracle::ShiftVariant;
use binius_m3::builder::{
    Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1, B32,
};
use binius_m3::gadgets::barrel_shifter::BarrelShifter;

use crate::channels::Channels;
use crate::table::Table;
use crate::types::ProverPackedField;

/// Event representing a right logical shift operation
#[derive(Debug, Clone)]
pub struct RightShiftEvent {
    pub input: u32,        // The input value to be shifted
    pub shift_amount: u16, // The shift amount (masked to 5 bits for 32-bit values)
    pub output: u32,       // The result after shifting
}

/// Table that implements a right logical shifter channel
pub struct RightShifterTable {
    id: TableId,
    shifter: BarrelShifter,
    input: Col<B1, 32>,        // Input value in unpacked form
    input_packed: Col<B32>,    // Input value in packed form
    shift_amount: Col<B1, 16>, // Shift amount in unpacked form
    output: Col<B32>,          // Output value (shifted result)
}

impl Table for RightShifterTable {
    type Event = RightShiftEvent;

    fn name(&self) -> &'static str {
        "RightShifterTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("right_shifter");

        // Define columns
        let input: Col<B1, 32> = table.add_committed("input");
        let input_packed: Col<B32> = table.add_packed("input_packed", input);

        let shift_amount: Col<B1, 16> = table.add_committed("shift_amount");

        // Create barrel shifter for right logical shift
        let shifter =
            BarrelShifter::new(&mut table, input, shift_amount, ShiftVariant::LogicalRight);

        let output = table.add_packed("output", shifter.output);

        // Push values to the right shifter channel
        table.push(channels.right_shifter_channel, [input_packed, output]);

        Self {
            id: table.id(),
            shifter,
            input,
            input_packed,
            shift_amount,
            output,
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl TableFiller<ProverPackedField> for RightShifterTable {
    type Event = RightShiftEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a RightShiftEvent> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        // Fill input and shift amount columns
        {
            let mut input_unpacked = witness.get_mut_as(self.input)?;
            let mut shift_unpacked = witness.get_mut_as(self.shift_amount)?;

            for (i, ev) in rows.clone().enumerate() {
                input_unpacked[i] = ev.input;
                shift_unpacked[i] = ev.shift_amount;
            }
        }

        // Populate the barrel shifter
        self.shifter.populate(witness)?;

        Ok(())
    }
}
