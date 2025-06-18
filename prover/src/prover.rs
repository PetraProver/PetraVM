//! Main prover interface for PetraVM.
//!
//! This module provides the main entry point for creating proofs from
//! PetraVM execution traces.

use anyhow::{anyhow, Result};
use binius_compute::{alloc::HostBumpAllocator, cpu::alloc::CpuComputeAllocator, ComputeHolder};
use binius_core::{
    constraint_system::{prove, verify, ConstraintSystem, Proof},
    fiat_shamir::HasherChallenger,
};
use binius_fast_compute::layer::FastCpuLayerHolder;
use binius_field::arch::OptimalUnderlier;
use binius_field::tower::CanonicalTowerFamily;
use binius_hal::make_portable_backend;
use binius_hash::groestl::{Groestl256, Groestl256ByteCompression};
use binius_m3::builder::{WitnessIndex, B128};
use petravm_asm::isa::ISA;
use tracing::instrument;

use crate::types::Statement;
use crate::{circuit::Circuit, model::Trace, types::ProverPackedField};

const LOG_INV_RATE: usize = 1;
const SECURITY_BITS: usize = 100;
#[cfg(not(feature = "disable_prom_channel"))]
pub(crate) const PROM_MULTIPLICITY_BITS: usize = 32;
#[cfg(not(feature = "disable_vrom_channel"))]
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

    #[instrument(level = "info", skip_all)]
    pub fn generate_witness<'a>(
        &self,
        trace: &Trace,
        allocator: &'a HostBumpAllocator<'a, ProverPackedField>,
    ) -> Result<WitnessIndex<'_, 'a, ProverPackedField>> {
        // Build the witness structure
        let mut witness = WitnessIndex::new(&self.circuit.cs, allocator);

        // Fill all table witnesses in sequence

        // 1. Fill PROM table with program instructions
        witness.fill_table_parallel(&self.circuit.prom_table, &trace.program)?;

        // 2. Fill VROM table with VROM addresses and values
        let vrom_addr_space_size = (trace.max_vrom_addr + 1).next_power_of_two();
        let mut vrom_with_multiplicities = (0..vrom_addr_space_size)
            .map(|addr| (addr as u32, 0u32, 0u32))
            .collect::<Vec<_>>();
        for &(addr, val, mul) in trace.vrom_writes.iter() {
            vrom_with_multiplicities[addr as usize] = (addr, val, mul);
        }
        vrom_with_multiplicities.sort_by_key(|(_, _, mul)| *mul);
        vrom_with_multiplicities.reverse();
        witness.fill_table_sequential(&self.circuit.vrom_table, &vrom_with_multiplicities)?;

        // 3. Fill the right shifter table
        witness.fill_table_sequential(
            &self.circuit.right_shifter_table,
            trace.right_shift_events(),
        )?;

        // 4. Fill all event tables
        for table in &self.circuit.tables {
            table.fill(&mut witness, trace)?;
        }

        Ok(witness)
    }

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
    #[instrument(level = "info", skip_all)]
    pub fn prove(&self, trace: &Trace) -> Result<(Proof, Statement, ConstraintSystem<B128>)> {
        // Create a statement from the trace
        let statement = self.circuit.create_statement(trace)?;

        // Compile the constraint system
        let compiled_cs = self.circuit.cs.compile().map_err(|e| anyhow!(e))?;

        let witness_allocator_span = tracing::info_span!("Witness Alloc").entered();

        // Create a memory allocator for the witness
        let mut allocator = CpuComputeAllocator::new(1 << 25);
        let allocator = allocator.into_bump_allocator();

        drop(witness_allocator_span);

        // Convert witness to multilinear extension format
        let witness = self
            .generate_witness(trace, &allocator)?
            .into_multilinear_extension_index();

        // Validate the witness against the constraint system in debug mode only
        #[cfg(debug_assertions)]
        binius_core::constraint_system::validate::validate_witness(
            &compiled_cs,
            &statement.boundaries,
            &statement.table_sizes,
            &witness,
        )?;

        let ccs_digest = compiled_cs.digest::<Groestl256>();

        let hal_span = tracing::info_span!("HAL Setup").entered();
        let mut compute_holder =
            FastCpuLayerHolder::<CanonicalTowerFamily, ProverPackedField>::new(1 << 20, 1 << 26);
        drop(hal_span);

        // Generate the proof
        let proof = prove::<
            _,
            OptimalUnderlier,
            CanonicalTowerFamily,
            Groestl256,
            Groestl256ByteCompression,
            HasherChallenger<Groestl256>,
            _,
            _,
            _,
        >(
            &mut compute_holder.to_data(),
            &compiled_cs,
            LOG_INV_RATE,
            SECURITY_BITS,
            &ccs_digest,
            &statement.boundaries,
            &statement.table_sizes,
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
        let mut allocator = CpuComputeAllocator::new(1 << 25);
        let allocator = allocator.into_bump_allocator();

        // Fill all table witnesses in sequence
        let witness = self.generate_witness(trace, &allocator)?;

        binius_m3::builder::test_utils::validate_system_witness::<OptimalUnderlier>(
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
#[instrument(level = "info", skip_all)]
pub fn verify_proof(
    statement: &Statement,
    compiled_cs: &ConstraintSystem<B128>,
    proof: Proof,
) -> Result<()> {
    let ccs_digest = compiled_cs.digest::<Groestl256>();

    verify::<
        OptimalUnderlier,
        CanonicalTowerFamily,
        Groestl256,
        Groestl256ByteCompression,
        HasherChallenger<Groestl256>,
    >(
        compiled_cs,
        LOG_INV_RATE,
        SECURITY_BITS,
        &ccs_digest,
        &statement.boundaries,
        proof,
    )?;

    Ok(())
}
