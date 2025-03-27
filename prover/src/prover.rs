//! Main prover interface for zCrayVM.
//!
//! This module provides the main entry point for creating proofs from
//! zCrayVM execution traces.

use anyhow::Result;
use binius_core::{
    constraint_system::{prove, validate, Proof},
    fiat_shamir::HasherChallenger,
    tower::CanonicalTowerFamily,
};
use binius_field::arch::OptimalUnderlier128b;
use binius_hal::make_portable_backend;
use binius_hash::groestl::{Groestl256, Groestl256ByteCompression};
use bumpalo::Bump;

use crate::{circuit::ZkVMCircuit, model::ZkVMTrace};

const LOG_INV_RATE: usize = 1;
const SECURITY_BITS: usize = 100;

/// Main prover for zCrayVM.
// TODO: should be customizable by supported opcodes
pub struct ZkVMProver {
    /// Arithmetic circuit for zCrayVM
    circuit: ZkVMCircuit,
}

impl ZkVMProver {
    /// Create a new zCrayVM prover.
    pub fn new() -> Self {
        Self {
            circuit: ZkVMCircuit::new(),
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
    /// * Result containing the proof or error
    pub fn prove(&self, trace: &ZkVMTrace) -> Result<Proof> {
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

        // 2. Fill LDI table with load immediate events
        witness.fill_table_sequential(&self.circuit.ldi_table, trace.ldi_events())?;

        // 3. Fill RET table with return events
        witness.fill_table_sequential(&self.circuit.ret_table, trace.ret_events())?;

        // Convert witness to multilinear extension format for validation
        let mle_witness = witness.into_multilinear_extension_index(&statement);

        // Validate the witness against the constraint system
        validate::validate_witness(&compiled_cs, &statement.boundaries, &mle_witness)?;

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
            mle_witness,
            &make_portable_backend(),
        )?;

        Ok(proof)
    }

    /// Verify a zCrayVM execution proof.
    ///
    /// This function:
    /// 1. Creates a statement from the trace
    /// 2. Compiles the constraint system
    /// 3. Verifies the proof against the statement
    ///
    /// # Arguments
    /// * `trace` - The zCrayVM execution trace
    /// * `proof` - The proof to verify
    ///
    /// # Returns
    /// * Result indicating success or error
    pub fn verify(&self, trace: &ZkVMTrace, proof: &Proof) -> Result<()> {
        // Create a statement from the trace
        let statement = self.circuit.create_statement(trace)?;

        // Compile the constraint system
        let compiled_cs = self.circuit.compile(&statement)?;

        // Verify the proof
        binius_core::constraint_system::verify::<
            OptimalUnderlier128b,
            CanonicalTowerFamily,
            Groestl256,
            Groestl256ByteCompression,
            HasherChallenger<Groestl256>,
        >(
            &compiled_cs,
            LOG_INV_RATE,
            SECURITY_BITS,
            &statement.boundaries,
            proof.clone(),
        )?;

        Ok(())
    }
}
