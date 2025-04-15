//! Circuit definition for the zCrayVM proving system.
//!
//! This module defines the complete M3 circuit for zCrayVM, combining
//! all the individual tables and channels.

use binius_m3::builder::{Boundary, ConstraintSystem, FlushDirection, Statement, B128};

use crate::{
    channels::Channels,
    model::Trace,
    opcodes::SrliTable,
    tables::{
        B32MulTable, BnzTable, BzTable, LdiTable, PromTable, RetTable, VromAddrSpaceTable,
        VromSkipTable, VromWriteTable,
    },
};

/// Arithmetic circuit for the zCrayVM proving system.
///
/// This struct represents the complete M3 arithmetization circuit for zCrayVM.
/// It contains all the tables and channels needed to encode program execution
/// as arithmetic constraints.
pub struct Circuit {
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
    /// LDI instruction table
    pub ldi_table: LdiTable,
    /// RET instruction table
    pub ret_table: RetTable,
    /// B32_MUL instruction table
    pub b32_mul_table: B32MulTable,
    /// BNZ branch non-zero instruction table
    pub bnz_table: BnzTable,
    /// BNZ branch zero instruction table
    pub bz_table: BzTable,
    /// SRLI shift right immediate logical instruction table
    pub srli_table: SrliTable,
}

impl Default for Circuit {
    fn default() -> Self {
        Self::new()
    }
}

impl Circuit {
    /// Create a new zCrayVM circuit.
    ///
    /// This initializes the constraint system, channels, and all tables
    /// needed for the zCrayVM execution.
    pub fn new() -> Self {
        let mut cs = ConstraintSystem::new();
        let channels = Channels::new(&mut cs);

        // Create all the tables
        let vrom_write_table = VromWriteTable::new(&mut cs, &channels);
        let prom_table = PromTable::new(&mut cs, &channels);
        let vrom_addr_space_table = VromAddrSpaceTable::new(&mut cs, &channels);
        let vrom_skip_table = VromSkipTable::new(&mut cs, &channels);
        let ldi_table = LdiTable::new(&mut cs, &channels);
        let ret_table = RetTable::new(&mut cs, &channels);
        let b32_mul_table = B32MulTable::new(&mut cs, &channels);
        let bnz_table = BnzTable::new(&mut cs, &channels);
        let bz_table = BzTable::new(&mut cs, &channels);
        let srli_table = SrliTable::new(&mut cs, &channels);

        Self {
            cs,
            channels,
            vrom_write_table,
            prom_table,
            vrom_addr_space_table,
            vrom_skip_table,
            ldi_table,
            ret_table,
            b32_mul_table,
            bnz_table,
            bz_table,
            srli_table,
        }
    }

    /// Create a circuit statement for a given trace.
    ///
    /// # Arguments
    /// * `trace` - The zCrayVM execution trace
    /// * `vrom_size` - Size of the VROM address space (must be a power of 2)
    ///
    /// # Returns
    /// * A Statement that defines boundaries and table sizes
    pub fn create_statement(&self, trace: &Trace) -> anyhow::Result<Statement> {
        let vrom_size = trace.trace.vrom_size().next_power_of_two();

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

        // Use the provided VROM address space size
        let vrom_addr_space_size = vrom_size;

        // VROM write size is the number of addresses we write to
        let vrom_write_size = trace.vrom_writes.len();

        // VROM skip size is the number of addresses we skip
        let vrom_skip_size = vrom_addr_space_size - vrom_write_size;

        let ldi_size = trace.ldi_events().len();
        let ret_size = trace.ret_events().len();
        let b32_mul_size = trace.b32_mul_events().len();
        let bnz_size = trace.bnz_events().len();
        let bz_size = trace.bz_events().len();
        let srli_size = trace.srli_events().collect::<Vec<_>>().len();
        println!("SRLI size: {}", srli_size);

        // Define the table sizes in order of table creation
        let table_sizes = vec![
            vrom_write_size,      // VROM write table size
            prom_size,            // PROM table size
            vrom_addr_space_size, // VROM address space table size
            vrom_skip_size,       // VROM skip table size
            ldi_size,             // LDI table size
            ret_size,             // RET table size
            b32_mul_size,         // B32_MUL table size
            bnz_size,             // BNZ table size
            bz_size,              // BZ table size
            srli_size,            // SRLI table size
        ];

        // Create the statement with all boundaries
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
    ) -> anyhow::Result<binius_core::constraint_system::ConstraintSystem<B128>> {
        Ok(self.cs.compile(statement)?)
    }
}
