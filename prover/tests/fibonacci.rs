use std::time::Instant;

use anyhow::Result;
use petravm_asm::isa::GenericISA;
use petravm_prover::prover::{verify_proof, Prover};
use petravm_prover::test_utils::{fibonacci, generate_fibonacci_trace};

#[test]
fn test_fibonacci() -> Result<()> {
    // Step 1: Generate trace
    let n = 11;
    let res = fibonacci(n);

    let start = Instant::now();
    let trace = generate_fibonacci_trace(n, res)?;
    let trace_time = start.elapsed();
    println!("Trace generation time: {:?}", trace_time);

    // Step 2: Validate trace
    trace.validate()?;

    // Step 3: Create prover
    let prover = Prover::new(Box::new(GenericISA));

    // Step 4: Generate proof
    let start = Instant::now();
    let (proof, statement, compiled_cs) = prover.prove(&trace)?;
    let proving_time = start.elapsed();
    println!("Proof generation time: {:?}", proving_time);

    // Step 5: Verify proof
    let start = Instant::now();
    verify_proof(&statement, &compiled_cs, proof)?;
    let verification_time = start.elapsed();
    println!("Proof verification time: {:?}", verification_time);

    Ok(())
}
