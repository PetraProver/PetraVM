//! Main prover interface for zCrayVM.
//!
//! This module provides the main entry point for creating proofs from
//! zCrayVM execution traces.

use binius_field::arch::OptimalUnderlier128b;
use bumpalo::Bump;

use crate::{
    model::ZkVMTrace,
    circuit::ZkVMCircuit,
};

/// Main prover for zCrayVM.
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
    pub fn prove(&self, trace: &ZkVMTrace) -> anyhow::Result<()> {
        // Create a statement from the trace
        let statement = self.circuit.create_statement(trace)?;
        
        // Compile the constraint system
        let compiled_cs = self.circuit.compile(&statement)?;
        
        // Create and fill witness
        let allocator = Bump::new();
        let mut witness = self.circuit.cs.build_witness::<OptimalUnderlier128b>(
            &allocator, &statement
        )?;
        
        // Fill PROM table
        witness.fill_table_sequential(&self.circuit.prom_table, &trace.program)?;
        
        // Fill LDI table
        witness.fill_table_sequential(&self.circuit.ldi_table, &trace.ldi_events)?;
        
        // Fill RET table
        witness.fill_table_sequential(&self.circuit.ret_table, &trace.ret_events)?;
        
        // Convert witness to MLE for validation
        let mle_witness = witness.into_multilinear_extension_index(&statement);
        
        // Validate the witness
        binius_core::constraint_system::validate::validate_witness(
            &compiled_cs,
            &statement.boundaries,
            &mle_witness,
        )?;
        
        Ok(())
    }
    
}