use anyhow::Result;
use petravm_asm::init_logger;
use petravm_asm::isa::GenericISA;
use petravm_prover::prover::{verify_proof, Prover};
use petravm_prover::test_utils::{fibonacci, generate_fibonacci_trace};

#[test]
fn test_fibonacci() -> Result<()> {
    init_logger();
    // Step 1: Generate trace
    let n = 11;
    let res = fibonacci(n);

    let trace = generate_fibonacci_trace(n, res)?;

    // Step 2: Validate trace
    trace.validate()?;

    // Step 3: Create prover
    let prover = Prover::new(Box::new(GenericISA));

    // Step 4: Generate proof
    let (proof, statement, compiled_cs) = prover.prove(&trace)?;

    // Step 5: Verify proof
    verify_proof(&statement, &compiled_cs, proof)
}
