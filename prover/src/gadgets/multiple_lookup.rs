use std::array::from_fn;

use binius_core::constraint_system::channel::ChannelId;
use binius_m3::builder::{Col, TableBuilder, TableWitnessSegment, B32};

use crate::types::ProverPackedField;

/// A gadget for reading a large value in memory with multiple
/// consecutive B32 lookups.
pub(crate) struct MultipleLookupGadget<const N: usize> {
    /// The address of the first lookup.
    pub(crate) addr: u32,
    /// The full value of the lookup.
    pub(crate) vals: [u32; N],
}

/// The columns associated with the multiple lookup gadget.
pub(crate) struct MultipleLookupColumns<const N: usize> {
    pub(crate) addr_cols: [Col<B32>; N], // Virtual
    pub(crate) val_cols: [Col<B32>; N],  // Virtual
}

impl<const N: usize> MultipleLookupColumns<N> {
    pub fn new(
        table: &mut TableBuilder,
        channel: ChannelId,
        addr_base: Col<B32>,
        val: Col<B32, N>,
        label: &str,
    ) -> Self {
        let addr_cols = from_fn(|i| {
            if i == 0 {
                addr_base
            } else {
                table.add_computed(
                    format!("{label}_b{}_lookup_addr_{}", 32 * N, i),
                    addr_base + B32::new(i as u32),
                )
            }
        });
        let val_cols = from_fn(|i| {
            table.add_selected(format!("{label}_b{}_lookup_val_{}", 32 * N, i), val, i)
        });
        for i in 0..N {
            table.pull(channel, [addr_cols[i], val_cols[i]]);
        }

        Self {
            addr_cols,
            val_cols,
        }
    }

    pub fn populate<T>(
        &self,
        index: &mut TableWitnessSegment<ProverPackedField>,
        rows: T,
    ) -> anyhow::Result<()>
    where
        T: Iterator<Item = MultipleLookupGadget<N>>,
    {
        let mut addr_cols = (0..N)
            .map(|i| {
                index
                    .get_mut_as(self.addr_cols[i])
                    .map_err(anyhow::Error::new)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut val_cols = (0..N)
            .map(|i| {
                index
                    .get_mut_as(self.val_cols[i])
                    .map_err(anyhow::Error::new)
            })
            .collect::<Result<Vec<_>, _>>()?;

        for (row, MultipleLookupGadget { addr, vals }) in rows.enumerate() {
            for (i, col) in addr_cols.iter_mut().enumerate() {
                col[row] = B32::new(addr + i as u32);
            }

            for (i, col) in val_cols.iter_mut().enumerate() {
                col[row] = B32::new(vals[i]);
            }
        }

        Ok(())
    }
}
