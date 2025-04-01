//! Circuit definition for the zCrayVM proving system.
//!
//! This module defines the complete M3 circuit for zCrayVM, combining
//! all the individual tables and channels.

use binius_m3::builder::{Boundary, ConstraintSystem, FlushDirection, Statement, B128};

use crate::{
    channels::ZkVMChannels,
    model::ZkVMTrace,
    opcodes::{
        binary_ops::b32::AndiTable,
        branch::{BnzTable, BzTable},
        integer_ops::AddTable,
        XoriTable,
    },
    tables::{LdiTable, PromTable, RetTable, VromAddrSpaceTable, VromSkipTable, VromWriteTable},
};

/// Complete zCrayVM circuit with all tables for the proving system.
pub struct ZkVMCircuit {
    /// Constraint system
    pub cs: ConstraintSystem,
    /// Channels for connecting tables
    pub channels: ZkVMChannels,
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
    /// BNZ non zero branch instruction
    pub bnz_table: BnzTable,
    /// ADDD instruction table
    pub add_table: AddTable,
    /// BNZ zero branch instruction
    pub bz_table: BzTable,
    /// XORI instruction table
    pub xori_table: XoriTable,
    /// ANDI instruction table
    pub andi_table: AndiTable,
}

impl Default for ZkVMCircuit {
    fn default() -> Self {
        Self::new()
    }
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
        let vrom_addr_space_table = VromAddrSpaceTable::new(&mut cs, &channels);
        let vrom_write_table = VromWriteTable::new(&mut cs, &channels);
        let vrom_skip_table = VromSkipTable::new(&mut cs, &channels);
        let ldi_table = LdiTable::new(&mut cs, &channels);
        let ret_table = RetTable::new(&mut cs, &channels);
        let add_table = AddTable::new(&mut cs, &channels);
        let bnz_table = BnzTable::new(&mut cs, &channels);
        let bz_table = BzTable::new(&mut cs, &channels);
        let xori_table = XoriTable::new(&mut cs, &channels);
        let andi_table = AndiTable::new(&mut cs, &channels);

        Self {
            cs,
            channels,
            prom_table,
            vrom_addr_space_table,
            vrom_write_table,
            vrom_skip_table,
            ldi_table,
            ret_table,
            bnz_table,
            bz_table,
            xori_table,
            add_table,
            andi_table,
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
    pub fn create_statement(&self, trace: &ZkVMTrace) -> anyhow::Result<Statement> {
        let vrom_size = trace.trace.vrom_size().next_power_of_two();

        // Build the statement with boundary values

        // Define the initial state boundary (program starts at PC=1, FP=0)
        let initial_state = Boundary {
            // first_pc = 1, first_fp = 0
            //                                       |..pc..||..fp..|
            values: vec![B128::new(0x00000000000000000000000100000000)],
            channel_id: self.channels.state_channel,
            direction: FlushDirection::Push,
            multiplicity: 1,
        };

        // Define the final state boundary (program ends with PC=0, FP=0)
        let final_state = Boundary {
            values: vec![B128::new(0)],
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
        let add_size = trace.add_events().len(); // TODO: We need the add_events() function?
        let xori_size = trace.trace.xori.len();
        let bnz_size = trace.trace.bnz.len();
        let bz_size = trace.trace.bz.len();
        let andi_size = trace.trace.andi.len();

        // Define the table sizes in order of table creation
        let table_sizes = vec![
            prom_size,            // PROM table size
            vrom_addr_space_size, // VROM address space table size
            vrom_write_size,      // VROM write table size
            vrom_skip_size,       // VROM skip table size
            ldi_size,             // LDI table size
            ret_size,             // RET table size
            add_size,             // ADD table size
            bnz_size,             // BNZ !=0 table size
            bz_size,              // BNZ 0 table size
            xori_size,            // XORI table size
            andi_size,            // ANDI table size
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
    ) -> anyhow::Result<
        binius_core::constraint_system::ConstraintSystem<binius_field::BinaryField128b>,
    > {
        Ok(self.cs.compile(statement)?)
    }
}
