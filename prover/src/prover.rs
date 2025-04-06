//! Main prover interface for zCrayVM.
//!
//! This module provides the main entry point for creating proofs from
//! zCrayVM execution traces.

use anyhow::Result;
use binius_core::{
    constraint_system::{prove, validate, verify, ConstraintSystem, Proof},
    fiat_shamir::HasherChallenger,
    tower::CanonicalTowerFamily,
};
use binius_field::arch::OptimalUnderlier128b;
use binius_hal::make_portable_backend;
use binius_hash::groestl::{Groestl256, Groestl256ByteCompression};
use binius_m3::builder::{Statement, B128};
use bumpalo::Bump;

use crate::{circuit::Circuit, model::Trace, types::ProverPackedField};

const LOG_INV_RATE: usize = 1;
const SECURITY_BITS: usize = 100;

/// Main prover for zCrayVM.
// TODO: should be customizable by supported opcodes
pub struct Prover {
    /// Arithmetic circuit for zCrayVM
    circuit: Circuit,
}

impl Default for Prover {
    fn default() -> Self {
        Self::new()
    }
}

impl Prover {
    /// Create a new zCrayVM prover.
    pub fn new() -> Self {
        Self {
            circuit: Circuit::new(),
        }
    }

    /// Prove a zCrayVM execution trace.
    ///
    /// This function:
    /// 1. Creates a statement from the trace
    /// 2. Compiles the constraint system
    /// 3. Builds and fills the witness
    /// 4. Validates the witness against the constraints
    /// 5. Generates a proof
    ///
    /// # Arguments
    /// * `trace` - The zCrayVM execution trace to prove
    ///
    /// # Returns
    /// * Result containing the proof, statement, and compiled constraint system
    pub fn prove(&self, trace: &Trace) -> Result<(Proof, Statement, ConstraintSystem<B128>)> {
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
            .build_witness::<ProverPackedField>(&allocator, &statement)?;

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

        // 7. Fill BNZ table with branch not zero events
        witness.fill_table_sequential(&self.circuit.bnz_table, trace.bnz_events())?;

        // 8. Fill BZ table with branch zero events
        witness.fill_table_sequential(&self.circuit.bz_table, trace.bz_events())?;

        // Convert witness to multilinear extension format for validation
        let witness = witness.into_multilinear_extension_index();

        // Validate the witness against the constraint system
        validate::validate_witness(&compiled_cs, &statement.boundaries, &witness)?;

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
}

/// Verify a zCrayVM execution proof.
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
    // Verify the proof
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
