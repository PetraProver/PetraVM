use binius_core::oracle::ShiftVariant;
use binius_field::{
    packed::set_packed_slice, ExtensionField, Field, PackedExtension, PackedFieldIndexable,
};
use binius_m3::builder::B16;
use binius_m3::builder::{
    column::Col, types::B1, upcast_col, witness::TableWitnessSegment, TableBuilder, B128, B32,
};

use crate::utils::pack_b16_into_b32;

/// A gadget for performing 32-bit integer addition on vertically-packed bit
/// columns.
///
/// This gadget has input columns `xin` and `yin` for the two 32-bit integers to
/// be added, and an output column `zout`, and it constrains that `xin + yin =
/// zout` as integers.
#[derive(Debug)]
pub struct U32U16Add {
    // Inputs
    pub xin: Col<B1, 32>,
    pub yin: Col<B1, 16>,

    // Private
    cin: Col<B1, 32>,
    cout: Col<B1, 32>,
    cout_shl: Col<B1, 32>,

    // Outputs
    /// The output column, either committed if `flags.commit_zout` is set,
    /// otherwise a linear combination derived column.
    pub zout: Col<B32, 1>,
    /// This is `Some` if `flags.expose_final_carry` is set, otherwise it is
    /// `None`.
    pub final_carry: Option<Col<B1>>,
    /// Flags modifying the gadget's behavior.
    pub flags: U32AddFlags,
}

/// Flags modifying the behavior of the [`U32Add`] gadget.
#[derive(Debug, Default, Clone)]
pub struct U32AddFlags {
    // Optionally a column for a dynamic carry in bit. This *must* be zero in all bits except the
    // 0th.
    pub carry_in_bit: Option<Col<B1, 32>>,
    pub commit_zout: bool,
    pub expose_final_carry: bool,
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

        // table.add_selected(name, col, index)
        let query_low = 0b0 as usize;
        let query_high = 0b1 as usize;
        let start_index = 4;
        let xin_low = table.add_projected("xin_low", xin, 1, query_low, start_index);
        let xin_high: Col<B1, 16> =
            table.add_projected("xin_high", xin, 1, query_high, start_index);

        let cout_low = table.add_projected("cin_low", cout, 1, query_low, start_index);
        let cout_high = table.add_projected("cin_high", cout, 1, query_high, start_index);
        let final_carry_low = table.add_projected("final carry low", cout_low, 1, query_low, 0);

        let cin_low = table.add_projected("cin_low", cin, 1, query_low, start_index);
        let cin_high_bits = table.add_projected("cin_high", cin, 1, query_high, start_index);
        let cin_high = table.add_computed("cin high full", cin_high_bits + final_carry_low);

        table.assert_zero(
            "carry_out_low",
            (xin_low + cin_low) * (yin + cin_low) + cin_low - cout_low,
        );
        table.assert_zero(
            "carry_out_high",
            (xin_high + cin_high) * cin_high + cin_high - cout_high,
        );

        let zout = if flags.commit_zout {
            let zout = table.add_committed::<B1, 32>("zout");
            let zout_low = table.add_projected("zout_low", zout, 1, query_low, start_index);
            let zout_high = table.add_projected("zout high", zout, 1, query_high, start_index);
            table.assert_zero("zout_low", xin_low + yin + cin_low - zout_low);
            table.assert_zero("zout high", xin_high + cin_high - zout_high);
            table.add_packed("zout_packed", zout)
        } else {
            let zout_low = table.add_computed("zout_low", xin_low + yin + cin_low);
            let zout_low_packed: Col<B16> = table.add_packed("zout low packed", zout_low);
            let zout_high = table.add_computed("zout high", xin_high + cin_high);
            let zout_high_packed: Col<B16> = table.add_packed("zout high packed", zout_high);
            table.add_computed(
                "zout",
                upcast_col(zout_low_packed)
                    + upcast_col(zout_high_packed) * <B32 as ExtensionField<B16>>::basis(1),
            )
        };

        Self {
            xin,
            yin,
            cin,
            cout,
            cout_shl,
            final_carry,
            zout,
            flags,
        }
    }

    pub fn populate<P>(&self, index: &mut TableWitnessSegment<P>) -> Result<(), anyhow::Error>
    where
        P: PackedFieldIndexable<Scalar = B128> + PackedExtension<B1>,
    {
        let xin: std::cell::RefMut<'_, [u32]> = index.get_mut_as(self.xin)?;
        let yin = index.get_mut_as(self.yin)?;
        let mut cout = index.get_mut_as(self.cout)?;
        let mut zout = index.get_mut_as(self.zout)?;
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
                let (x_plus_y, carry0) = xin[i].overflowing_add(yin[i]);
                let carry1;
                (zout[i], carry1) = x_plus_y.overflowing_add(carry_in_bit[i]);
                let carry = carry0 | carry1;

                cin[i] = xin[i] ^ yin[i] ^ zout[i];
                cout[i] = (carry as u32) << 31 | cin[i] >> 1;
                cout_shl[i] = cout[i] << 1;

                if let Some(ref mut final_carry) = final_carry {
                    set_packed_slice(&mut *final_carry, i, if carry { B1::ONE } else { B1::ZERO });
                }
            }
        } else {
            // When the carry in bit is fixed to zero, we can simplify the logic.
            let mut cin = index.get_mut_as(self.cin)?;
            for i in 0..index.size() {
                let carry;
                (zout[i], carry) = xin[i].overflowing_add(yin[i]);
                cin[i] = xin[i] ^ yin[i] ^ zout[i];
                cout[i] = (carry as u32) << 31 | cin[i] >> 1;
                if let Some(ref mut final_carry) = final_carry {
                    set_packed_slice(&mut *final_carry, i, if carry { B1::ONE } else { B1::ZERO });
                }
            }
        };
        Ok(())
    }
}
