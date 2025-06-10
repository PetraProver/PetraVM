use anyhow::Result;
use petravm_asm::{init_logger, isa::GenericISA};
use petravm_prover::{
    prover::{verify_proof, Prover},
    test_utils::generate_asm_trace,
};

fn run_test(files: &[&str], init_values: Vec<u32>) -> Result<()> {
    // Step 1: Generate trace
    let trace = generate_asm_trace(files, init_values).unwrap();

    // Step 2: Validate trace
    trace.validate()?;

    // Step 3: Create prover
    let prover = Prover::new(Box::new(GenericISA));

    // Step 4: Generate proof
    let (proof, statement, compiled_cs) = prover.prove(&trace)?;

    // Step 5: Verify proof
    verify_proof(&statement, &compiled_cs, proof)
}

#[test]
fn test_add() {
    let files = ["add.asm"];
    // Initialize memory with:
    // Slot 0: Return PC = 0
    // Slot 1: Return FP = 0
    let init_values = vec![0, 0];
    run_test(&files, init_values).unwrap();
}

#[test]
fn test_div() {
    let files = ["div.asm"];
    // Initialize memory with:
    // Slot 0: Return PC = 0
    // Slot 1: Return FP = 0
    // Slot 2: Arg: a
    // Slot 3: Arg: b
    let init_values = |a, b| vec![0, 0, a, b];
    // Test case 1: a < b
    run_test(&files, init_values(5, 10)).unwrap();

    // Test case 2: a == b
    run_test(&files, init_values(10, 10)).unwrap();

    // Test case 3: a > b
    run_test(&files, init_values(20, 6)).unwrap();

    // Test case 4: a = 0
    run_test(&files, init_values(0, 7)).unwrap();

    // Test case 5: b = 1
    run_test(&files, init_values(15, 1)).unwrap();
}

#[test]
fn test_bezout() {
    let files = ["bezout.asm", "div.asm"];
    // Initialize memory with:
    // Slot 0: Return PC = 0
    // Slot 1: Return FP = 0
    // Slot 2: Arg: a
    // Slot 3: Arg: b
    let init_values = |a, b| vec![0, 0, a, b];
    // gcd(56, 15) = 1
    run_test(&files, init_values(56, 15)).unwrap();

    // gcd(48, 18) = 6
    run_test(&files, init_values(48, 18)).unwrap();

    // gcd(101, 103) = 1
    run_test(&files, init_values(101, 103)).unwrap();

    // gcd(270, 192) = 6
    run_test(&files, init_values(270, 192)).unwrap();
}

#[test]
fn test_bit_ops() {
    let files = ["bit_ops.asm"];
    // Initialize memory with:
    // Slot 0: Return PC = 0
    // Slot 1: Return FP = 0
    let init_values = vec![0, 0];
    run_test(&files, init_values).unwrap();
}

#[test]
fn test_bit_shifts() {
    let files = ["bit_shifts.asm"];
    // Initialize memory with:
    // Slot 0: Return PC = 0
    // Slot 1: Return FP = 0
    let init_values = vec![0, 0];
    run_test(&files, init_values).unwrap();
}

#[test]
fn test_branch() {
    let files = ["branch.asm"];
    // Initialize memory with:
    // Slot 0: Return PC = 0
    // Slot 1: Return FP = 0
    // Slot 2: Arg: n
    let init_values = |n| vec![0, 0, n];
    run_test(&files, init_values(2)).unwrap();
    run_test(&files, init_values(3)).unwrap();
}

#[test]
fn test_func_call() {
    let files = ["func_call.asm"];
    // Initialize memory with:
    // Slot 0: Return PC = 0
    // Slot 1: Return FP = 0
    let init_values = vec![0, 0];
    run_test(&files, init_values).unwrap();
}

#[test]
fn test_linked_list() {
    let files = ["linked_list.asm"];
    // Initialize memory with:
    // Slot 0: Return PC = 0
    // Slot 1: Return FP = 0
    // Slot 2: Arg: cur_val
    // Slot 3: Arg: list_size

    let list_size = 20;
    let init_values = |cur_val| vec![0, 0, cur_val, list_size];
    for cur_val in 0..list_size {
        run_test(&files, init_values(cur_val)).unwrap();
    }
}

#[test]
fn test_mul() {
    let files = ["mul.asm"];
    // Initialize memory with:
    // Slot 0: Return PC = 0
    // Slot 1: Return FP = 0
    let init_values = vec![0, 0];
    run_test(&files, init_values).unwrap();
}

#[test]
fn test_non_tail_long_div() {
    let files = ["non_tail_long_div.asm"];
    // Initialize memory with:
    // Slot 0: Return PC = 0
    // Slot 1: Return FP = 0
    // Slot 2: Arg: a
    // Slot 3: Arg: b
    init_logger();
    let init_values = |a, b| vec![0, 0, a, b];
    // Test case 1: a < b
    run_test(&files, init_values(1, 3)).unwrap();

    // Test case 2: a == b
    run_test(&files, init_values(20, 4)).unwrap();

    // Test case 3: a > b
    run_test(&files, init_values(15, 6)).unwrap();

    // Test case 4: a = 0
    run_test(&files, init_values(7, 7)).unwrap();

    // Test case 5: b = 1
    run_test(&files, init_values(9, 2)).unwrap();
}

#[test]
fn test_tail_long_div() {
    let files = ["tail_long_div.asm"];
    // Initialize memory with:
    // Slot 0: Return PC = 0
    // Slot 1: Return FP = 0
    // Slot 2: Arg: a
    // Slot 3: Arg: b
    let init_values = |a, b| vec![0, 0, a, b];
    // Test case 1: a < b
    run_test(&files, init_values(10, 3)).unwrap();

    // Test case 2: a == b
    run_test(&files, init_values(20, 4)).unwrap();

    // Test case 3: a > b
    run_test(&files, init_values(15, 6)).unwrap();

    // Test case 4: a = 0
    run_test(&files, init_values(7, 7)).unwrap();

    // Test case 5: b = 1
    run_test(&files, init_values(9, 2)).unwrap();
}
