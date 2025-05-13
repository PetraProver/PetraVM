use binius_core::oracle::ShiftVariant;
use binius_m3::builder::{
    upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1, B16, B32,
};
use binius_m3::gadgets::barrel_shifter::BarrelShifter;

use crate::channels::Channels;
use crate::table::Table;
use crate::types::ProverPackedField;

/// Event representing a right logical shift operation
#[derive(Debug, Clone)]
pub struct RightShiftEvent {
    pub input: u32,        // The input value to be shifted
    pub shift_amount: u32, // The shift amount (masked to 5 bits for 32-bit values)
    pub output: u32,       // The result after shifting
}

/// Table that implements a right logical shifter channel
pub struct RightShifterTable {
    id: TableId,
    shifter: BarrelShifter,
    input: Col<B1, 32>,        // Input value in unpacked form
    shift_amount: Col<B1, 16>, // Shift amount in unpacked form (truncated to 16 bits)
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

        // For shift amount, we'll store both the truncated 16-bit version for the
        // barrel shifter and the full 32-bit version for the channel
        let shift_amount: Col<B1, 16> = table.add_committed("shift_amount");
        let shift_amount_packed: Col<B16> = table.add_packed("shift_amount_packed", shift_amount);
        let shift_amount_full: Col<B32> = upcast_col(shift_amount_packed);

        // Create barrel shifter for right logical shift
        let shifter =
            BarrelShifter::new(&mut table, input, shift_amount, ShiftVariant::LogicalRight);

        let output = table.add_packed("output", shifter.output);

        // Push values to the right shifter channel
        table.push(
            channels.right_shifter_channel,
            [input_packed, shift_amount_full, output],
        );

        Self {
            id: table.id(),
            shifter,
            input,
            shift_amount,
        }
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
                // Truncate shift amount to 16 bits for the barrel shifter
                shift_unpacked[i] = ev.shift_amount as u16;
            }
        }

        // Populate the barrel shifter
        self.shifter.populate(witness)?;

        Ok(())
    }
}
