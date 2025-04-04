//! Tables for the zCrayVM M3 circuit.
//!
//! This module contains the definitions of all the arithmetic tables needed
//! to represent the zCrayVM execution in the M3 arithmetization system.

use std::any::Any;

use anyhow::anyhow;
use binius_m3::builder::ConstraintSystem;
use binius_m3::builder::TableFiller;
use binius_m3::builder::WitnessIndex;

use crate::model::Trace;
// Re-export instruction-specific tables
pub use crate::opcodes::{LdiTable, RetTable};
use crate::{channels::Channels, types::ProverPackedField};

pub trait Table: Any {
    type Event: 'static;

    fn name(&self) -> &'static str;

    /// Creates a new table with the given constraint system and channels.
    ///
    /// # Arguments
    /// * `cs` - Constraint system to add the table to
    /// * `channels` - Channel IDs for communication with other tables
    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self
    where
        Self: Sized;

    fn as_any(&self) -> &dyn Any;
}

pub trait Fill {
    fn fill(
        &self,
        witness: &mut WitnessIndex<'_, '_, ProverPackedField>,
        trace: &Trace,
    ) -> anyhow::Result<()>;

    fn num_events(&self, trace: &Trace) -> usize;
}

impl<T> Fill for TableEntry<T>
where
    T: TableFiller<ProverPackedField> + 'static,
{
    fn fill(
        &self,
        witness: &mut WitnessIndex<'_, '_, ProverPackedField>,
        trace: &Trace,
    ) -> anyhow::Result<()> {
        witness
            .fill_table_sequential(&*self.table, (self.get_events)(trace))
            .map_err(|e| anyhow!(e))
    }

    fn num_events(&self, trace: &Trace) -> usize {
        (self.get_events)(trace).len()
    }
}

pub struct TableEntry<T: TableFiller<ProverPackedField> + 'static> {
    pub table: Box<T>,
    pub get_events: fn(&Trace) -> &[T::Event],
}
