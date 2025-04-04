//! Tables for the zCrayVM M3 circuit.
//!
//! This module contains the definitions of all the arithmetic tables needed
//! to represent the zCrayVM execution in the M3 arithmetization system.

use std::any::Any;

use binius_m3::builder::ConstraintSystem;
use binius_m3::builder::WitnessIndex;

use crate::model::Trace;
// Re-export instruction-specific tables
pub use crate::opcodes::{LdiTable, RetTable};
use crate::{channels::Channels, types::ProverPackedField};

pub trait Table: Any {
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

    fn fill(
        &self,
        witness: &mut WitnessIndex<'_, '_, ProverPackedField>,
        trace: &Trace,
    ) -> anyhow::Result<()>;
}
