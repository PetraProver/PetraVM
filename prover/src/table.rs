//! Tables for the PetraVM M3 circuit.
//!
//! This module contains the definitions and abstractions of all the arithmetic
//! tables, to be leveraged by any PetraVM instruction set. Each of these
//! instruction tables are registered through the
//! [`ISA`](petravm_asm::isa::ISA) interface, and are dynamically managed
//! when building the proving circuit.

use anyhow::anyhow;
use binius_m3::builder::ConstraintSystem;
use binius_m3::builder::TableFiller;
use binius_m3::builder::WitnessIndex;
use petravm_asm::opcodes::InstructionInfo;
use tracing::instrument;

use crate::model::Trace;
// Re-export instruction-specific tables
pub use crate::opcodes::*;
use crate::{channels::Channels, types::ProverPackedField};

pub trait TableInfo: InstructionInfo {
    type Table: TableFiller<ProverPackedField> + Table + 'static;

    fn accessor() -> fn(&Trace) -> &[<Self::Table as Table>::Event];
}

/// Trait implemented by all instruction tables in the PetraVM circuit.
///
/// This trait provides table-specific metadata and registration logic,
/// and is used to generically construct tables.
///
/// The associated `Event` type defines the kind of event this table
/// expects during witness generation.
pub trait Table {
    /// The event type associated with this table.
    type Event: 'static;

    // TODO(Robin): Do we need this?
    /// Returns the name of this [`Table`].
    fn name(&self) -> &'static str;

    /// Creates a new [`Table`] with the given constraint system and channels.
    ///
    /// # Arguments
    /// * `cs` - [`ConstraintSystem`] to add the table to
    /// * `channels` - [`Channels`] IDs for communication with other tables
    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self
    where
        Self: Sized;
}

/// Trait use for convenience to easily fill a witness from a provided
/// [`Trace`].
///
/// NOTE: This is necessary to "hide" the associated [`Event`](Table::Event)
/// type of the [`Table`] trait, so that it can be used within the definition of
/// [`ISA`](petravm_asm::isa::ISA).
pub trait FillableTable {
    /// Fills the table's witness rows with data from the corresponding events
    /// prevent in the provided [`Trace`].
    fn fill(
        &self,
        witness: &mut WitnessIndex<'_, '_, ProverPackedField>,
        trace: &Trace,
    ) -> anyhow::Result<()>;

    /// Outputs the number of events associated with the corresponding [`Table`]
    /// in the provided [`Trace`].
    fn num_events(&self, trace: &Trace) -> usize;

    /// Outputs the name of the table.
    fn name(&self) -> &'static str;
}

/// A dynamic table entry that binds a [`Table`] instance with an event
/// accessor, to be used when defining an [`ISA`](petravm_asm::isa::ISA)
/// and to register tables inside a [`Circuit`](crate::circuit::Circuit).
///
/// The underlying table type is a pointer to an instance implementing both
/// [`Table`] and [`TableFiller`] traits.
/// The entry also implements the [`FillableTable`] trait.
pub struct TableEntry<T: Table + TableFiller<ProverPackedField> + 'static> {
    pub table: Box<T>,
    pub get_events: fn(&Trace) -> &[<T as TableFiller<ProverPackedField>>::Event],
}

impl<T> FillableTable for TableEntry<T>
where
    T: Table + TableFiller<ProverPackedField> + 'static,
{
    #[instrument(level = "debug", skip_all, fields(table = %self.table.name()))]
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

    fn name(&self) -> &'static str {
        self.table.name()
    }
}
