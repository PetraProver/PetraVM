use std::array::from_fn;

use binius_core::constraint_system::channel::ChannelId;
use binius_field::underlier::Divisible;
use binius_m3::builder::{Col, TableBuilder, TableWitnessSegment, B32};

use crate::types::ProverPackedField;

/// A gadget for reading a B128 value in VROM with four
/// consecutive lookups.
#[derive(Default)]
pub(crate) struct B128LookupGadget {
    pub(crate) addr: u32,
    pub(crate) val: u128,
}

/// The columns associated with the CPU gadget.
pub(crate) struct B128LookupColumns {
    pub(crate) addr_base: Col<B32>,
    pub(crate) addr_tail: [Col<B32>; 3], // Virtual
    pub(crate) val_cols: [Col<B32>; 4],  // Virtual
}

impl B128LookupColumns {
    pub fn new(
        table: &mut TableBuilder,
        vrom_channel: ChannelId,
        addr_base: Col<B32>,
        val: Col<B32, 4>,
        label: &str,
    ) -> Self {
        let addr_tail = from_fn(|i| {
            table.add_computed(
                format!("{label}_b128_lookup_addr_{}", i + 1),
                addr_base + B32::new(1 + i as u32),
            )
        });
        let val_cols =
            from_fn(|i| table.add_selected(format!("{label}_b128_lookup_val_{}", i), val, i));
        table.pull(vrom_channel, [addr_base, val_cols[0]]);
        for i in 0..3 {
            table.pull(vrom_channel, [addr_tail[i], val_cols[i + 1]]);
        }

        Self {
            addr_base,
            addr_tail,
            val_cols,
        }
    }

    pub fn populate<T>(
        &self,
        index: &mut TableWitnessSegment<ProverPackedField>,
        rows: T,
    ) -> Result<(), anyhow::Error>
    where
        T: Iterator<Item = B128LookupGadget>,
    {
        let mut addr_tail = [
            index.get_scalars_mut(self.addr_tail[0])?,
            index.get_scalars_mut(self.addr_tail[1])?,
            index.get_scalars_mut(self.addr_tail[2])?,
        ];

        let mut val_cols = [
            index.get_scalars_mut(self.val_cols[0])?,
            index.get_scalars_mut(self.val_cols[1])?,
            index.get_scalars_mut(self.val_cols[2])?,
            index.get_scalars_mut(self.val_cols[3])?,
        ];

        for (row, B128LookupGadget { addr, val }) in rows.enumerate() {
            for (i, col) in addr_tail.iter_mut().enumerate() {
                col[row] = B32::new(addr + 1 + i as u32);
            }

            let vals: [u32; 4] = <u128 as Divisible<u32>>::split_val(val);
            for (i, col) in val_cols.iter_mut().enumerate() {
                col[row] = B32::new(vals[i]);
            }
        }

        Ok(())
    }
}
