//! Main prover interface for PetraVM.
//!
//! This module provides the main entry point for creating proofs from
//! PetraVM execution traces.

use anyhow::{anyhow, Result};
use binius_core::{
    constraint_system::{prove, verify, ConstraintSystem, Proof},
    fiat_shamir::HasherChallenger,
};
use binius_field::arch::OptimalUnderlier128b;
use binius_field::tower::CanonicalTowerFamily;
use binius_hal::make_portable_backend;
use binius_hash::groestl::{Groestl256, Groestl256ByteCompression};
use binius_m3::builder::{Statement, TableFiller, B128};
use bumpalo::Bump;
use petravm_asm::isa::ISA;

use crate::{circuit::Circuit, model::Trace, types::ProverPackedField};

const LOG_INV_RATE: usize = 1;
const SECURITY_BITS: usize = 100;
pub(crate) const PROM_MULTIPLICITY_BITS: usize = 32;
pub(crate) const VROM_MULTIPLICITY_BITS: usize = 8;

/// Main prover for PetraVM.
pub struct Prover {
    /// Arithmetic circuit for PetraVM
    circuit: Circuit,
}

impl Prover {
    /// Create a new PetraVM prover.
    pub fn new(isa: Box<dyn ISA>) -> Self {
        Self {
            circuit: Circuit::new(isa),
        }
    }

    // TODO: Split witness generation from actual proving?

    /// Prove a PetraVM execution trace.
    ///
    /// This function:
    /// 1. Creates a statement from the trace
    /// 2. Compiles the constraint system
    /// 3. Builds and fills the witness
    /// 4. Validates the witness against the constraints (in debug mode only)
    /// 5. Generates a proof
    ///
    /// # Arguments
    /// * `trace` - The PetraVM execution trace to prove
    ///
    /// # Returns
    /// * Result containing the proof, statement, and compiled constraint system
    pub fn prove(&self, trace: &Trace) -> Result<(Proof, Statement, ConstraintSystem<B128>)> {
        // Create a statement from the trace
        let statement = self.circuit.create_statement(trace)?;

        // Compile the constraint system
        let compiled_cs = self
            .circuit
            .cs
            .compile(&statement)
            .map_err(|e| anyhow!(e))?;

        // Create a memory allocator for the witness
        let allocator = Bump::new();

        // Build the witness structure
        let mut witness = self
            .circuit
            .cs
            .build_witness::<ProverPackedField>(&allocator);

        // Fill all table witnesses in sequence

        // 1. Fill PROM table with program instructions
        witness.fill_table_sequential(&self.circuit.prom_table, &trace.program)?;

        // 2. Fill VROM address space table with the full address space
        let vrom_addr_space_size = statement.table_sizes[self.circuit.vrom_addr_space_table.id()];
        let vrom_addr_space: Vec<u32> = (0..vrom_addr_space_size as u32).collect();
        witness.fill_table_sequential(&self.circuit.vrom_addr_space_table, &vrom_addr_space)?;

        // 3. Fill VROM write table with writes
        witness.fill_table_sequential(&self.circuit.vrom_write_table, &trace.vrom_writes)?;

        // 4. Fill VROM skip table with skipped addresses
        // Generate the list of skipped addresses (addresses not in vrom_writes)
        let write_addrs: std::collections::HashSet<u32> =
            trace.vrom_writes.iter().map(|(addr, _, _)| *addr).collect();

        let vrom_skips: Vec<u32> = (0..vrom_addr_space_size as u32)
            .filter(|addr| !write_addrs.contains(addr))
            .collect();

        witness.fill_table_sequential(&self.circuit.vrom_skip_table, &vrom_skips)?;

        // 5. Fill all event tables
        for table in &self.circuit.tables {
            table.fill(&mut witness, trace)?;
        }

        // Convert witness to multilinear extension format
        let witness = witness.into_multilinear_extension_index();

        // Validate the witness against the constraint system in debug mode only
        #[cfg(debug_assertions)]
        binius_core::constraint_system::validate::validate_witness(
            &compiled_cs,
            &statement.boundaries,
            &witness,
        )?;

        // Generate the proof
        let proof = prove::<
            OptimalUnderlier128b,
            CanonicalTowerFamily,
            Groestl256,
            Groestl256ByteCompression,
            HasherChallenger<Groestl256>,
            _,
        >(
            &compiled_cs,
            LOG_INV_RATE,
            SECURITY_BITS,
            &statement.boundaries,
            witness,
            &make_portable_backend(),
        )?;

        Ok((proof, statement, compiled_cs))
    }

    /// Validate a PetraVM execution trace.
    #[cfg(test)]
    pub fn validate_witness(&self, trace: &Trace) -> Result<()> {
        // Create a statement from the trace
        let statement = self.circuit.create_statement(trace)?;

        // Create a memory allocator for the witness
        let allocator = Bump::new();

        // Build the witness structure
        let mut witness = self
            .circuit
            .cs
            .build_witness::<ProverPackedField>(&allocator);

        // Fill all table witnesses in sequence
        // 1. Fill PROM table with program instructions
        witness.fill_table_sequential(&self.circuit.prom_table, &trace.program)?;

        // 2. Fill VROM address space table with the full address space
        let vrom_addr_space_size = statement.table_sizes[self.circuit.vrom_addr_space_table.id()];
        let vrom_addr_space: Vec<u32> = (0..vrom_addr_space_size as u32).collect();
        witness.fill_table_sequential(&self.circuit.vrom_addr_space_table, &vrom_addr_space)?;

        // 3. Fill VROM write table with writes
        witness.fill_table_sequential(&self.circuit.vrom_write_table, &trace.vrom_writes)?;

        // 4. Fill VROM skip table with skipped addresses
        // Generate the list of skipped addresses (addresses not in vrom_writes)
        let write_addrs: std::collections::HashSet<u32> =
            trace.vrom_writes.iter().map(|(addr, _, _)| *addr).collect();

        let vrom_skips: Vec<u32> = (0..vrom_addr_space_size as u32)
            .filter(|addr| !write_addrs.contains(addr))
            .collect();

        witness.fill_table_sequential(&self.circuit.vrom_skip_table, &vrom_skips)?;

        // 5. Fill all event tables
        for table in &self.circuit.tables {
            table.fill(&mut witness, trace)?;
        }

        binius_m3::builder::test_utils::validate_system_witness::<OptimalUnderlier128b>(
            &self.circuit.cs,
            witness,
            statement.boundaries,
        );

        Ok(())
    }
}

/// Verify a PetraVM execution proof.
///
/// This function:
/// 1. Uses the provided compiled constraint system
/// 2. Verifies the proof against the statement
///
/// # Arguments
/// * `statement` - The complete statement for verification
/// * `compiled_cs` - The pre-compiled constraint system
/// * `proof` - The proof to verify (taken by value)
///
/// # Returns
/// * Result indicating success or error
pub fn verify_proof(
    statement: &Statement,
    compiled_cs: &ConstraintSystem<B128>,
    proof: Proof,
) -> Result<()> {
    verify::<
        OptimalUnderlier128b,
        CanonicalTowerFamily,
        Groestl256,
        Groestl256ByteCompression,
        HasherChallenger<Groestl256>,
    >(
        compiled_cs,
        LOG_INV_RATE,
        SECURITY_BITS,
        &statement.boundaries,
        proof,
    )?;

    Ok(())
}
