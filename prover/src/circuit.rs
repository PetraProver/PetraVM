//! Circuit definition for the PetraVM proving system.
//!
//! This module defines the complete M3 circuit for PetraVM, combining
//! all the individual tables and channels.

#[cfg(feature = "disable_state_channel")]
use binius_m3::builder::{Boundary, ConstraintSystem, FlushDirection, Statement};
#[cfg(not(feature = "disable_state_channel"))]
use binius_m3::builder::{Boundary, ConstraintSystem, FlushDirection, Statement, B128};
use petravm_asm::isa::ISA;

use crate::{
    channels::Channels,
    gadgets::right_shifter_table::RightShifterTable,
    memory::{PromTable, VromAddrSpaceTable, VromSkipTable, VromWriteTable},
    model::{build_table_for_opcode, Trace},
    table::{FillableTable, Table},
};

/// Arithmetic circuit for the PetraVM proving system.
///
/// This struct represents the complete M3 arithmetization circuit for PetraVM.
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
    /// Right Logical Shifter table
    pub right_shifter_table: RightShifterTable,
    /// Instruction tables
    pub tables: Vec<Box<dyn FillableTable>>,
}

impl Circuit {
    /// Create a new PetraVM circuit.
    ///
    /// This initializes the constraint system, channels, and all tables
    /// needed for the PetraVM execution.
    pub fn new(isa: Box<dyn ISA>) -> Self {
        let mut cs = ConstraintSystem::new();
        let channels = Channels::new(&mut cs);

        // Create all the tables
        let prom_table = PromTable::new(&mut cs, &channels);
        let vrom_write_table = VromWriteTable::new(&mut cs, &channels);
        let vrom_addr_space_table = VromAddrSpaceTable::new(&mut cs, &channels);
        let vrom_skip_table = VromSkipTable::new(&mut cs, &channels);
        let right_shifter_table = RightShifterTable::new(&mut cs, &channels);

        // Generate all tables required to prove the instructions supported by this ISA.
        // Sort the opcodes to ensure deterministic table creation
        let mut sorted_opcodes = isa.supported_opcodes().iter().copied().collect::<Vec<_>>();
        sorted_opcodes.sort_by_key(|op| *op as u16);
        let tables = sorted_opcodes
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
            right_shifter_table,
            tables,
        }
    }

    /// Create a circuit statement for a given trace.
    ///
    /// # Arguments
    /// * `trace` - The PetraVM execution trace
    ///
    /// # Returns
    /// * A Statement that defines boundaries and table sizes
    pub fn create_statement(&self, trace: &Trace) -> anyhow::Result<Statement> {
        // Build the statement with boundary values

        // Define the initial state boundary (program starts at PC=1, FP=0)
        #[cfg(not(feature = "disable_state_channel"))]
        let init_values = vec![B128::new(1), B128::new(0)];
        #[cfg(feature = "disable_state_channel")]
        let init_values = vec![];
        let initial_state = Boundary {
            values: init_values,
            channel_id: self.channels.state_channel,
            direction: FlushDirection::Push,
            multiplicity: 1,
        };

        // Define the final state boundary (program ends with PC=0, FP=0)
        #[cfg(not(feature = "disable_state_channel"))]
        let final_values = vec![B128::new(0), B128::new(0)];
        #[cfg(feature = "disable_state_channel")]
        let final_values = vec![];
        let final_state = Boundary {
            values: final_values,
            channel_id: self.channels.state_channel,
            direction: FlushDirection::Pull,
            multiplicity: 1,
        };

        let prom_size = trace.program.len();

        // By adding 1 to `max_vrom_addr`, `next_power_of_two()` will advance to the
        // next power of two even when `max_vrom_addr` is already a power of two,
        // ensuring the VROM address space includes the highest address.
        let vrom_addr_space_size = (trace.max_vrom_addr + 1).next_power_of_two();

        // VROM write size is the number of addresses we write to
        let vrom_write_size = trace.vrom_writes.len();

        // VROM skip size is the number of addresses we skip
        let vrom_skip_size = vrom_addr_space_size - vrom_write_size;

        // Size of the right shifter table is the number of right shift events
        let right_shifter_size = trace.right_shift_events().len();

        // Define the table sizes in order of table creation
        let mut table_sizes = vec![
            prom_size,            // PROM table size
            vrom_write_size,      // VROM write table size
            vrom_addr_space_size, // VROM address space table size
            vrom_skip_size,       // VROM skip table size
            right_shifter_size,   // Right shifter table size
        ];

        // Add table sizes for each supported instruction
        for table in &self.tables {
            let num_events = table.num_events(trace);
            log::debug!(
                "Number of events for table {}: {}",
                table.name(),
                num_events
            );
            table_sizes.push(num_events);
        }

        // Create the statement with all boundaries
        let statement = Statement {
            boundaries: vec![initial_state, final_state],
            table_sizes,
        };

        Ok(statement)
    }
}
