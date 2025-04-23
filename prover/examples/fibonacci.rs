use std::time::Instant;

use anyhow::Result;
use clap::{value_parser, Parser};
use zcrayvm_assembly::isa::GenericISA;
use zcrayvm_prover::{
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

    let res = fibonacci(args.n);

    println!("Generating trace for fib({}) = {}...", args.n, res);

    let start = Instant::now();
    let trace = generate_fibonacci_trace(args.n, res)?;
    println!("Trace generation time: {:?}", start.elapsed());

    trace.validate()?;

    let prover = Prover::new(Box::new(GenericISA));
    let start = Instant::now();
    let (proof, statement, compiled_cs) = prover.prove(&trace)?;
    println!("Proof generation time: {:?}", start.elapsed());

    let start = Instant::now();
    verify_proof(&statement, &compiled_cs, proof)?;
    println!("Proof verification time: {:?}", start.elapsed());

    Ok(())
}
