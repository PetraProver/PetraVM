use anyhow::Result;
use clap::{value_parser, Parser};
use petravm_asm::isa::GenericISA;
use petravm_prover::{
    prover::{verify_proof, Prover},
    test_utils::{fibonacci, generate_fibonacci_trace},
};

#[derive(Debug, Parser)]
struct Args {
    /// The targeted index of the Fibonacci sequence.
    #[arg(short, long, default_value_t = 100, value_parser = value_parser!(u32).range(1..))]
    n: u32,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let _guard = petravm_asm::init_logger();

    let res = fibonacci(args.n);

    println!("Generating trace for fib({}) = {}...", args.n, res);

    let trace = generate_fibonacci_trace(args.n, res)?;
    trace.validate()?;

    let prover = Prover::new(Box::new(GenericISA));
    let (proof, statement, compiled_cs) = prover.prove(&trace)?;

    verify_proof(&statement, &compiled_cs, proof)?;

    Ok(())
}
