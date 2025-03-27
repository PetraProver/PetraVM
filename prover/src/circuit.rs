//! Circuit definition for the zCrayVM proving system.
//!
//! This module defines the complete M3 circuit for zCrayVM, combining
//! all the individual tables and channels.

use binius_field::{BinaryField, BinaryField32b, Field};
use binius_m3::builder::{Boundary, ConstraintSystem, FlushDirection, Statement, B128};

use crate::{
    channels::ZkVMChannels,
    model::ZkVMTrace,
    tables::{LdiTable, PromTable, RetTable, VromTable},
};

/// Complete zCrayVM circuit with all tables for the proving system.
pub struct ZkVMCircuit {
    /// Constraint system
    pub cs: ConstraintSystem,
    /// Channels for connecting tables
    pub channels: ZkVMChannels,
    /// Program ROM table
    pub prom_table: PromTable,
    /// Value ROM table
    pub vrom_table: VromTable,
    /// LDI instruction table
    pub ldi_table: LdiTable,
    /// RET instruction table
    pub ret_table: RetTable,
}

impl ZkVMCircuit {
    /// Create a new zCrayVM circuit.
    ///
    /// This initializes the constraint system, channels, and all tables
    /// needed for the zCrayVM execution.
    pub fn new() -> Self {
        let mut cs = ConstraintSystem::new();
        let channels = ZkVMChannels::new(&mut cs);

        // Create all the tables
        let prom_table = PromTable::new(&mut cs, &channels);
        let vrom_table = VromTable::new(&mut cs, &channels);
        let ldi_table = LdiTable::new(&mut cs, &channels);
        let ret_table = RetTable::new(&mut cs, &channels);

        Self {
            cs,
            channels,
            prom_table,
            vrom_table,
            ldi_table,
            ret_table,
        }
    }

    /// Create a circuit statement for a given trace.
    ///
    /// # Arguments
    /// * `trace` - The zCrayVM execution trace
    ///
    /// # Returns
    /// * A Statement that defines boundaries and table sizes
    pub fn create_statement(&self, trace: &ZkVMTrace) -> anyhow::Result<Statement> {
        // Build the statement with boundary values
        let generator = BinaryField32b::MULTIPLICATIVE_GENERATOR;

        // Define the initial state boundary (program starts at PC=G, FP=0)
        let initial_state = Boundary {
            values: vec![
                B128::from(generator),            // Initial PC = G
                B128::from(BinaryField32b::ZERO), // Initial FP = 0
            ],
            channel_id: self.channels.state_channel,
            direction: FlushDirection::Push,
            multiplicity: 1,
        };

        // Define the final state boundary (program ends with PC=0, FP=0)
        let final_state = Boundary {
            values: vec![
                B128::from(BinaryField32b::ZERO), // Final PC = 0
                B128::from(BinaryField32b::ZERO), // Final FP = 0
            ],
            channel_id: self.channels.state_channel,
            direction: FlushDirection::Pull,
            multiplicity: 1,
        };

        // Calculate more accurate table sizes
        let prom_size = trace.program.len();

        // VROM entries = initial values (2) + LDI writes (1 per LDI) + RET reads (2 per
        // RET)
        let vrom_size = 2 + trace.ldi_events().len() + (2 * trace.ret_events().len());

        let ldi_size = trace.ldi_events().len();
        let ret_size = trace.ret_events().len();

        // Define the table sizes in order of table creation
        let table_sizes = vec![
            prom_size, // PROM table size
            vrom_size, // VROM table size
            ldi_size,  // LDI table size
            ret_size,  // RET table size
        ];

        // Create the statement
        let statement = Statement {
            boundaries: vec![initial_state, final_state],
            table_sizes,
        };

        Ok(statement)
    }

    /// Compile the circuit with a given statement.
    ///
    /// # Arguments
    /// * `statement` - The statement to compile
    ///
    /// # Returns
    /// * A compiled constraint system
    pub fn compile(
        &self,
        statement: &Statement,
    ) -> anyhow::Result<
        binius_core::constraint_system::ConstraintSystem<binius_field::BinaryField128b>,
    > {
        Ok(self.cs.compile(statement)?)
    }
}
