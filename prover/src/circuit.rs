//! Circuit definition for the zCrayVM proving system.
//!
//! This module defines the complete M3 circuit for zCrayVM, combining
//! all the individual tables and channels.

use binius_m3::builder::{Boundary, ConstraintSystem, FlushDirection, Statement, B128};
use zcrayvm_assembly::isa::ISA;

use crate::{
    channels::Channels,
    memory::{PromTable, VromAddrSpaceTable, VromSkipTable, VromWriteTable},
    model::{build_table_for_opcode, Trace},
    table::FillableTable,
};

/// Arithmetic circuit for the zCrayVM proving system.
///
/// This struct represents the complete M3 arithmetization circuit for zCrayVM.
/// It contains all the tables and channels needed to encode program execution
/// as arithmetic constraints.
pub struct Circuit {
    /// The Instruction Set Architecture [`ISA`] targeted for this [`Circuit`]
    /// instance.
    pub isa: Box<dyn ISA>,
    /// Constraint system
    pub cs: ConstraintSystem,
    /// Channels for connecting tables
    pub channels: Channels,
    /// Program ROM table
    pub prom_table: PromTable,
    // TODO: We should not have this table in prover
    /// VROM address space table
    pub vrom_addr_space_table: VromAddrSpaceTable,
    /// VROM Write table
    pub vrom_write_table: VromWriteTable,
    /// VROM Skip table
    pub vrom_skip_table: VromSkipTable,
    /// Instruction tables
    pub tables: Vec<Box<dyn FillableTable>>,
}

impl Circuit {
    /// Create a new zCrayVM circuit.
    ///
    /// This initializes the constraint system, channels, and all tables
    /// needed for the zCrayVM execution.
    pub fn new(isa: Box<dyn ISA>) -> Self {
        let mut cs = ConstraintSystem::new();
        let channels = Channels::new(&mut cs);

        // Create all the tables
        let prom_table = PromTable::new(&mut cs, &channels);
        let vrom_write_table = VromWriteTable::new(&mut cs, &channels);
        let vrom_addr_space_table = VromAddrSpaceTable::new(&mut cs, &channels);
        let vrom_skip_table = VromSkipTable::new(&mut cs, &channels);

        // Generate all tables required to prove the instructions supported by this ISA.
        let tables = isa
            .supported_opcodes()
            .iter()
            .filter_map(|op| build_table_for_opcode(*op, &mut cs, &channels))
            .collect::<Vec<_>>();

        Self {
            isa,
            cs,
            channels,
            prom_table,
            vrom_write_table,
            vrom_addr_space_table,
            vrom_skip_table,
            tables,
        }
    }

    /// Create a circuit statement for a given trace.
    ///
    /// # Arguments
    /// * `trace` - The zCrayVM execution trace
    ///
    /// # Returns
    /// * A Statement that defines boundaries and table sizes
    pub fn create_statement(&self, trace: &Trace) -> anyhow::Result<Statement> {
        // Build the statement with boundary values

        // Define the initial state boundary (program starts at PC=1, FP=0)
        let initial_state = Boundary {
            values: vec![B128::new(1), B128::new(0)],
            channel_id: self.channels.state_channel,
            direction: FlushDirection::Push,
            multiplicity: 1,
        };

        // Define the final state boundary (program ends with PC=0, FP=0)
        let final_state = Boundary {
            values: vec![B128::new(0), B128::new(0)],
            channel_id: self.channels.state_channel,
            direction: FlushDirection::Pull,
            multiplicity: 1,
        };

        let prom_size = trace.program.len();

        let vrom_addr_space_size = trace.max_vrom_addr.next_power_of_two();

        // VROM write size is the number of addresses we write to
        let vrom_write_size = trace.vrom_writes.len();

        // VROM skip size is the number of addresses we skip
        let vrom_skip_size = vrom_addr_space_size - vrom_write_size;

        // Define the table sizes in order of table creation
        let mut table_sizes = vec![
            vrom_write_size,      // VROM write table size
            prom_size,            // PROM table size
            vrom_addr_space_size, // VROM address space table size
            vrom_skip_size,       // VROM skip table size
        ];

        // Add table sizes for each supported instruction
        for table in &self.tables {
            table_sizes.push(table.num_events(trace));
        }

        // Create the statement with all boundaries
        let statement = Statement {
            boundaries: vec![initial_state, final_state],
            table_sizes,
        };

        Ok(statement)
    }
}
