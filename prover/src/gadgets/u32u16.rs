use binius_core::oracle::ShiftVariant;
use binius_field::{packed::set_packed_slice, Field, PackedExtension, PackedFieldIndexable};
use binius_m3::{
    builder::{
        column::Col, types::B1, upcast_col, witness::TableWitnessSegment, TableBuilder, B128, B16,
    },
    gadgets::u32::U32AddFlags,
};
/// TODO: It might be better to use a new version of ZeroPadding that would
/// allow to pad "in the middle" so we can go from 16 bits to 32 bits. We would
/// no longer need this gadget in that case.
///
/// A gadget for performing the addition of a 32-bit integer with a 16-bit
/// immediate on vertically-packed bit columns.
///
/// This gadget has input columns `xin` and `yin` for the 32-bit and 16-bit
/// integers to be added, and an output column `zout`, and it constrains that
/// `xin + yin = zout` as integers.
#[derive(Debug)]
pub struct U32U16Add {
    // Inputs
    pub xin: Col<B1, 32>,
    pub yin: Col<B1, 16>,
    pub xin_low: Col<B1, 16>,  // Virtual
    pub xin_high: Col<B1, 16>, // Virtual

    // Private
    // Shifted carry
    cin: Col<B1, 32>,
    // Carry
    cout: Col<B1, 32>,
    cout_shl: Col<B1, 32>,
    // Projections (lower and higher bits)
    cin_low: Col<B1, 16>,   // Virtual
    cin_high: Col<B1, 16>,  // Virtual
    cout_low: Col<B1, 16>,  // Virtual
    cout_high: Col<B1, 16>, // Virtual

    // Outputs
    /// The output column, either committed if `flags.commit_zout` is set,
    /// otherwise a linear combination derived column.
    pub zout: Col<B1, 32>,
    // Projections (lower and higher bits) of the output column.
    pub zout_low: Col<B1, 16>,  // Virtual
    pub zout_high: Col<B1, 16>, // Virtual
    // Final carry of the lower 16 bits.
    pub final_carry_low: Col<B1, 16>,
    /// This is `Some` if `flags.expose_final_carry` is set, otherwise it is
    /// `None`.
    pub final_carry: Option<Col<B1>>,
    /// Flags modifying the gadget's behavior.
    pub flags: U32AddFlags,
}

impl U32U16Add {
    pub fn new(
        table: &mut TableBuilder,
        xin: Col<B1, 32>,
        yin: Col<B1, 16>,
        flags: U32AddFlags,
    ) -> Self {
        let cout = table.add_committed::<B1, 32>("cout");
        let cout_shl = table.add_shifted("cout_shl", cout, 5, 1, ShiftVariant::LogicalLeft);

        let cin = if let Some(carry_in_bit) = flags.carry_in_bit {
            table.add_computed("cin", cout_shl + carry_in_bit)
        } else {
            cout_shl
        };

        let final_carry = flags
            .expose_final_carry
            .then(|| table.add_selected("final_carry", cout, 31));

        // Get lower and higher bits values for the necessary columns.
        let xin_low = table.add_selected_block("xin_low", xin, 0);
        let xin_high: Col<B1, 16> = table.add_selected_block("xin_high", xin, 1);
        let cout_low = table.add_selected_block("cin_low", cout, 0);
        let cout_high = table.add_selected_block("cin_high", cout, 1);

        // TODO: If we have padding on the left as a virtual column, then we do not need
        // to commit to this column.
        let final_carry_low_16b_unpacked = table.add_committed("final carry low unpacked");
        let final_carry_low: Col<B16> =
            upcast_col(table.add_selected("final carry low", cout_low, 15));

        // Check that the committed final carry for the lower bits is correct.
        let packed = table.add_packed("low carry packed", final_carry_low_16b_unpacked);
        table.assert_zero("packed low carry", packed - final_carry_low);

        let cin_low = table.add_selected_block("cin_low", cin, 0);
        let cin_high_bits = table.add_selected_block("cin_high", cin, 1);
        // Compute the carry for the higher bits, taking the final lower bits carry into
        // account.
        let cin_high = table.add_computed(
            "cin high full",
            cin_high_bits + final_carry_low_16b_unpacked,
        );

        // Check that the carry for the lower bits is correct.
        table.assert_zero(
            "carry_out_low",
            (xin_low + cin_low) * (yin + cin_low) + cin_low - cout_low,
        );
        // Check that the carry for the higher bits is correct.
        table.assert_zero(
            "carry_out_high",
            (xin_high + cin_high) * cin_high + cin_high - cout_high,
        );

        // Check that the output is correct.
        let zout = table.add_committed::<B1, 32>("zout");
        let zout_low = table.add_selected_block("zout_low", zout, 0);
        let zout_high = table.add_selected_block("zout high", zout, 1);
        table.assert_zero("zout_low_zerocheck", xin_low + yin + cin_low - zout_low);
        table.assert_zero("zout_high_zerocheck", xin_high + cin_high - zout_high);

        Self {
            xin,
            yin,
            cin,
            cout,
            cout_shl,
            final_carry_low: final_carry_low_16b_unpacked,
            final_carry,
            zout,
            xin_low,
            xin_high,
            cin_low,
            cin_high,
            cout_low,
            cout_high,
            zout_low,
            zout_high,
            flags,
        }
    }

    pub fn populate<P>(&self, index: &mut TableWitnessSegment<P>) -> Result<(), anyhow::Error>
    where
        P: PackedFieldIndexable<Scalar = B128> + PackedExtension<B1>,
    {
        let xin: std::cell::RefMut<'_, [u32]> = index.get_mut_as(self.xin)?;
        let yin: std::cell::RefMut<'_, [u16]> = index.get_mut_as(self.yin)?;
        let mut cout = index.get_mut_as(self.cout)?;
        let mut zout = index.get_mut_as(self.zout)?;
        let mut cin_low = index.get_mut_as(self.cin_low)?;
        let mut cin_high = index.get_mut_as(self.cin_high)?;
        let mut cout_low = index.get_mut_as(self.cout_low)?;
        let mut cout_high = index.get_mut_as(self.cout_high)?;
        let mut xin_low = index.get_mut_as(self.xin_low)?;
        let mut xin_high = index.get_mut_as(self.xin_high)?;
        let mut zout_low = index.get_mut_as(self.zout_low)?;
        let mut zout_high = index.get_mut_as(self.zout_high)?;
        let mut final_carry_low: std::cell::RefMut<'_, [u16]> =
            index.get_mut_as(self.final_carry_low)?;
        let mut final_carry = if let Some(final_carry) = self.final_carry {
            let final_carry = index.get_mut(final_carry)?;
            Some(final_carry)
        } else {
            None
        };

        if let Some(carry_in_bit_col) = self.flags.carry_in_bit {
            // This is u32 assumed to be either 0 or 1.
            let carry_in_bit = index.get_mut_as(carry_in_bit_col)?;

            let mut cin = index.get_mut_as(self.cin)?;
            let mut cout_shl = index.get_mut_as(self.cout_shl)?;
            for i in 0..index.size() {
                let (x_plus_y, carry0) = xin[i].overflowing_add(yin[i] as u32);
                let carry1;
                (zout[i], carry1) = x_plus_y.overflowing_add(carry_in_bit[i]);
                let carry = carry0 | carry1;

                // Fill the projected values.
                xin_low[i] = xin[i] as u16;
                xin_high[i] = (xin[i] >> 16) as u16;
                cin_low[i] = cin[i] as u16;
                cin_high[i] = (cin[i] >> 16) as u16;
                cout_low[i] = cout[i] as u16;
                cout_high[i] = (cout[i] >> 16) as u16;
                zout_low[i] = zout[i] as u16;
                zout_high[i] = (zout[i] >> 16) as u16;

                cin[i] = xin[i] ^ yin[i] as u32 ^ zout[i];
                cout[i] = (carry as u32) << 31 | cin[i] >> 1;
                cout_shl[i] = cout[i] << 1;

                let carry_val = (cout[i] >> 16) & 1;
                final_carry_low[i] = carry_val as u16;
                if let Some(ref mut final_carry) = final_carry {
                    set_packed_slice(&mut *final_carry, i, if carry { B1::ONE } else { B1::ZERO });
                }
            }
        } else {
            // When the carry in bit is fixed to zero, we can simplify the logic.
            let mut cin = index.get_mut_as(self.cin)?;
            for i in 0..index.size() {
                let carry;
                (zout[i], carry) = xin[i].overflowing_add(yin[i] as u32);
                cin[i] = xin[i] ^ yin[i] as u32 ^ zout[i];
                cout[i] = (carry as u32) << 31 | cin[i] >> 1;

                // Fill the projected values.
                xin_low[i] = xin[i] as u16;
                xin_high[i] = (xin[i] >> 16) as u16;
                cin_low[i] = cin[i] as u16;
                cin_high[i] = (cin[i] >> 16) as u16;
                cout_low[i] = cout[i] as u16;
                cout_high[i] = (cout[i] >> 16) as u16;
                zout_low[i] = zout[i] as u16;
                zout_high[i] = (zout[i] >> 16) as u16;

                let carry_val = (cout[i] >> 16) & 1;
                final_carry_low[i] = carry_val as u16;

                if let Some(ref mut final_carry) = final_carry {
                    set_packed_slice(&mut *final_carry, i, if carry { B1::ONE } else { B1::ZERO });
                }
            }
        };
        Ok(())
    }
}
