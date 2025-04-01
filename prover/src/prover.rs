//! Main prover interface for zCrayVM.
//!
//! This module provides the main entry point for creating proofs from
//! zCrayVM execution traces.

use anyhow::Result;
use binius_core::constraint_system::validate;
use binius_field::arch::OptimalUnderlier128b;
use bumpalo::Bump;
use groestl_crypto::Groestl256;

use crate::{circuit::ZkVMCircuit, model::ZkVMTrace};

/// Main prover for zCrayVM.
// TODO: should be customizable by supported opcodes
pub struct ZkVMProver {
    /// Arithmetic circuit for zCrayVM
    circuit: ZkVMCircuit,
}

impl Default for ZkVMProver {
    fn default() -> Self {
        Self::new()
    }
}

impl ZkVMProver {
    /// Create a new zCrayVM prover.
    pub fn new() -> Self {
        Self {
            circuit: ZkVMCircuit::new(),
        }
    }

    /// Validate a zCrayVM execution trace.
    ///
    /// This function:
    /// 1. Creates a statement from the trace
    /// 2. Compiles the constraint system
    /// 3. Builds and fills the witness
    /// 4. Validates the witness against the constraints
    ///
    /// # Arguments
    /// * `trace` - The zCrayVM execution trace to validate
    ///
    /// # Returns
    /// * Result containing success or error
    pub fn validate(&self, trace: &ZkVMTrace) -> Result<()> {
        // Create a statement from the trace
        let statement = self.circuit.create_statement(trace)?;

        // Compile the constraint system
        let compiled_cs = self.circuit.compile(&statement)?;

        // Create a memory allocator for the witness
        let allocator = Bump::new();

        // Build the witness structure
        let mut witness = self
            .circuit
            .cs
            .build_witness::<OptimalUnderlier128b>(&allocator, &statement)?;

        // Fill all table witnesses in sequence

        // 1. Fill PROM table with program instructions
        witness.fill_table_sequential(&self.circuit.prom_table, &trace.program)?;

        // 2. Fill VROM address space table with the full address space
        let vrom_size = trace.trace.vrom_size().next_power_of_two();
        let vrom_addr_space: Vec<u32> = (0..vrom_size as u32).collect();
        witness.fill_table_sequential(&self.circuit.vrom_addr_space_table, &vrom_addr_space)?;

        // 3. Fill VROM write table with writes
        witness.fill_table_sequential(&self.circuit.vrom_write_table, &trace.vrom_writes)?;

        // 4. Fill VROM skip table with skipped addresses
        // Generate the list of skipped addresses (addresses not in vrom_writes)
        let write_addrs: std::collections::HashSet<u32> =
            trace.vrom_writes.iter().map(|(addr, _)| *addr).collect();

        let vrom_skips: Vec<u32> = (0..vrom_size as u32)
            .filter(|addr| !write_addrs.contains(addr))
            .collect();

        witness.fill_table_sequential(&self.circuit.vrom_skip_table, &vrom_skips)?;

        // 5. Fill LDI table with load immediate events
        witness.fill_table_sequential(&self.circuit.ldi_table, trace.ldi_events())?;

        // 6. Fill RET table with return events
        witness.fill_table_sequential(&self.circuit.ret_table, trace.ret_events())?;

        // 7. Fill ADD table with return events
        witness.fill_table_sequential(&self.circuit.add_table, trace.add_events())?;

        // 8. Fill BNZ non zero branch
        witness.fill_table_sequential(&self.circuit.bnz_table, trace.bnz_events())?;

        // 9. Fill BNZ zero branch
        witness.fill_table_sequential(&self.circuit.bz_table, trace.bz_events())?;

        // 10. Fill XORI table
        witness.fill_table_sequential(&self.circuit.xori_table, trace.xori_events())?;

        // 11. Fill ANDI table
        witness.fill_table_sequential(&self.circuit.andi_table, trace.andi_events())?;

        // Convert witness to multilinear extension format for validation
        let mle_witness = witness.into_multilinear_extension_index(&statement);

        // Validate the witness against the constraint system
        validate::validate_witness(&compiled_cs, &statement.boundaries, &mle_witness)?;

        Ok(())
    }
}
