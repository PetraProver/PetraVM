use anyhow::Result;
use petravm_asm::execution::FramePointer;
use petravm_asm::init_logger;
use petravm_asm::isa::GenericISA;
use petravm_prover::model::Trace;
use petravm_prover::prover::{verify_proof, Prover};
use petravm_prover::test_utils::generate_trace;

pub fn generate_opcodes_trace() -> Result<(Trace, FramePointer)> {
    // Read the Fibonacci assembly code from examples directory
    let asm_path = format!("{}/../examples/opcodes.asm", env!("CARGO_MANIFEST_DIR"));
    let asm_code = std::fs::read_to_string(asm_path)
        .map_err(|e| anyhow::anyhow!("Failed to read opcodes.asm: {}", e))?;

    // Initialize memory with:
    // Slot 0: Return PC = 0
    // Slot 1: Return FP = 0
    // Slot 2: Final result 0 means success
    let init_values = vec![0, 0, 0];

    let isa = Box::new(GenericISA);
    generate_trace(asm_code, Some(init_values), None, isa)
}

#[test]
fn test_all_opcodes() -> Result<()> {
    let _ = init_logger();
    // Step 1: Generate trace
    let (trace, final_fp) = generate_opcodes_trace()?;

    // Step 2: Validate trace
    trace.validate()?;

    // Step 3: Create prover
    let prover = Prover::new(Box::new(GenericISA));

    // Step 4: Generate proof
    let (proof, statement, compiled_cs) = prover.prove_with_final_fp(&trace, *final_fp as u128)?;

    // Step 5: Verify proof
    verify_proof(&statement, &compiled_cs, proof)
}
